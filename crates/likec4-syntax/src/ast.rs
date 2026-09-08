//! Thin typed wrappers over the rowan syntax tree (rust-analyzer `ast` style).
//!
//! Every wrapper is a newtype around a [`SyntaxNode`] whose kind is fixed; accessors
//! are positional lookups of child nodes/tokens (see [`support`]). Wrappers never fail
//! once constructed: a malformed tree simply yields `None` from optional accessors.
//!
//! Node kinds and children follow the shapes documented in `docs/DESIGN.md`.

use std::marker::PhantomData;

use text_size::TextRange;

use crate::kind::SyntaxKind::{self, *};
use crate::kind::{SyntaxNode, SyntaxNodeChildren, SyntaxToken};

/// A typed view over a [`SyntaxNode`] of a specific kind (or set of kinds).
pub trait AstNode: Sized {
    /// Whether nodes of `kind` can be wrapped by this type.
    fn can_cast(kind: SyntaxKind) -> bool;

    /// Wrap `syntax` if its kind matches.
    fn cast(syntax: SyntaxNode) -> Option<Self>;

    /// The underlying node.
    fn syntax(&self) -> &SyntaxNode;

    /// Byte range of the node (including trivia that Langium attaches to it).
    fn text_range(&self) -> TextRange {
        self.syntax().text_range()
    }
}

/// Iterator over the children of a node that cast to `N`.
#[derive(Clone, Debug)]
pub struct AstChildren<N> {
    inner: SyntaxNodeChildren,
    _marker: PhantomData<N>,
}

impl<N> AstChildren<N> {
    fn new(parent: &SyntaxNode) -> Self {
        AstChildren { inner: parent.children(), _marker: PhantomData }
    }
}

impl<N: AstNode> Iterator for AstChildren<N> {
    type Item = N;

    fn next(&mut self) -> Option<N> {
        self.inner.find_map(N::cast)
    }
}

/// Positional child/token lookups used to implement the accessors.
pub mod support {
    use super::{AstChildren, AstNode, SyntaxKind, SyntaxNode, SyntaxToken};

    /// First child node that casts to `N`.
    pub fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
        parent.children().find_map(N::cast)
    }

    /// `n`-th (0-based) child node that casts to `N`.
    pub fn nth_child<N: AstNode>(parent: &SyntaxNode, n: usize) -> Option<N> {
        children(parent).nth(n)
    }

    /// All child nodes that cast to `N`.
    pub fn children<N: AstNode>(parent: &SyntaxNode) -> AstChildren<N> {
        AstChildren::new(parent)
    }

    /// First direct token child of `kind`.
    pub fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
        tokens(parent, kind).next()
    }

    /// `n`-th (0-based) direct token child of `kind`.
    pub fn nth_token(parent: &SyntaxNode, kind: SyntaxKind, n: usize) -> Option<SyntaxToken> {
        tokens(parent, kind).nth(n)
    }

    /// All direct token children of `kind`, in order.
    pub fn tokens(parent: &SyntaxNode, kind: SyntaxKind) -> impl Iterator<Item = SyntaxToken> {
        parent.children_with_tokens().filter_map(|el| el.into_token()).filter(move |t| t.kind() == kind)
    }

    /// First direct token child satisfying `pred`.
    pub fn find_token(parent: &SyntaxNode, pred: impl Fn(SyntaxKind) -> bool) -> Option<SyntaxToken> {
        parent.children_with_tokens().filter_map(|el| el.into_token()).find(|t| pred(t.kind()))
    }

    /// First direct token child that is a keyword (`*_KW`).
    pub fn keyword(parent: &SyntaxNode) -> Option<SyntaxToken> {
        find_token(parent, SyntaxKind::is_keyword)
    }

    /// First direct `STRING` or `MARKDOWN_STRING` token child.
    pub fn string(parent: &SyntaxNode) -> Option<SyntaxToken> {
        find_token(parent, SyntaxKind::is_string)
    }
}

macro_rules! ast_node {
    ($(#[$attr:meta])* $name:ident, $kind:ident) => {
        $(#[$attr])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                if Self::can_cast(syntax.kind()) {
                    Some($name { syntax })
                } else {
                    None
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

macro_rules! ast_enum {
    ($(#[$attr:meta])* $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$attr])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant($variant),)+
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                $($variant::can_cast(kind))||+
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                $(
                    if $variant::can_cast(syntax.kind()) {
                        return $variant::cast(syntax).map($name::$variant);
                    }
                )+
                None
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $($name::$variant(it) => it.syntax(),)+
                }
            }
        }

        $(
            impl From<$variant> for $name {
                fn from(it: $variant) -> Self {
                    $name::$variant(it)
                }
            }
        )+
    };
}

// ---------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------

ast_node!(
    /// The document root.
    Root,
    ROOT
);
ast_node!(
    /// `likec4lib { icons { ... } }`
    Likec4Lib,
    LIKEC4LIB
);
ast_node!(
    /// One `LIB_ICON` inside `likec4lib`.
    LibIconDecl,
    LIB_ICON_DECL
);
ast_node!(
    /// `import a, b from 'project'`
    Imports,
    IMPORTS
);
ast_node!(
    /// The imported names of an `import`.
    Imported,
    IMPORTED
);
ast_node!(
    /// `specification { ... }`
    Specification,
    SPECIFICATION
);
ast_node!(
    /// `model { ... }`
    Model,
    MODEL
);
ast_node!(
    /// `views { ... }`
    Views,
    VIEWS
);
ast_node!(
    /// `deployment { ... }`
    Deployment,
    DEPLOYMENT
);
ast_node!(
    /// `global { ... }`
    Globals,
    GLOBALS
);

impl Root {
    /// All `import` statements.
    pub fn imports(&self) -> AstChildren<Imports> {
        support::children(&self.syntax)
    }

    /// All `specification` blocks.
    pub fn specifications(&self) -> AstChildren<Specification> {
        support::children(&self.syntax)
    }

    /// All `model` blocks.
    pub fn models(&self) -> AstChildren<Model> {
        support::children(&self.syntax)
    }

    /// All `views` blocks.
    pub fn views(&self) -> AstChildren<Views> {
        support::children(&self.syntax)
    }

    /// All `deployment` blocks.
    pub fn deployments(&self) -> AstChildren<Deployment> {
        support::children(&self.syntax)
    }

    /// All `global` blocks.
    pub fn globals(&self) -> AstChildren<Globals> {
        support::children(&self.syntax)
    }

    /// All `likec4lib` blocks.
    pub fn likec4libs(&self) -> AstChildren<Likec4Lib> {
        support::children(&self.syntax)
    }
}

impl Likec4Lib {
    pub fn icons(&self) -> AstChildren<LibIconDecl> {
        support::children(&self.syntax)
    }
}

impl LibIconDecl {
    pub fn icon_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, LIB_ICON)
    }
}

impl Imports {
    pub fn imported(&self) -> Option<Imported> {
        support::child(&self.syntax)
    }

    /// The `'project'` string.
    pub fn project_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }
}

impl Imported {
    pub fn name_tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, IDENT)
    }
}

// ---------------------------------------------------------------------------
// Specification
// ---------------------------------------------------------------------------

ast_node!(
    /// `element kind { ... }`
    SpecElementKind,
    SPEC_ELEMENT_KIND
);
ast_node!(
    /// `deploymentNode kind { ... }`
    SpecDeploymentNodeKind,
    SPEC_DEPLOYMENT_NODE_KIND
);
ast_node!(
    /// `relationship kind { ... }`
    SpecRelationshipKind,
    SPEC_RELATIONSHIP_KIND
);
ast_node!(
    /// `tag name { color ... }`
    SpecTag,
    SPEC_TAG
);
ast_node!(
    /// `color name #hex`
    SpecColor,
    SPEC_COLOR
);
ast_node!(
    /// String property inside a specification (`title`, `notation`, ...).
    SpecStringProperty,
    SPEC_STRING_PROPERTY
);
ast_node!(
    /// `rgb(...)` / `rgba(...)`
    RgbaColor,
    RGBA_COLOR
);
ast_node!(
    /// `#hex`
    HexColor,
    HEX_COLOR
);

ast_enum!(
    /// A colour literal.
    ColorLiteral { RgbaColor, HexColor }
);

impl Specification {
    pub fn element_kinds(&self) -> AstChildren<SpecElementKind> {
        support::children(&self.syntax)
    }

    pub fn deployment_node_kinds(&self) -> AstChildren<SpecDeploymentNodeKind> {
        support::children(&self.syntax)
    }

    pub fn relationship_kinds(&self) -> AstChildren<SpecRelationshipKind> {
        support::children(&self.syntax)
    }

    pub fn tags(&self) -> AstChildren<SpecTag> {
        support::children(&self.syntax)
    }

    pub fn colors(&self) -> AstChildren<SpecColor> {
        support::children(&self.syntax)
    }
}

impl SpecElementKind {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn string_properties(&self) -> AstChildren<SpecStringProperty> {
        support::children(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    pub fn style(&self) -> Option<StyleProperty> {
        support::child(&self.syntax)
    }

    pub fn body_l_curly(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, L_CURLY)
    }
}

impl SpecDeploymentNodeKind {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn string_properties(&self) -> AstChildren<SpecStringProperty> {
        support::children(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    pub fn style(&self) -> Option<StyleProperty> {
        support::child(&self.syntax)
    }
}

impl SpecRelationshipKind {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn string_properties(&self) -> AstChildren<SpecStringProperty> {
        support::children(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    /// `color`, `line`, `head`, `tail`, `multiple` leafs.
    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }
}

impl SpecTag {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn color_kw(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, COLOR_KW)
    }

    /// Named colour (`color red`); `None` when a literal is used.
    pub fn color_name_token(&self) -> Option<SyntaxToken> {
        support::nth_token(&self.syntax, IDENT, 1)
    }

    pub fn color_literal(&self) -> Option<ColorLiteral> {
        support::child(&self.syntax)
    }
}

impl SpecColor {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn literal(&self) -> Option<ColorLiteral> {
        support::child(&self.syntax)
    }
}

impl SpecStringProperty {
    pub fn key_token(&self) -> Option<SyntaxToken> {
        support::keyword(&self.syntax)
    }

    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::string(&self.syntax)
    }
}

impl RgbaColor {
    /// `rgb` / `rgba` word.
    pub fn function_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// Red, green, blue and (optional) alpha components in order.
    pub fn components(&self) -> impl Iterator<Item = SyntaxToken> {
        self.syntax
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| matches!(t.kind(), NUMBER | FLOAT | PERCENT))
    }
}

impl HexColor {
    /// The token after `#` (`HEX`, `NUMBER` or `IDENT`).
    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::find_token(&self.syntax, |k| matches!(k, HEX | NUMBER | IDENT))
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

ast_node!(
    /// `kind name 'title' { ... }` / `name = kind`
    Element,
    ELEMENT
);
ast_node!(
    /// Body of an element.
    ElementBody,
    ELEMENT_BODY
);
ast_node!(
    /// `a -> b 'title' #tag { ... }`
    Relation,
    RELATION
);
ast_node!(
    /// Body of a relation.
    RelationBody,
    RELATION_BODY
);
ast_node!(
    /// `extend a.b { ... }`
    ExtendElement,
    EXTEND_ELEMENT
);
ast_node!(
    /// Body of `extend element`.
    ExtendElementBody,
    EXTEND_ELEMENT_BODY
);
ast_node!(
    /// `extend a -> b { ... }`
    ExtendRelation,
    EXTEND_RELATION
);
ast_node!(
    /// Body of `extend relation`.
    ExtendRelationBody,
    EXTEND_RELATION_BODY
);
ast_node!(
    /// `#a, #b`
    Tags,
    TAGS
);
ast_node!(
    /// `#tag`
    TagRef,
    TAG_REF
);
ast_node!(
    /// `a.b.c`
    FqnRef,
    FQN_REF
);
ast_node!(
    /// `.kind` relation connector.
    RelationKindDotRef,
    RELATION_KIND_DOT_REF
);

ast_enum!(
    /// A statement inside `model` or an element body.
    ModelStatement { Element, Relation, ExtendElement, ExtendRelation }
);

impl Model {
    pub fn statements(&self) -> AstChildren<ModelStatement> {
        support::children(&self.syntax)
    }

    pub fn elements(&self) -> AstChildren<Element> {
        support::children(&self.syntax)
    }

    pub fn relations(&self) -> AstChildren<Relation> {
        support::children(&self.syntax)
    }

    pub fn extend_elements(&self) -> AstChildren<ExtendElement> {
        support::children(&self.syntax)
    }

    pub fn extend_relations(&self) -> AstChildren<ExtendRelation> {
        support::children(&self.syntax)
    }
}

/// `(kind, name)` of a `kind name | name = kind` head.
fn kind_and_name(node: &SyntaxNode) -> (Option<SyntaxToken>, Option<SyntaxToken>) {
    let mut idents = support::tokens(node, IDENT);
    let first = idents.next();
    let second = idents.next();
    if support::token(node, EQ).is_some() {
        (second, first)
    } else {
        (first, second)
    }
}

impl Element {
    /// The element kind identifier (works for both `kind name` and `name = kind`).
    pub fn kind_token(&self) -> Option<SyntaxToken> {
        kind_and_name(&self.syntax).0
    }

    /// The element name identifier.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        kind_and_name(&self.syntax).1
    }

    /// The element name as text.
    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    /// True for the `name = kind` form.
    pub fn is_assignment_form(&self) -> bool {
        support::token(&self.syntax, EQ).is_some()
    }

    /// All inline strings (title, summary, technology, tags).
    pub fn strings(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, STRING)
    }

    /// The first inline string (title).
    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<ElementBody> {
        support::child(&self.syntax)
    }
}

impl ElementBody {
    /// Leading `#tags`.
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    /// Properties in source order (they precede nested statements).
    pub fn properties(&self) -> AstChildren<ElementProperty> {
        support::children(&self.syntax)
    }

    /// Nested elements and relations in source order.
    pub fn statements(&self) -> AstChildren<ModelStatement> {
        support::children(&self.syntax)
    }

    /// Nested elements only.
    pub fn elements(&self) -> AstChildren<Element> {
        support::children(&self.syntax)
    }

    /// Nested relations only.
    pub fn relations(&self) -> AstChildren<Relation> {
        support::children(&self.syntax)
    }

    pub fn l_curly(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, L_CURLY)
    }

    pub fn r_curly(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, R_CURLY)
    }
}

/// Source/target of a relation-like node with positional `FQN_REF` children.
/// The source is present exactly when the node starts with an `FQN_REF`.
fn source_and_target(node: &SyntaxNode) -> (Option<FqnRef>, Option<FqnRef>) {
    let mut refs = support::children::<FqnRef>(node);
    let first = refs.next();
    if node.first_child_or_token().is_some_and(|el| el.kind() == FQN_REF) {
        (first, refs.next())
    } else {
        (None, first)
    }
}

/// Relationship kind of a relation-like node: `.kind` or `-[kind]->`.
fn relation_kind_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    if let Some(dot) = support::child::<RelationKindDotRef>(node) {
        return dot.name_token();
    }
    if support::token(node, ARROW_L_BRACK).is_some() {
        return support::token(node, IDENT);
    }
    None
}

impl Relation {
    /// The source reference; `None` for `-> target` inside an element body.
    pub fn source(&self) -> Option<FqnRef> {
        source_and_target(&self.syntax).0
    }

    /// The target reference.
    pub fn target(&self) -> Option<FqnRef> {
        source_and_target(&self.syntax).1
    }

    /// The relationship kind identifier from `.kind` or `-[kind]->`.
    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    /// The `.kind` connector node, if that form is used.
    pub fn dot_kind(&self) -> Option<RelationKindDotRef> {
        support::child(&self.syntax)
    }

    /// True for `<->` and `-[kind]<->`.
    pub fn is_bidirectional(&self) -> bool {
        support::token(&self.syntax, BI_ARROW).is_some()
            || support::token(&self.syntax, R_BRACK_BI_ARROW).is_some()
    }

    /// Inline strings: title, description, technology.
    pub fn strings(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, STRING)
    }

    /// The first inline string (title).
    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    /// Inline `#tags` after the strings.
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<RelationBody> {
        support::child(&self.syntax)
    }
}

impl RelationBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<RelationProperty> {
        support::children(&self.syntax)
    }
}

impl ExtendElement {
    pub fn target(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    pub fn body(&self) -> Option<ExtendElementBody> {
        support::child(&self.syntax)
    }
}

impl ExtendElementBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    pub fn metadata(&self) -> Option<MetadataProperty> {
        support::child(&self.syntax)
    }

    pub fn statements(&self) -> AstChildren<ModelStatement> {
        support::children(&self.syntax)
    }
}

impl ExtendRelation {
    pub fn source(&self) -> Option<FqnRef> {
        support::nth_child(&self.syntax, 0)
    }

    pub fn target(&self) -> Option<FqnRef> {
        support::nth_child(&self.syntax, 1)
    }

    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn body(&self) -> Option<ExtendRelationBody> {
        support::child(&self.syntax)
    }
}

impl ExtendRelationBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    pub fn metadata(&self) -> Option<MetadataProperty> {
        support::child(&self.syntax)
    }
}

impl Tags {
    /// All `#tag` references in order.
    pub fn tag_refs(&self) -> AstChildren<TagRef> {
        support::children(&self.syntax)
    }
}

impl TagRef {
    /// The identifier after `#`.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// The tag name as text.
    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }
}

impl FqnRef {
    /// Identifier segments in order.
    pub fn segments(&self) -> Vec<SyntaxToken> {
        support::tokens(&self.syntax, IDENT).collect()
    }

    /// Dotted text without any whitespace or comments (`a.b.c`).
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (i, seg) in support::tokens(&self.syntax, IDENT).enumerate() {
            if i > 0 {
                out.push('.');
            }
            out.push_str(seg.text());
        }
        out
    }

    /// The last segment.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::tokens(&self.syntax, IDENT).last()
    }
}

impl RelationKindDotRef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

ast_node!(
    /// `title 'x'`, `description`, `technology`, `summary`, `notation`, `notes`.
    StringProperty,
    STRING_PROPERTY
);
ast_node!(
    /// `link uri 'title'`
    LinkProperty,
    LINK_PROPERTY
);
ast_node!(
    /// `icon aws:x` / `icon none` / `icon https://...`
    IconProperty,
    ICON_PROPERTY
);
ast_node!(
    /// `metadata { ... }`
    MetadataProperty,
    METADATA_PROPERTY
);
ast_node!(
    /// Braces of a `metadata` property.
    MetadataBody,
    METADATA_BODY
);
ast_node!(
    /// `key 'value'` inside `metadata`.
    MetadataAttribute,
    METADATA_ATTRIBUTE
);
ast_node!(
    /// `['a', 'b']`
    MetadataArray,
    METADATA_ARRAY
);
ast_node!(
    /// `style { ... }`
    StyleProperty,
    STYLE_PROPERTY
);
ast_node!(
    /// `color red`
    ColorProperty,
    COLOR_PROPERTY
);
ast_node!(
    /// `shape person`
    ShapeProperty,
    SHAPE_PROPERTY
);
ast_node!(
    /// `border dashed`
    BorderProperty,
    BORDER_PROPERTY
);
ast_node!(
    /// `opacity 30%`
    OpacityProperty,
    OPACITY_PROPERTY
);
ast_node!(
    /// `multiple true`
    MultipleProperty,
    MULTIPLE_PROPERTY
);
ast_node!(
    /// `iconColor red`
    IconColorProperty,
    ICON_COLOR_PROPERTY
);
ast_node!(
    /// `iconSize md`
    IconSizeProperty,
    ICON_SIZE_PROPERTY
);
ast_node!(
    /// `iconPosition left`
    IconPositionProperty,
    ICON_POSITION_PROPERTY
);
ast_node!(
    /// `size md`
    SizeProperty,
    SIZE_PROPERTY
);
ast_node!(
    /// `padding md`
    PaddingProperty,
    PADDING_PROPERTY
);
ast_node!(
    /// `textSize md`
    TextSizeProperty,
    TEXT_SIZE_PROPERTY
);
ast_node!(
    /// `line dashed`
    LineProperty,
    LINE_PROPERTY
);
ast_node!(
    /// `head normal` / `tail none`
    ArrowProperty,
    ARROW_PROPERTY
);
ast_node!(
    /// `order 1`
    OrderProperty,
    ORDER_PROPERTY
);
ast_node!(
    /// `variant sequence`
    VariantProperty,
    VARIANT_PROPERTY
);
ast_node!(
    /// `navigateTo view`
    NavigateToProperty,
    NAVIGATE_TO_PROPERTY
);
ast_node!(
    /// `includeAncestors true`
    IncludeAncestorsProperty,
    INCLUDE_ANCESTORS_PROPERTY
);

ast_enum!(
    /// Leaf style properties (`color`, `shape`, `line`, ...).
    StyleLeaf {
        ColorProperty,
        ShapeProperty,
        BorderProperty,
        OpacityProperty,
        IconProperty,
        IconColorProperty,
        MultipleProperty,
        SizeProperty,
        PaddingProperty,
        TextSizeProperty,
        IconSizeProperty,
        IconPositionProperty,
        LineProperty,
        ArrowProperty,
    }
);

ast_enum!(
    /// Properties allowed in element and deployment node bodies.
    ElementProperty { StringProperty, StyleProperty, LinkProperty, IconProperty, MetadataProperty }
);

ast_enum!(
    /// Properties allowed in relation bodies.
    RelationProperty { StringProperty, NavigateToProperty, StyleProperty, LinkProperty, MetadataProperty }
);

ast_enum!(
    /// Properties allowed in view bodies.
    ViewProperty { StringProperty, OrderProperty, LinkProperty, VariantProperty }
);

/// A property with a keyword key and a single value token (`key [:] value [;]`).
pub trait LeafProperty: AstNode {
    /// The keyword token.
    fn key_token(&self) -> Option<SyntaxToken> {
        support::keyword(self.syntax())
    }

    /// The value token (kind depends on the property).
    fn value_token(&self) -> Option<SyntaxToken> {
        self.syntax().children_with_tokens().filter_map(|el| el.into_token()).find(|t| {
            !t.kind().is_trivia() && !t.kind().is_keyword() && !matches!(t.kind(), COLON | SEMICOLON)
        })
    }

    fn colon_token(&self) -> Option<SyntaxToken> {
        support::token(self.syntax(), COLON)
    }

    fn semicolon_token(&self) -> Option<SyntaxToken> {
        support::token(self.syntax(), SEMICOLON)
    }
}

impl LeafProperty for StringProperty {}
impl LeafProperty for SpecStringProperty {}
impl LeafProperty for ColorProperty {} // `value_token()` is the colour name
impl LeafProperty for ShapeProperty {}
impl LeafProperty for BorderProperty {}
impl LeafProperty for OpacityProperty {} // `value_token()` is the `PERCENT`
impl LeafProperty for MultipleProperty {}
impl LeafProperty for IconProperty {}
impl LeafProperty for IconColorProperty {}
impl LeafProperty for IconSizeProperty {}
impl LeafProperty for IconPositionProperty {}
impl LeafProperty for SizeProperty {}
impl LeafProperty for PaddingProperty {}
impl LeafProperty for TextSizeProperty {}
impl LeafProperty for LineProperty {}
impl LeafProperty for ArrowProperty {}
impl LeafProperty for OrderProperty {}
impl LeafProperty for VariantProperty {}
impl LeafProperty for IncludeAncestorsProperty {}
impl LeafProperty for LinkProperty {}

impl StringProperty {
    /// Key text (`title`, `description`, ...).
    pub fn key(&self) -> Option<&'static str> {
        self.key_token().and_then(|t| t.kind().keyword_text())
    }
}

impl LinkProperty {
    pub fn uri_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, URI)
    }

    pub fn title_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }
}

impl MetadataProperty {
    pub fn body(&self) -> Option<MetadataBody> {
        support::child(&self.syntax)
    }
}

impl MetadataBody {
    pub fn attributes(&self) -> AstChildren<MetadataAttribute> {
        support::children(&self.syntax)
    }
}

impl MetadataAttribute {
    pub fn key_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// String or boolean value token; `None` for arrays.
    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::find_token(&self.syntax, |k| matches!(k, STRING | MARKDOWN_STRING | BOOLEAN))
    }

    pub fn array(&self) -> Option<MetadataArray> {
        support::child(&self.syntax)
    }
}

impl MetadataArray {
    pub fn values(&self) -> impl Iterator<Item = SyntaxToken> {
        self.syntax.children_with_tokens().filter_map(|el| el.into_token()).filter(|t| t.kind().is_string())
    }
}

impl StyleProperty {
    pub fn leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }
}

impl NavigateToProperty {
    pub fn view_ref(&self) -> Option<ViewRef> {
        support::child(&self.syntax)
    }
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

ast_node!(
    /// `view name of x { ... }`
    ElementView,
    ELEMENT_VIEW
);
ast_node!(
    /// `dynamic view name { ... }`
    DynamicView,
    DYNAMIC_VIEW
);
ast_node!(
    /// `deployment view name { ... }`
    DeploymentView,
    DEPLOYMENT_VIEW
);
ast_node!(ElementViewBody, ELEMENT_VIEW_BODY);
ast_node!(DynamicViewBody, DYNAMIC_VIEW_BODY);
ast_node!(DeploymentViewBody, DEPLOYMENT_VIEW_BODY);
ast_node!(
    /// Reference to a view by name.
    ViewRef,
    VIEW_REF
);
ast_node!(
    /// `include ...` / `exclude ...`
    ViewRulePredicate,
    VIEW_RULE_PREDICATE
);
ast_node!(
    /// `global predicate name`
    ViewRuleGlobalPredicateRef,
    VIEW_RULE_GLOBAL_PREDICATE_REF
);
ast_node!(
    /// `group 'title' { ... }`
    ViewRuleGroup,
    VIEW_RULE_GROUP
);
ast_node!(
    /// `style a, b { ... }`
    ViewRuleStyle,
    VIEW_RULE_STYLE
);
ast_node!(
    /// `global style name`
    ViewRuleGlobalStyle,
    VIEW_RULE_GLOBAL_STYLE
);
ast_node!(
    /// `autoLayout TopBottom 100 50`
    ViewRuleAutoLayout,
    VIEW_RULE_AUTO_LAYOUT
);
ast_node!(
    /// `rank same { a, b }`
    ViewRuleRank,
    VIEW_RULE_RANK
);
ast_node!(
    /// Comma separated include/exclude expressions.
    Expressions,
    EXPRESSIONS
);
ast_node!(
    /// Comma separated element expressions (style targets, rank).
    FqnExpressions,
    FQN_EXPRESSIONS
);
ast_node!(WildcardExpression, WILDCARD_EXPRESSION);
ast_node!(ElementKindExpression, ELEMENT_KIND_EXPRESSION);
ast_node!(ElementTagExpression, ELEMENT_TAG_EXPRESSION);
ast_node!(FqnRefExpression, FQN_REF_EXPRESSION);
ast_node!(FqnExprWhere, FQN_EXPR_WHERE);
ast_node!(FqnExprWith, FQN_EXPR_WITH);
ast_node!(RelationExprWhere, RELATION_EXPR_WHERE);
ast_node!(RelationExprWith, RELATION_EXPR_WITH);
ast_node!(IncomingRelationExpr, INCOMING_RELATION_EXPR);
ast_node!(OutgoingRelationExpr, OUTGOING_RELATION_EXPR);
ast_node!(InOutRelationExpr, IN_OUT_RELATION_EXPR);
ast_node!(DirectedRelationExpr, DIRECTED_RELATION_EXPR);
ast_node!(WhereBinary, WHERE_BINARY);
ast_node!(WhereNot, WHERE_NOT);
ast_node!(WhereParen, WHERE_PAREN);
ast_node!(WhereKind, WHERE_KIND);
ast_node!(WhereTag, WHERE_TAG);
ast_node!(WhereMetadata, WHERE_METADATA);
ast_node!(CustomElementProperties, CUSTOM_ELEMENT_PROPERTIES);
ast_node!(CustomRelationProperties, CUSTOM_RELATION_PROPERTIES);

ast_enum!(
    /// Any view declaration.
    AnyView { ElementView, DynamicView, DeploymentView }
);

ast_enum!(
    /// Any view body.
    AnyViewBody { ElementViewBody, DynamicViewBody, DeploymentViewBody }
);

ast_enum!(
    /// Rules of a view body (include/exclude, styles, layout, ...).
    ViewRule {
        ViewRulePredicate,
        ViewRuleGlobalPredicateRef,
        ViewRuleGroup,
        ViewRuleStyle,
        ViewRuleGlobalStyle,
        ViewRuleAutoLayout,
        ViewRuleRank,
    }
);

ast_enum!(
    /// Element expressions (`*`, `a.b.*`, `element.kind = x`).
    FqnExpr { WildcardExpression, ElementKindExpression, ElementTagExpression, FqnRefExpression }
);

ast_enum!(
    /// Relation expressions (`-> a`, `a ->`, `a -> b`, `-> a ->`).
    RelationExpr { IncomingRelationExpr, OutgoingRelationExpr, InOutRelationExpr, DirectedRelationExpr }
);

ast_enum!(
    /// One include/exclude expression, including `where`/`with` wrappers.
    Expression {
        WildcardExpression,
        ElementKindExpression,
        ElementTagExpression,
        FqnRefExpression,
        FqnExprWhere,
        FqnExprWith,
        IncomingRelationExpr,
        OutgoingRelationExpr,
        InOutRelationExpr,
        DirectedRelationExpr,
        RelationExprWhere,
        RelationExprWith,
    }
);

ast_enum!(
    /// A `where` condition.
    WhereExpr { WhereBinary, WhereNot, WhereParen, WhereKind, WhereTag, WhereMetadata }
);

impl Views {
    /// The optional `views 'folder'` string.
    pub fn folder_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    /// All views in source order.
    pub fn views(&self) -> AstChildren<AnyView> {
        support::children(&self.syntax)
    }

    pub fn element_views(&self) -> AstChildren<ElementView> {
        support::children(&self.syntax)
    }

    pub fn dynamic_views(&self) -> AstChildren<DynamicView> {
        support::children(&self.syntax)
    }

    pub fn deployment_views(&self) -> AstChildren<DeploymentView> {
        support::children(&self.syntax)
    }

    /// Top-level `style` rules and `global style` references.
    pub fn styles(&self) -> AstChildren<ViewRule> {
        support::children(&self.syntax)
    }
}

impl AnyView {
    /// The view name (element views may be anonymous).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        match self {
            AnyView::ElementView(v) => v.name_token(),
            AnyView::DynamicView(v) => v.name_token(),
            AnyView::DeploymentView(v) => v.name_token(),
        }
    }

    /// The view body, if any.
    pub fn body(&self) -> Option<AnyViewBody> {
        support::child(self.syntax())
    }
}

impl AnyViewBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(self.syntax())
    }

    pub fn properties(&self) -> AstChildren<ViewProperty> {
        support::children(self.syntax())
    }

    pub fn rules(&self) -> AstChildren<ViewRule> {
        support::children(self.syntax())
    }
}

impl ElementView {
    /// The view name, if any.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    /// `extends other`
    pub fn extends(&self) -> Option<ViewRef> {
        support::child(&self.syntax)
    }

    /// `of a.b`
    pub fn of(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<ElementViewBody> {
        support::child(&self.syntax)
    }
}

impl DynamicView {
    /// The view name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<DynamicViewBody> {
        support::child(&self.syntax)
    }
}

impl DeploymentView {
    /// The view name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<DeploymentViewBody> {
        support::child(&self.syntax)
    }
}

impl ElementViewBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<ViewProperty> {
        support::children(&self.syntax)
    }

    /// Rules (`include`, `style`, ...) in source order.
    pub fn rules(&self) -> AstChildren<ViewRule> {
        support::children(&self.syntax)
    }
}

impl DynamicViewBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<ViewProperty> {
        support::children(&self.syntax)
    }

    /// Rules (`include`, `style`, ...) in source order.
    pub fn rules(&self) -> AstChildren<ViewRule> {
        support::children(&self.syntax)
    }

    /// Flow steps in source order.
    pub fn steps(&self) -> AstChildren<StepStatement> {
        support::children(&self.syntax)
    }
}

impl DeploymentViewBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<ViewProperty> {
        support::children(&self.syntax)
    }

    /// Rules (`include`, `style`, ...) in source order.
    pub fn rules(&self) -> AstChildren<ViewRule> {
        support::children(&self.syntax)
    }

    pub fn include_ancestors(&self) -> Option<IncludeAncestorsProperty> {
        support::child(&self.syntax)
    }
}

impl ViewRef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }
}

impl ViewRulePredicate {
    /// True for `include`, false for `exclude`.
    pub fn is_include(&self) -> bool {
        support::token(&self.syntax, INCLUDE_KW).is_some()
    }

    pub fn keyword_token(&self) -> Option<SyntaxToken> {
        support::keyword(&self.syntax)
    }

    /// The comma separated expressions.
    pub fn expressions(&self) -> Option<Expressions> {
        support::child(&self.syntax)
    }
}

impl ViewRuleGlobalPredicateRef {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }
}

impl ViewRuleGroup {
    pub fn title_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }

    pub fn rules(&self) -> AstChildren<ViewRule> {
        support::children(&self.syntax)
    }
}

impl ViewRuleStyle {
    pub fn targets(&self) -> Option<FqnExpressions> {
        support::child(&self.syntax)
    }

    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }

    pub fn notation(&self) -> Option<StringProperty> {
        support::child(&self.syntax)
    }
}

impl ViewRuleGlobalStyle {
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }
}

impl ViewRuleAutoLayout {
    pub fn direction_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn rank_sep_token(&self) -> Option<SyntaxToken> {
        support::nth_token(&self.syntax, NUMBER, 0)
    }

    pub fn node_sep_token(&self) -> Option<SyntaxToken> {
        support::nth_token(&self.syntax, NUMBER, 1)
    }
}

impl ViewRuleRank {
    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn targets(&self) -> Option<FqnExpressions> {
        support::child(&self.syntax)
    }
}

impl Expressions {
    /// The expressions in source order.
    pub fn expressions(&self) -> AstChildren<Expression> {
        support::children(&self.syntax)
    }
}

impl FqnExpressions {
    pub fn expressions(&self) -> AstChildren<FqnExpr> {
        support::children(&self.syntax)
    }
}

impl ElementKindExpression {
    pub fn kind_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn is_negated(&self) -> bool {
        support::token(&self.syntax, NOT_EQUAL).is_some()
    }
}

impl ElementTagExpression {
    pub fn tag_ref(&self) -> Option<TagRef> {
        support::child(&self.syntax)
    }

    pub fn is_negated(&self) -> bool {
        support::token(&self.syntax, NOT_EQUAL).is_some()
    }
}

impl FqnRefExpression {
    pub fn fqn_ref(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    /// `._` or `.*` / `.**` selector.
    pub fn selector_token(&self) -> Option<SyntaxToken> {
        support::find_token(&self.syntax, |k| matches!(k, DOT_UNDERSCORE | DOT_WILDCARD))
    }
}

impl FqnExprWhere {
    pub fn subject(&self) -> Option<FqnExpr> {
        support::child(&self.syntax)
    }

    pub fn condition(&self) -> Option<WhereExpr> {
        support::child(&self.syntax)
    }
}

impl FqnExprWith {
    /// The wrapped expression (`FqnExpr` or `FqnExprWhere`).
    pub fn subject(&self) -> Option<Expression> {
        support::child(&self.syntax)
    }

    pub fn custom(&self) -> Option<CustomElementProperties> {
        support::child(&self.syntax)
    }
}

impl RelationExprWhere {
    pub fn subject(&self) -> Option<RelationExpr> {
        support::child(&self.syntax)
    }

    pub fn condition(&self) -> Option<WhereExpr> {
        support::child(&self.syntax)
    }
}

impl RelationExprWith {
    /// The wrapped expression (`RelationExpr` or `RelationExprWhere`).
    pub fn subject(&self) -> Option<Expression> {
        support::child(&self.syntax)
    }

    pub fn custom(&self) -> Option<CustomRelationProperties> {
        support::child(&self.syntax)
    }
}

impl IncomingRelationExpr {
    pub fn target(&self) -> Option<FqnExpr> {
        support::child(&self.syntax)
    }
}

impl OutgoingRelationExpr {
    pub fn source(&self) -> Option<FqnExpr> {
        support::child(&self.syntax)
    }

    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    pub fn is_bidirectional(&self) -> bool {
        support::token(&self.syntax, BI_ARROW).is_some()
    }
}

impl InOutRelationExpr {
    pub fn incoming(&self) -> Option<IncomingRelationExpr> {
        support::child(&self.syntax)
    }
}

impl DirectedRelationExpr {
    pub fn outgoing(&self) -> Option<OutgoingRelationExpr> {
        support::child(&self.syntax)
    }

    pub fn target(&self) -> Option<FqnExpr> {
        support::child(&self.syntax)
    }
}

impl WhereBinary {
    pub fn left(&self) -> Option<WhereExpr> {
        support::nth_child(&self.syntax, 0)
    }

    pub fn right(&self) -> Option<WhereExpr> {
        support::nth_child(&self.syntax, 1)
    }

    /// `and` / `or` token.
    pub fn operator_token(&self) -> Option<SyntaxToken> {
        support::find_token(&self.syntax, |k| matches!(k, AND_KW | OR_KW))
    }
}

impl WhereNot {
    pub fn operand(&self) -> Option<WhereExpr> {
        support::child(&self.syntax)
    }
}

impl WhereParen {
    pub fn inner(&self) -> Option<WhereExpr> {
        support::child(&self.syntax)
    }
}

/// Common accessors of `WHERE_KIND`, `WHERE_TAG` and `WHERE_METADATA`.
pub trait WhereLeaf: AstNode {
    /// `source` / `target` participant, when present.
    fn participant_token(&self) -> Option<SyntaxToken> {
        support::find_token(self.syntax(), |k| !k.is_trivia()).filter(|t| t.kind() == IDENT)
    }

    /// `=`, `!=` or `is` token.
    fn operator_token(&self) -> Option<SyntaxToken> {
        support::find_token(self.syntax(), |k| matches!(k, EQ | NOT_EQUAL | IS_KW))
    }

    /// True for `!=` and `is not`.
    fn is_negated(&self) -> bool {
        support::token(self.syntax(), NOT_EQUAL).is_some() || support::token(self.syntax(), NOT_KW).is_some()
    }
}

impl WhereLeaf for WhereKind {}
impl WhereLeaf for WhereTag {}
impl WhereLeaf for WhereMetadata {}

impl WhereKind {
    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::tokens(&self.syntax, IDENT).last()
    }
}

impl WhereTag {
    pub fn tag_ref(&self) -> Option<TagRef> {
        support::child(&self.syntax)
    }
}

impl WhereMetadata {
    /// The metadata key (`metadata.key`).
    pub fn key_token(&self) -> Option<SyntaxToken> {
        support::tokens(&self.syntax, IDENT).last()
    }

    pub fn value_token(&self) -> Option<SyntaxToken> {
        support::find_token(&self.syntax, |k| matches!(k, STRING | BOOLEAN))
    }
}

impl CustomElementProperties {
    pub fn navigate_to(&self) -> Option<NavigateToProperty> {
        support::child(&self.syntax)
    }

    pub fn string_properties(&self) -> AstChildren<StringProperty> {
        support::children(&self.syntax)
    }

    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }
}

impl CustomRelationProperties {
    pub fn navigate_to(&self) -> Option<NavigateToProperty> {
        support::child(&self.syntax)
    }

    pub fn string_properties(&self) -> AstChildren<StringProperty> {
        support::children(&self.syntax)
    }

    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }
}

// ---------------------------------------------------------------------------
// Dynamic view steps
// ---------------------------------------------------------------------------

ast_node!(
    /// `a -> b 'title' { ... }`
    Step,
    STEP
);
ast_node!(
    /// `<step> -> c` continuation (left nested).
    StepSeries,
    STEP_SERIES
);
ast_node!(
    /// `opt 'title' { ... }`, `parallel { ... }`, ...
    SubflowStep,
    SUBFLOW_STEP
);
ast_node!(
    /// `alt { when {...} else {...} }`
    AltSteps,
    ALT_STEPS
);
ast_node!(
    /// `try { ... }`
    TryBlock,
    TRY_BLOCK
);
ast_node!(
    /// `<try> catch { ... }`
    CatchBlock,
    CATCH_BLOCK
);
ast_node!(
    /// `<try|catch> finally { ... }`
    FinallyBlock,
    FINALLY_BLOCK
);
ast_node!(
    /// `{ steps }` of `catch` / `finally`.
    StepsBlock,
    STEPS_BLOCK
);

ast_enum!(
    /// A statement inside a dynamic view flow.
    StepStatement { Step, StepSeries, SubflowStep, AltSteps, TryBlock, CatchBlock, FinallyBlock }
);

ast_enum!(
    /// Source of a `STEP_SERIES`.
    StepOrSeries { Step, StepSeries }
);

ast_enum!(
    /// Inner block of a `FINALLY_BLOCK`.
    TryOrCatch { TryBlock, CatchBlock }
);

impl Step {
    pub fn source(&self) -> Option<FqnRef> {
        support::nth_child(&self.syntax, 0)
    }

    pub fn target(&self) -> Option<FqnRef> {
        support::nth_child(&self.syntax, 1)
    }

    pub fn is_backward(&self) -> bool {
        support::token(&self.syntax, BACK_ARROW).is_some()
    }

    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn custom(&self) -> Option<CustomRelationProperties> {
        support::child(&self.syntax)
    }
}

impl StepSeries {
    pub fn source(&self) -> Option<StepOrSeries> {
        support::child(&self.syntax)
    }

    pub fn target(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn custom(&self) -> Option<CustomRelationProperties> {
        support::child(&self.syntax)
    }
}

impl SubflowStep {
    /// `opt`, `par`, `loop`, ... keyword.
    pub fn kind_token(&self) -> Option<SyntaxToken> {
        support::keyword(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn steps(&self) -> AstChildren<StepStatement> {
        support::children(&self.syntax)
    }
}

impl AltSteps {
    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn branches(&self) -> AstChildren<SubflowStep> {
        support::children(&self.syntax)
    }
}

impl TryBlock {
    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn steps(&self) -> AstChildren<StepStatement> {
        support::children(&self.syntax)
    }
}

impl CatchBlock {
    pub fn try_block(&self) -> Option<TryBlock> {
        support::child(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn steps_block(&self) -> Option<StepsBlock> {
        support::child(&self.syntax)
    }
}

impl FinallyBlock {
    pub fn inner(&self) -> Option<TryOrCatch> {
        support::child(&self.syntax)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn steps_block(&self) -> Option<StepsBlock> {
        support::child(&self.syntax)
    }
}

impl StepsBlock {
    pub fn steps(&self) -> AstChildren<StepStatement> {
        support::children(&self.syntax)
    }
}

// ---------------------------------------------------------------------------
// Deployment
// ---------------------------------------------------------------------------

ast_node!(
    /// `kind name 'title' { ... }` inside `deployment`.
    DeploymentNode,
    DEPLOYMENT_NODE
);
ast_node!(DeploymentNodeBody, DEPLOYMENT_NODE_BODY);
ast_node!(
    /// `name = instanceOf a.b`
    DeployedInstance,
    DEPLOYED_INSTANCE
);
ast_node!(DeployedInstanceBody, DEPLOYED_INSTANCE_BODY);
ast_node!(
    /// `a -> b` inside `deployment`.
    DeploymentRelation,
    DEPLOYMENT_RELATION
);
ast_node!(DeploymentRelationBody, DEPLOYMENT_RELATION_BODY);
ast_node!(
    /// `extend a.b { ... }` inside `deployment`.
    ExtendDeployment,
    EXTEND_DEPLOYMENT
);
ast_node!(ExtendDeploymentBody, EXTEND_DEPLOYMENT_BODY);

ast_enum!(
    /// A statement inside `deployment` or a deployment node body.
    DeploymentStatement { DeploymentNode, DeployedInstance, DeploymentRelation, ExtendDeployment }
);

impl Deployment {
    pub fn statements(&self) -> AstChildren<DeploymentStatement> {
        support::children(&self.syntax)
    }

    pub fn nodes(&self) -> AstChildren<DeploymentNode> {
        support::children(&self.syntax)
    }

    pub fn relations(&self) -> AstChildren<DeploymentRelation> {
        support::children(&self.syntax)
    }

    pub fn extends(&self) -> AstChildren<ExtendDeployment> {
        support::children(&self.syntax)
    }
}

impl DeploymentNode {
    /// The node kind identifier.
    pub fn kind_token(&self) -> Option<SyntaxToken> {
        kind_and_name(&self.syntax).0
    }

    /// The node name identifier.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        kind_and_name(&self.syntax).1
    }

    pub fn name(&self) -> Option<String> {
        self.name_token().map(|t| t.text().to_string())
    }

    pub fn is_assignment_form(&self) -> bool {
        support::token(&self.syntax, EQ).is_some()
    }

    /// Inline strings: title, summary.
    pub fn strings(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, STRING)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<DeploymentNodeBody> {
        support::child(&self.syntax)
    }
}

impl DeploymentNodeBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<ElementProperty> {
        support::children(&self.syntax)
    }

    pub fn statements(&self) -> AstChildren<DeploymentStatement> {
        support::children(&self.syntax)
    }

    pub fn nodes(&self) -> AstChildren<DeploymentNode> {
        support::children(&self.syntax)
    }

    pub fn instances(&self) -> AstChildren<DeployedInstance> {
        support::children(&self.syntax)
    }

    pub fn relations(&self) -> AstChildren<DeploymentRelation> {
        support::children(&self.syntax)
    }
}

impl DeployedInstance {
    /// The explicit instance name (`name = instanceOf ...`).
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// The referenced model element.
    pub fn target(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    pub fn strings(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, STRING)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<DeployedInstanceBody> {
        support::child(&self.syntax)
    }
}

impl DeployedInstanceBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<ElementProperty> {
        support::children(&self.syntax)
    }

    pub fn relations(&self) -> AstChildren<DeploymentRelation> {
        support::children(&self.syntax)
    }
}

impl DeploymentRelation {
    /// The source reference; `None` when omitted inside a node body.
    pub fn source(&self) -> Option<FqnRef> {
        source_and_target(&self.syntax).0
    }

    /// The target reference.
    pub fn target(&self) -> Option<FqnRef> {
        source_and_target(&self.syntax).1
    }

    /// The relationship kind identifier from `.kind` or `-[kind]->`.
    pub fn kind_ref(&self) -> Option<SyntaxToken> {
        relation_kind_token(&self.syntax)
    }

    pub fn strings(&self) -> impl Iterator<Item = SyntaxToken> {
        support::tokens(&self.syntax, STRING)
    }

    pub fn title(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, STRING)
    }

    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    /// The `{ ... }` body, if any.
    pub fn body(&self) -> Option<DeploymentRelationBody> {
        support::child(&self.syntax)
    }
}

impl DeploymentRelationBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn properties(&self) -> AstChildren<RelationProperty> {
        support::children(&self.syntax)
    }
}

impl ExtendDeployment {
    pub fn target(&self) -> Option<FqnRef> {
        support::child(&self.syntax)
    }

    pub fn body(&self) -> Option<ExtendDeploymentBody> {
        support::child(&self.syntax)
    }
}

impl ExtendDeploymentBody {
    pub fn tags(&self) -> Option<Tags> {
        support::child(&self.syntax)
    }

    pub fn links(&self) -> AstChildren<LinkProperty> {
        support::children(&self.syntax)
    }

    pub fn metadata(&self) -> Option<MetadataProperty> {
        support::child(&self.syntax)
    }

    pub fn statements(&self) -> AstChildren<DeploymentStatement> {
        support::children(&self.syntax)
    }
}

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

ast_node!(
    /// `predicateGroup name { ... }`
    GlobalPredicateGroup,
    GLOBAL_PREDICATE_GROUP
);
ast_node!(
    /// `dynamicPredicateGroup name { ... }`
    GlobalDynamicPredicateGroup,
    GLOBAL_DYNAMIC_PREDICATE_GROUP
);
ast_node!(
    /// `style name targets { ... }`
    GlobalStyle,
    GLOBAL_STYLE
);
ast_node!(
    /// `styleGroup name { ... }`
    GlobalStyleGroup,
    GLOBAL_STYLE_GROUP
);

impl Globals {
    pub fn predicate_groups(&self) -> AstChildren<GlobalPredicateGroup> {
        support::children(&self.syntax)
    }

    pub fn dynamic_predicate_groups(&self) -> AstChildren<GlobalDynamicPredicateGroup> {
        support::children(&self.syntax)
    }

    pub fn styles(&self) -> AstChildren<GlobalStyle> {
        support::children(&self.syntax)
    }

    pub fn style_groups(&self) -> AstChildren<GlobalStyleGroup> {
        support::children(&self.syntax)
    }
}

impl GlobalPredicateGroup {
    /// The group name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// The `include` / `exclude` rules.
    pub fn predicates(&self) -> AstChildren<ViewRulePredicate> {
        support::children(&self.syntax)
    }
}

impl GlobalDynamicPredicateGroup {
    /// The group name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn predicates(&self) -> AstChildren<ViewRulePredicate> {
        support::children(&self.syntax)
    }
}

impl GlobalStyle {
    /// The style name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    /// The styled element expressions.
    pub fn targets(&self) -> Option<FqnExpressions> {
        support::child(&self.syntax)
    }

    pub fn style_leafs(&self) -> AstChildren<StyleLeaf> {
        support::children(&self.syntax)
    }

    pub fn notation(&self) -> Option<StringProperty> {
        support::child(&self.syntax)
    }
}

impl GlobalStyleGroup {
    /// The group name.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        support::token(&self.syntax, IDENT)
    }

    pub fn styles(&self) -> AstChildren<ViewRuleStyle> {
        support::children(&self.syntax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn root(text: &str) -> Root {
        let parse = parse(text);
        assert!(parse.ok(), "{:?}", parse.errors());
        Root::cast(parse.syntax()).expect("root")
    }

    #[test]
    fn element_forms() {
        let root = root("model {\n  system a 'A'\n  b = system 'B' 'summary'\n}\n");
        let model = root.models().next().unwrap();
        let elements: Vec<Element> = model.elements().collect();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].kind_token().unwrap().text(), "system");
        assert_eq!(elements[0].name().as_deref(), Some("a"));
        assert_eq!(elements[0].title().unwrap().text(), "'A'");
        assert!(!elements[0].is_assignment_form());
        assert_eq!(elements[1].kind_token().unwrap().text(), "system");
        assert_eq!(elements[1].name().as_deref(), Some("b"));
        assert!(elements[1].is_assignment_form());
        assert_eq!(elements[1].strings().count(), 2);
    }

    #[test]
    fn relation_accessors() {
        let root = root(
            "model {\n  a.b -[uses]-> c 'title' #t1 #t2 { technology 'http' }\n  x .calls y\n  q = system { -> z }\n}\n",
        );
        let model = root.models().next().unwrap();
        let relations: Vec<Relation> = model.relations().collect();
        assert_eq!(relations.len(), 2);
        let r = &relations[0];
        assert_eq!(r.source().unwrap().text(), "a.b");
        assert_eq!(r.target().unwrap().text(), "c");
        assert_eq!(r.kind_ref().unwrap().text(), "uses");
        assert_eq!(r.title().unwrap().text(), "'title'");
        let tags: Vec<String> = r.tags().unwrap().tag_refs().filter_map(|t| t.name()).collect();
        assert_eq!(tags, ["t1", "t2"]);
        let props: Vec<RelationProperty> = r.body().unwrap().properties().collect();
        assert!(matches!(props.as_slice(), [RelationProperty::StringProperty(_)]));
        assert_eq!(relations[1].kind_ref().unwrap().text(), "calls");

        let nested = model.elements().next().unwrap().body().unwrap().relations().next().unwrap();
        assert!(nested.source().is_none());
        assert_eq!(nested.target().unwrap().text(), "z");
    }

    #[test]
    fn views_and_rules() {
        let root = root(
            "views {\n  view idx of a.b {\n    title 'T'\n    include *, -> x with { color red }\n    exclude y\n    style * { color red }\n  }\n  dynamic view d {\n    a -> b -> c\n  }\n}\n",
        );
        let views = root.views().next().unwrap();
        let all: Vec<AnyView> = views.views().collect();
        assert_eq!(all.len(), 2);
        let AnyView::ElementView(view) = &all[0] else { panic!("expected element view") };
        assert_eq!(view.name().as_deref(), Some("idx"));
        assert_eq!(view.of().unwrap().text(), "a.b");
        let body = view.body().unwrap();
        assert_eq!(body.properties().count(), 1);
        let rules: Vec<ViewRule> = body.rules().collect();
        assert_eq!(rules.len(), 3);
        let ViewRule::ViewRulePredicate(include) = &rules[0] else { panic!("expected predicate") };
        assert!(include.is_include());
        let exprs: Vec<Expression> = include.expressions().unwrap().expressions().collect();
        assert!(matches!(
            exprs.as_slice(),
            [Expression::WildcardExpression(_), Expression::RelationExprWith(_)]
        ));
        let ViewRule::ViewRulePredicate(exclude) = &rules[1] else { panic!("expected predicate") };
        assert!(!exclude.is_include());

        let AnyView::DynamicView(dynamic) = &all[1] else { panic!("expected dynamic view") };
        let steps: Vec<StepStatement> = dynamic.body().unwrap().steps().collect();
        let [StepStatement::StepSeries(series)] = steps.as_slice() else { panic!("expected series") };
        assert_eq!(series.target().unwrap().text(), "c");
        let Some(StepOrSeries::Step(first)) = series.source() else { panic!("expected step") };
        assert_eq!(first.source().unwrap().text(), "a");
        assert_eq!(first.target().unwrap().text(), "b");
    }

    #[test]
    fn deployment_and_globals() {
        let root = root(
            "deployment {\n  node n1 {\n    i = instanceOf a.b\n    instanceOf c\n    n1.i -> n1.c\n  }\n}\nglobal {\n  predicateGroup pg { include * }\n  style gs * { color red }\n}\n",
        );
        let deployment = root.deployments().next().unwrap();
        let node = deployment.nodes().next().unwrap();
        assert_eq!(node.kind_token().unwrap().text(), "node");
        assert_eq!(node.name().as_deref(), Some("n1"));
        let body = node.body().unwrap();
        let instances: Vec<DeployedInstance> = body.instances().collect();
        assert_eq!(instances[0].name_token().unwrap().text(), "i");
        assert_eq!(instances[0].target().unwrap().text(), "a.b");
        assert!(instances[1].name_token().is_none());
        let relation = body.relations().next().unwrap();
        assert_eq!(relation.source().unwrap().text(), "n1.i");

        let globals = root.globals().next().unwrap();
        assert_eq!(globals.predicate_groups().next().unwrap().name_token().unwrap().text(), "pg");
        let style = globals.styles().next().unwrap();
        assert_eq!(style.name_token().unwrap().text(), "gs");
        let StyleLeaf::ColorProperty(color) = style.style_leafs().next().unwrap() else {
            panic!("expected color")
        };
        assert_eq!(color.value_token().unwrap().text(), "red");
    }

    #[test]
    fn specification_and_colors() {
        let root = root(
            "specification {\n  element system { style { opacity 20% } }\n  tag t { color #fff }\n  relationship uses\n  color custom rgba(1, 2, 3, 0.5)\n  deploymentNode node\n}\n",
        );
        let spec = root.specifications().next().unwrap();
        assert_eq!(spec.element_kinds().next().unwrap().name().as_deref(), Some("system"));
        assert_eq!(spec.tags().next().unwrap().name().as_deref(), Some("t"));
        assert_eq!(spec.relationship_kinds().next().unwrap().name().as_deref(), Some("uses"));
        assert_eq!(spec.deployment_node_kinds().next().unwrap().name().as_deref(), Some("node"));
        let color = spec.colors().next().unwrap();
        assert_eq!(color.name().as_deref(), Some("custom"));
        let Some(ColorLiteral::RgbaColor(rgba)) = color.literal() else { panic!("expected rgba") };
        assert_eq!(rgba.components().count(), 4);
        let Some(ColorLiteral::HexColor(hex)) = spec.tags().next().unwrap().color_literal() else {
            panic!("expected hex")
        };
        assert_eq!(hex.value_token().unwrap().text(), "fff");
        let style = spec.element_kinds().next().unwrap().style().unwrap();
        let StyleLeaf::OpacityProperty(opacity) = style.leafs().next().unwrap() else {
            panic!("expected opacity")
        };
        assert_eq!(opacity.value_token().unwrap().text(), "20%");
    }

    #[test]
    fn where_and_try_blocks() {
        let root = root(
            "views {\n  view {\n    include * -> * where source.kind is not a and (tag = #t or not metadata.k is 'v')\n  }\n  dynamic view d {\n    try { a -> b } catch 'c' { b -> c } finally { c -> d }\n  }\n}\n",
        );
        let views = root.views().next().unwrap();
        let view = views.element_views().next().unwrap();
        let ViewRule::ViewRulePredicate(include) = view.body().unwrap().rules().next().unwrap() else {
            panic!("expected predicate")
        };
        let Expression::RelationExprWhere(where_expr) =
            include.expressions().unwrap().expressions().next().unwrap()
        else {
            panic!("expected where")
        };
        assert!(matches!(where_expr.subject(), Some(RelationExpr::DirectedRelationExpr(_))));
        let Some(WhereExpr::WhereBinary(and)) = where_expr.condition() else { panic!("expected binary") };
        assert_eq!(and.operator_token().unwrap().text(), "and");
        let Some(WhereExpr::WhereKind(kind)) = and.left() else { panic!("expected kind") };
        assert_eq!(kind.participant_token().unwrap().text(), "source");
        assert!(kind.is_negated());
        assert_eq!(kind.value_token().unwrap().text(), "a");
        let Some(WhereExpr::WhereParen(paren)) = and.right() else { panic!("expected paren") };
        let Some(WhereExpr::WhereBinary(or)) = paren.inner() else { panic!("expected binary") };
        let Some(WhereExpr::WhereNot(not)) = or.right() else { panic!("expected not") };
        let Some(WhereExpr::WhereMetadata(meta)) = not.operand() else { panic!("expected metadata") };
        assert!(meta.participant_token().is_none());
        assert_eq!(meta.key_token().unwrap().text(), "k");
        assert_eq!(meta.value_token().unwrap().text(), "'v'");

        let dynamic = views.dynamic_views().next().unwrap();
        let steps: Vec<StepStatement> = dynamic.body().unwrap().steps().collect();
        let [StepStatement::FinallyBlock(finally)] = steps.as_slice() else { panic!("expected finally") };
        assert_eq!(finally.steps_block().unwrap().steps().count(), 1);
        let Some(TryOrCatch::CatchBlock(catch)) = finally.inner() else { panic!("expected catch") };
        assert_eq!(catch.title().unwrap().text(), "'c'");
        assert_eq!(catch.try_block().unwrap().steps().count(), 1);
    }
}

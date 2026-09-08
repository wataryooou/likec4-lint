//! Syntax kinds for tokens and nodes of the LikeC4 DSL.
//!
//! The enum is flat (rowan style). Node kinds mirror the Langium AST type names
//! of the reference grammar so that formatter rules can be ported one-to-one.

#![allow(non_camel_case_types)]

/// All token and node kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // ----- trivia -----
    WHITESPACE,
    NEWLINE,
    LINE_COMMENT,
    BLOCK_COMMENT,

    // ----- words -----
    IDENT,
    BOOLEAN,
    LIB_ICON,
    URI,

    // ----- literals -----
    STRING,
    MARKDOWN_STRING,
    NUMBER,
    FLOAT,
    PERCENT,
    HEX,

    // ----- symbols -----
    L_CURLY,
    R_CURLY,
    L_PAREN,
    R_PAREN,
    L_BRACK,
    R_BRACK,
    COMMA,
    COLON,
    SEMICOLON,
    STAR,
    HASH,
    DOT,
    STICKY_DOT,
    DOT_UNDERSCORE,
    DOT_WILDCARD,
    EQ,
    NOT_EQUAL,
    ARROW,
    BACK_ARROW,
    BI_ARROW,
    ARROW_L_BRACK,
    R_BRACK_ARROW,
    R_BRACK_BI_ARROW,

    /// Unlexable input.
    ERROR,

    // ----- keywords (remapped from IDENT by the parser) -----
    LIKEC4LIB_KW,
    ICONS_KW,
    SPECIFICATION_KW,
    ELEMENT_KW,
    DEPLOYMENT_NODE_KW,
    TAG_KW,
    COLOR_KW,
    RELATIONSHIP_KW,
    TITLE_KW,
    DESCRIPTION_KW,
    TECHNOLOGY_KW,
    NOTATION_KW,
    SUMMARY_KW,
    NOTES_KW,
    IMPORT_KW,
    FROM_KW,
    MODEL_KW,
    EXTEND_KW,
    STYLE_KW,
    METADATA_KW,
    VIEWS_KW,
    VIEW_KW,
    EXTENDS_KW,
    OF_KW,
    DYNAMIC_KW,
    VARIANT_KW,
    ORDER_KW,
    RANK_KW,
    GROUP_KW,
    INCLUDE_KW,
    EXCLUDE_KW,
    GLOBAL_KW,
    PREDICATE_KW,
    AUTO_LAYOUT_KW,
    NAVIGATE_TO_KW,
    WITH_KW,
    WHERE_KW,
    AND_KW,
    OR_KW,
    NOT_KW,
    IS_KW,
    KIND_KW,
    OPT_KW,
    PAR_KW,
    PARALLEL_KW,
    LOOP_KW,
    WHEN_KW,
    IF_KW,
    ELSE_KW,
    BREAK_KW,
    TRY_KW,
    CATCH_KW,
    FINALLY_KW,
    ALT_KW,
    DEPLOYMENT_KW,
    INSTANCE_OF_KW,
    INCLUDE_ANCESTORS_KW,
    PREDICATE_GROUP_KW,
    DYNAMIC_PREDICATE_GROUP_KW,
    STYLE_GROUP_KW,
    LINK_KW,
    OPACITY_KW,
    MULTIPLE_KW,
    ICON_KW,
    ICON_COLOR_KW,
    ICON_SIZE_KW,
    ICON_POSITION_KW,
    SHAPE_KW,
    BORDER_KW,
    SIZE_KW,
    PADDING_KW,
    TEXT_SIZE_KW,
    LINE_KW,
    HEAD_KW,
    TAIL_KW,

    // ----- nodes: top level -----
    ROOT,
    LIKEC4LIB,
    LIB_ICON_DECL,
    IMPORTS,
    IMPORTED,
    SPECIFICATION,
    MODEL,
    VIEWS,
    DEPLOYMENT,
    GLOBALS,

    // ----- nodes: specification -----
    SPEC_ELEMENT_KIND,
    SPEC_DEPLOYMENT_NODE_KIND,
    SPEC_RELATIONSHIP_KIND,
    SPEC_TAG,
    SPEC_COLOR,
    SPEC_STRING_PROPERTY,
    RGBA_COLOR,
    HEX_COLOR,

    // ----- nodes: model -----
    ELEMENT,
    ELEMENT_BODY,
    RELATION,
    RELATION_BODY,
    EXTEND_ELEMENT,
    EXTEND_ELEMENT_BODY,
    EXTEND_RELATION,
    EXTEND_RELATION_BODY,
    TAGS,
    TAG_REF,
    FQN_REF,
    RELATION_KIND_DOT_REF,

    // ----- nodes: properties -----
    STRING_PROPERTY,
    LINK_PROPERTY,
    ICON_PROPERTY,
    METADATA_PROPERTY,
    METADATA_BODY,
    METADATA_ATTRIBUTE,
    METADATA_ARRAY,
    STYLE_PROPERTY,
    COLOR_PROPERTY,
    SHAPE_PROPERTY,
    BORDER_PROPERTY,
    OPACITY_PROPERTY,
    MULTIPLE_PROPERTY,
    ICON_COLOR_PROPERTY,
    ICON_SIZE_PROPERTY,
    ICON_POSITION_PROPERTY,
    SIZE_PROPERTY,
    PADDING_PROPERTY,
    TEXT_SIZE_PROPERTY,
    LINE_PROPERTY,
    ARROW_PROPERTY,
    ORDER_PROPERTY,
    VARIANT_PROPERTY,
    NAVIGATE_TO_PROPERTY,
    INCLUDE_ANCESTORS_PROPERTY,

    // ----- nodes: views -----
    ELEMENT_VIEW,
    DYNAMIC_VIEW,
    DEPLOYMENT_VIEW,
    ELEMENT_VIEW_BODY,
    DYNAMIC_VIEW_BODY,
    DEPLOYMENT_VIEW_BODY,
    VIEW_REF,
    VIEW_RULE_PREDICATE,
    VIEW_RULE_GLOBAL_PREDICATE_REF,
    VIEW_RULE_GROUP,
    VIEW_RULE_STYLE,
    VIEW_RULE_GLOBAL_STYLE,
    VIEW_RULE_AUTO_LAYOUT,
    VIEW_RULE_RANK,
    EXPRESSIONS,
    FQN_EXPRESSIONS,
    WILDCARD_EXPRESSION,
    ELEMENT_KIND_EXPRESSION,
    ELEMENT_TAG_EXPRESSION,
    FQN_REF_EXPRESSION,
    FQN_EXPR_WHERE,
    FQN_EXPR_WITH,
    RELATION_EXPR_WHERE,
    RELATION_EXPR_WITH,
    INCOMING_RELATION_EXPR,
    OUTGOING_RELATION_EXPR,
    IN_OUT_RELATION_EXPR,
    DIRECTED_RELATION_EXPR,
    WHERE_BINARY,
    WHERE_NOT,
    WHERE_PAREN,
    WHERE_KIND,
    WHERE_TAG,
    WHERE_METADATA,
    CUSTOM_ELEMENT_PROPERTIES,
    CUSTOM_RELATION_PROPERTIES,

    // ----- nodes: dynamic view steps -----
    STEP,
    STEP_SERIES,
    SUBFLOW_STEP,
    ALT_STEPS,
    TRY_BLOCK,
    CATCH_BLOCK,
    FINALLY_BLOCK,
    STEPS_BLOCK,

    // ----- nodes: deployment -----
    DEPLOYMENT_NODE,
    DEPLOYMENT_NODE_BODY,
    DEPLOYED_INSTANCE,
    DEPLOYED_INSTANCE_BODY,
    DEPLOYMENT_RELATION,
    DEPLOYMENT_RELATION_BODY,
    EXTEND_DEPLOYMENT,
    EXTEND_DEPLOYMENT_BODY,

    // ----- nodes: globals -----
    GLOBAL_PREDICATE_GROUP,
    GLOBAL_DYNAMIC_PREDICATE_GROUP,
    GLOBAL_STYLE,
    GLOBAL_STYLE_GROUP,

    /// Recovery node wrapping skipped tokens.
    ERROR_NODE,

    /// Marker: must stay last.
    #[doc(hidden)]
    __LAST,
}

use SyntaxKind::*;

impl SyntaxKind {
    /// Whitespace, newlines and comments.
    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | NEWLINE | LINE_COMMENT | BLOCK_COMMENT)
    }

    /// Whitespace and newlines (not part of the Langium CST at all).
    pub fn is_whitespace(self) -> bool {
        matches!(self, WHITESPACE | NEWLINE)
    }

    /// Line or block comment (Langium "hidden" CST leaf).
    pub fn is_comment(self) -> bool {
        matches!(self, LINE_COMMENT | BLOCK_COMMENT)
    }

    /// True for keyword kinds produced by remapping an identifier.
    pub fn is_keyword(self) -> bool {
        (self as u16) >= (LIKEC4LIB_KW as u16) && (self as u16) <= (TAIL_KW as u16)
    }

    /// True for token kinds (as opposed to node kinds).
    pub fn is_token(self) -> bool {
        (self as u16) < (ROOT as u16)
    }

    /// True for `STRING` and `MARKDOWN_STRING`.
    pub fn is_string(self) -> bool {
        matches!(self, STRING | MARKDOWN_STRING)
    }

    /// True for the string literal keywords used as the `key` of leaf properties.
    pub fn from_keyword(text: &str) -> Option<SyntaxKind> {
        Some(match text {
            "likec4lib" => LIKEC4LIB_KW,
            "icons" => ICONS_KW,
            "specification" => SPECIFICATION_KW,
            "element" => ELEMENT_KW,
            "deploymentNode" => DEPLOYMENT_NODE_KW,
            "tag" => TAG_KW,
            "color" => COLOR_KW,
            "relationship" => RELATIONSHIP_KW,
            "title" => TITLE_KW,
            "description" => DESCRIPTION_KW,
            "technology" => TECHNOLOGY_KW,
            "notation" => NOTATION_KW,
            "summary" => SUMMARY_KW,
            "notes" => NOTES_KW,
            "import" => IMPORT_KW,
            "from" => FROM_KW,
            "model" => MODEL_KW,
            "extend" => EXTEND_KW,
            "style" => STYLE_KW,
            "metadata" => METADATA_KW,
            "views" => VIEWS_KW,
            "view" => VIEW_KW,
            "extends" => EXTENDS_KW,
            "of" => OF_KW,
            "dynamic" => DYNAMIC_KW,
            "variant" => VARIANT_KW,
            "order" => ORDER_KW,
            "rank" => RANK_KW,
            "group" => GROUP_KW,
            "include" => INCLUDE_KW,
            "exclude" => EXCLUDE_KW,
            "global" => GLOBAL_KW,
            "predicate" => PREDICATE_KW,
            "autoLayout" => AUTO_LAYOUT_KW,
            "navigateTo" => NAVIGATE_TO_KW,
            "with" => WITH_KW,
            "where" => WHERE_KW,
            "and" => AND_KW,
            "or" => OR_KW,
            "not" => NOT_KW,
            "is" => IS_KW,
            "kind" => KIND_KW,
            "opt" => OPT_KW,
            "par" => PAR_KW,
            "parallel" => PARALLEL_KW,
            "loop" => LOOP_KW,
            "when" => WHEN_KW,
            "if" => IF_KW,
            "else" => ELSE_KW,
            "break" => BREAK_KW,
            "try" => TRY_KW,
            "catch" => CATCH_KW,
            "finally" => FINALLY_KW,
            "alt" => ALT_KW,
            "deployment" => DEPLOYMENT_KW,
            "instanceOf" => INSTANCE_OF_KW,
            "includeAncestors" => INCLUDE_ANCESTORS_KW,
            "predicateGroup" => PREDICATE_GROUP_KW,
            "dynamicPredicateGroup" => DYNAMIC_PREDICATE_GROUP_KW,
            "styleGroup" => STYLE_GROUP_KW,
            "link" => LINK_KW,
            "opacity" => OPACITY_KW,
            "multiple" => MULTIPLE_KW,
            "icon" => ICON_KW,
            "iconColor" => ICON_COLOR_KW,
            "iconSize" => ICON_SIZE_KW,
            "iconPosition" => ICON_POSITION_KW,
            "shape" => SHAPE_KW,
            "border" => BORDER_KW,
            "size" => SIZE_KW,
            "padding" => PADDING_KW,
            "textSize" => TEXT_SIZE_KW,
            "line" => LINE_KW,
            "head" => HEAD_KW,
            "tail" => TAIL_KW,
            _ => return None,
        })
    }

    /// Source text of a keyword kind.
    pub fn keyword_text(self) -> Option<&'static str> {
        Some(match self {
            LIKEC4LIB_KW => "likec4lib",
            ICONS_KW => "icons",
            SPECIFICATION_KW => "specification",
            ELEMENT_KW => "element",
            DEPLOYMENT_NODE_KW => "deploymentNode",
            TAG_KW => "tag",
            COLOR_KW => "color",
            RELATIONSHIP_KW => "relationship",
            TITLE_KW => "title",
            DESCRIPTION_KW => "description",
            TECHNOLOGY_KW => "technology",
            NOTATION_KW => "notation",
            SUMMARY_KW => "summary",
            NOTES_KW => "notes",
            IMPORT_KW => "import",
            FROM_KW => "from",
            MODEL_KW => "model",
            EXTEND_KW => "extend",
            STYLE_KW => "style",
            METADATA_KW => "metadata",
            VIEWS_KW => "views",
            VIEW_KW => "view",
            EXTENDS_KW => "extends",
            OF_KW => "of",
            DYNAMIC_KW => "dynamic",
            VARIANT_KW => "variant",
            ORDER_KW => "order",
            RANK_KW => "rank",
            GROUP_KW => "group",
            INCLUDE_KW => "include",
            EXCLUDE_KW => "exclude",
            GLOBAL_KW => "global",
            PREDICATE_KW => "predicate",
            AUTO_LAYOUT_KW => "autoLayout",
            NAVIGATE_TO_KW => "navigateTo",
            WITH_KW => "with",
            WHERE_KW => "where",
            AND_KW => "and",
            OR_KW => "or",
            NOT_KW => "not",
            IS_KW => "is",
            KIND_KW => "kind",
            OPT_KW => "opt",
            PAR_KW => "par",
            PARALLEL_KW => "parallel",
            LOOP_KW => "loop",
            WHEN_KW => "when",
            IF_KW => "if",
            ELSE_KW => "else",
            BREAK_KW => "break",
            TRY_KW => "try",
            CATCH_KW => "catch",
            FINALLY_KW => "finally",
            ALT_KW => "alt",
            DEPLOYMENT_KW => "deployment",
            INSTANCE_OF_KW => "instanceOf",
            INCLUDE_ANCESTORS_KW => "includeAncestors",
            PREDICATE_GROUP_KW => "predicateGroup",
            DYNAMIC_PREDICATE_GROUP_KW => "dynamicPredicateGroup",
            STYLE_GROUP_KW => "styleGroup",
            LINK_KW => "link",
            OPACITY_KW => "opacity",
            MULTIPLE_KW => "multiple",
            ICON_KW => "icon",
            ICON_COLOR_KW => "iconColor",
            ICON_SIZE_KW => "iconSize",
            ICON_POSITION_KW => "iconPosition",
            SHAPE_KW => "shape",
            BORDER_KW => "border",
            SIZE_KW => "size",
            PADDING_KW => "padding",
            TEXT_SIZE_KW => "textSize",
            LINE_KW => "line",
            HEAD_KW => "head",
            TAIL_KW => "tail",
            _ => return None,
        })
    }
}

/// Words that are keyword tokens in the Langium grammar and are *not* listed as
/// alternatives of the `Id` rule. They cannot be used as identifiers.
pub const HARD_RESERVED_WORDS: &[&str] = &[
    "and",
    "autoLayout",
    "border",
    "color",
    "deploymentNode",
    "description",
    "dynamic",
    "dynamicPredicateGroup",
    "exclude",
    "extend",
    "extends",
    "from",
    "global",
    "head",
    "icon",
    "iconColor",
    "iconPosition",
    "iconSize",
    "icons",
    "import",
    "include",
    "includeAncestors",
    "instanceOf",
    "is",
    "kind",
    "likec4lib",
    "line",
    "link",
    "metadata",
    "multiple",
    "navigateTo",
    "not",
    "notation",
    "notes",
    "of",
    "opacity",
    "or",
    "order",
    "padding",
    "predicate",
    "predicateGroup",
    "rank",
    "rgb",
    "rgba",
    "shape",
    "size",
    "specification",
    "style",
    "styleGroup",
    "summary",
    "tag",
    "tail",
    "technology",
    "textSize",
    "title",
    "variant",
    "view",
    "views",
    "where",
    "with",
    "TopBottom",
    "LeftRight",
    "BottomTop",
    "RightLeft",
    "true",
    "false",
];

/// Every alphanumeric keyword literal of the grammar (keyword tokens in Langium).
/// `IdTerminal` rejects all of these; `Id` rejects only [`HARD_RESERVED_WORDS`].
#[rustfmt::skip]
pub const ALL_KEYWORDS: &[&str] = &[
    // structural
    "likec4lib", "icons", "specification", "element", "deploymentNode", "tag", "color",
    "relationship", "title", "description", "technology", "notation", "summary", "notes",
    "import", "from", "model", "extend", "style", "metadata", "views", "view", "extends", "of",
    "dynamic", "variant", "order", "rank", "group", "include", "exclude", "global", "predicate",
    "autoLayout", "navigateTo", "with", "where", "and", "or", "not", "is", "kind", "opt", "par",
    "parallel", "loop", "when", "if", "else", "break", "try", "catch", "finally", "alt",
    "deployment", "instanceOf", "includeAncestors", "predicateGroup", "dynamicPredicateGroup",
    "styleGroup", "link", "opacity", "multiple", "icon", "iconColor", "iconSize", "iconPosition",
    "shape", "border", "size", "padding", "textSize", "line", "head", "tail", "rgb", "rgba",
    "node", "instance",
    // enumerated values
    "TopBottom", "LeftRight", "BottomTop", "RightLeft", "same", "min", "max", "source", "sink",
    "target", "none", "left", "right", "top", "bottom", "xs", "sm", "md", "lg", "xl", "xsmall",
    "small", "medium", "large", "xlarge", "solid", "dashed", "dotted", "normal", "onormal", "dot",
    "odot", "diamond", "odiamond", "crow", "open", "vee", "diagram", "sequence", "primary",
    "secondary", "muted", "slate", "blue", "indigo", "sky", "red", "gray", "green", "amber",
    "rectangle", "component", "person", "browser", "mobile", "cylinder", "storage", "queue",
    "bucket", "document",
];

/// Words that parse as identifiers but are flagged by the linter (`reserved-name`).
pub const SOFT_RESERVED_WORDS: &[&str] =
    &["element", "model", "group", "node", "deployment", "instance", "relationship"];

/// Whether `text` may be used where the grammar expects `Id`.
pub fn is_valid_id(text: &str) -> bool {
    !HARD_RESERVED_WORDS.contains(&text)
}

/// Whether `text` may be used where the grammar expects `IdTerminal`.
pub fn is_valid_id_terminal(text: &str) -> bool {
    !ALL_KEYWORDS.contains(&text) && text != "true" && text != "false"
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl From<rowan::SyntaxKind> for SyntaxKind {
    fn from(raw: rowan::SyntaxKind) -> Self {
        assert!(raw.0 < (__LAST as u16));
        // SAFETY: the enum is `repr(u16)`, contiguous, and the value is bounds-checked above.
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }
}

/// Rowan language marker type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LikeC4Language {}

impl rowan::Language for LikeC4Language {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        raw.into()
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<LikeC4Language>;
pub type SyntaxToken = rowan::SyntaxToken<LikeC4Language>;
pub type SyntaxElement = rowan::SyntaxElement<LikeC4Language>;
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<LikeC4Language>;
pub type SyntaxElementChildren = rowan::SyntaxElementChildren<LikeC4Language>;

/// Convenience macro: `T![model]`, `T!['{']`, `T![->]`.
#[macro_export]
macro_rules! T {
    ['{'] => { $crate::SyntaxKind::L_CURLY };
    ['}'] => { $crate::SyntaxKind::R_CURLY };
    ['('] => { $crate::SyntaxKind::L_PAREN };
    [')'] => { $crate::SyntaxKind::R_PAREN };
    ['['] => { $crate::SyntaxKind::L_BRACK };
    [']'] => { $crate::SyntaxKind::R_BRACK };
    [,] => { $crate::SyntaxKind::COMMA };
    [:] => { $crate::SyntaxKind::COLON };
    [;] => { $crate::SyntaxKind::SEMICOLON };
    [*] => { $crate::SyntaxKind::STAR };
    [#] => { $crate::SyntaxKind::HASH };
    [.] => { $crate::SyntaxKind::DOT };
    [=] => { $crate::SyntaxKind::EQ };
    [!=] => { $crate::SyntaxKind::NOT_EQUAL };
    [->] => { $crate::SyntaxKind::ARROW };
    [<-] => { $crate::SyntaxKind::BACK_ARROW };
    [<->] => { $crate::SyntaxKind::BI_ARROW };
    [-'['] => { $crate::SyntaxKind::ARROW_L_BRACK };
    [']'->] => { $crate::SyntaxKind::R_BRACK_ARROW };
    [']'<->] => { $crate::SyntaxKind::R_BRACK_BI_ARROW };
    [ident] => { $crate::SyntaxKind::IDENT };
    [string] => { $crate::SyntaxKind::STRING };
    [markdown] => { $crate::SyntaxKind::MARKDOWN_STRING };
    [number] => { $crate::SyntaxKind::NUMBER };
    [$kw:ident] => { $crate::SyntaxKind::$kw };
}

//! Grammar rules of the LikeC4 DSL (hand-written recursive descent).
//!
//! Node shapes follow `docs/DESIGN.md`; the Langium grammar (`like-c4.langium`) is the
//! reference for the accepted language. Keywords are contextual: the lexer produces
//! `IDENT` for every word and the rules below remap the ones they consume as keywords.
//!
//! Every rule that produces a node consumes at least one token when it reports success,
//! so the statement loops in [`block_body`] always make progress. Unexpected tokens are
//! wrapped into `ERROR_NODE`s by [`recover_in_block`] / [`recover_top_level`].

use super::{describe, CompletedMarker, Parser};
use crate::kind::SyntaxKind::{self, *};

// ---------------------------------------------------------------------------
// Keyword tables
// ---------------------------------------------------------------------------

const ROOT_KEYWORDS: &[&str] =
    &["import", "specification", "model", "views", "global", "deployment", "likec4lib"];
const SUBFLOW_KINDS: &[&str] = &["opt", "par", "parallel", "loop", "when", "if", "else", "break"];
const LAYOUT_DIRECTIONS: &[&str] = &["TopBottom", "LeftRight", "BottomTop", "RightLeft"];
const RANK_VALUES: &[&str] = &["same", "min", "max", "source", "sink"];
const SHAPES: &[&str] = &[
    "rectangle",
    "component",
    "person",
    "browser",
    "mobile",
    "cylinder",
    "storage",
    "queue",
    "bucket",
    "document",
];
const LINE_OPTIONS: &[&str] = &["solid", "dashed", "dotted"];
const BORDER_STYLES: &[&str] = &["solid", "dashed", "dotted", "none"];
const ARROW_TYPES: &[&str] =
    &["none", "normal", "onormal", "dot", "odot", "diamond", "odiamond", "crow", "open", "vee"];
const SIZE_VALUES: &[&str] = &["xs", "sm", "md", "lg", "xl", "xsmall", "small", "medium", "large", "xlarge"];
const ICON_POSITIONS: &[&str] = &["left", "right", "top", "bottom"];
const VARIANTS: &[&str] = &["diagram", "sequence"];
const PARTICIPANTS: &[&str] = &["source", "target"];

/// Words accepted by the `CustomColorId` rule in addition to `IdTerminal`.
const CUSTOM_COLOR_EXTRA: &[&str] = &["source", "target", "element", "model"];

// String-property keys are context dependent in the Langium grammar.
const ELEMENT_STRING_KEYS: &[&str] = &["title", "description", "technology", "summary"];
const SPEC_ELEMENT_STRING_KEYS: &[&str] = &["title", "description", "technology", "notation", "summary"];
const SPEC_RELATIONSHIP_STRING_KEYS: &[&str] = &["title", "description", "technology", "notation"];
const RELATION_STRING_KEYS: &[&str] = &["title", "technology", "description"];
const VIEW_STRING_KEYS: &[&str] = &["title", "description"];
const CUSTOM_ELEMENT_STRING_KEYS: &[&str] =
    &["title", "description", "technology", "summary", "notation", "notes"];
const CUSTOM_RELATION_STRING_KEYS: &[&str] = &["title", "technology", "description", "notation", "notes"];
const NOTATION_KEYS: &[&str] = &["notation"];

/// Which relation connectors a rule accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Connectors {
    /// `.kind`, `-[kind]->`, `-[kind]<->`, `<->`, `->`
    Model,
    /// `.kind`, `-[kind]->`, `->`
    Deployment,
    /// `<->`, `.kind`, `->`, `-[kind]->`
    Expression,
    /// `<-`, `.kind`, `->`, `-[kind]->`
    StepFirst,
    /// `.kind`, `->`, `-[kind]->`
    StepNext,
}

/// Which leaf style properties a `style { }` block accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StyleContext {
    Element,
    Relation,
}

/// Where a deployment statement appears (decides which statements are allowed and
/// whether a relation source is mandatory).
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeployCtx {
    Root,
    ExtendBody,
    NodeBody,
    InstanceBody,
}

// ---------------------------------------------------------------------------
// Entry point and recovery
// ---------------------------------------------------------------------------

pub(crate) fn root(p: &mut Parser<'_>) {
    let m = p.start();
    while !p.at_eof() {
        p.step();
        if !top_level_statement(p) {
            recover_top_level(p);
        }
    }
    m.complete(p, ROOT);
}

fn top_level_statement(p: &mut Parser<'_>) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    match p.current_text() {
        "import" => imports(p),
        "specification" => specification(p),
        "model" => model(p),
        "views" => views(p),
        "global" => globals(p),
        "deployment" => deployment(p),
        "likec4lib" => likec4lib(p),
        _ => return false,
    }
    true
}

fn at_root_keyword(p: &Parser<'_>) -> bool {
    p.at(IDENT) && ROOT_KEYWORDS.contains(&p.current_text())
}

/// Wrap tokens into an `ERROR_NODE` until a top-level keyword starts a new line
/// (or directly precedes a `{`).
fn recover_top_level(p: &mut Parser<'_>) {
    report_unexpected(p);
    let m = p.start();
    let mut first = true;
    while !p.at_eof() {
        if !first && at_root_keyword(p) && (p.at_line_start() || p.nth_at(1, L_CURLY)) {
            break;
        }
        p.bump_any();
        first = false;
    }
    m.complete(p, ERROR_NODE);
}

/// Tokens that may start a statement inside some block. Used only to decide where
/// error recovery resumes, so it does not need to be context accurate.
fn at_statement_start(p: &Parser<'_>) -> bool {
    matches!(p.current(), IDENT | HASH | ARROW | BACK_ARROW | BI_ARROW | ARROW_L_BRACK | DOT | STAR)
}

/// Wrap tokens into an `ERROR_NODE` until the block's closing `}` (nested braces are
/// balanced) or a token that starts a new line and may start a statement.
fn recover_in_block(p: &mut Parser<'_>) {
    report_unexpected(p);
    let m = p.start();
    let mut depth = 0u32;
    let mut first = true;
    while !p.at_eof() {
        if depth == 0 {
            if p.at(R_CURLY) {
                break;
            }
            if !first && p.at_line_start() && at_statement_start(p) {
                break;
            }
        }
        match p.current() {
            L_CURLY => depth += 1,
            R_CURLY => depth -= 1,
            _ => {}
        }
        p.bump_any();
        first = false;
    }
    m.complete(p, ERROR_NODE);
}

/// Report the current token as unexpected. Lexer `ERROR` tokens already carry a
/// diagnostic from the lexer, so they are wrapped silently.
fn report_unexpected(p: &mut Parser<'_>) {
    let message = match p.current() {
        ERROR => return,
        IDENT if SyntaxKind::from_keyword(p.current_text()).is_some() => {
            format!("'{}' is not allowed here", p.current_text())
        }
        IDENT => format!("unexpected '{}'", p.current_text()),
        kind => format!("unexpected {}", describe(kind)),
    };
    p.error(message);
}

// ---------------------------------------------------------------------------
// Block helpers
// ---------------------------------------------------------------------------

/// Parse `{ stmt* }`. Returns `false` (after reporting) when there is no `{`.
fn block(p: &mut Parser<'_>, stmt: impl FnMut(&mut Parser<'_>) -> bool) -> bool {
    if !p.expect(L_CURLY) {
        return false;
    }
    block_body(p, stmt);
    true
}

/// After `{` has been consumed: parse statements until `}` (with recovery), then `}`.
fn block_body(p: &mut Parser<'_>, mut stmt: impl FnMut(&mut Parser<'_>) -> bool) {
    while !p.at(R_CURLY) && !p.at_eof() {
        p.step();
        if !stmt(p) {
            recover_in_block(p);
        }
    }
    p.expect(R_CURLY);
}

/// Consume up to `max` string literals.
fn strings(p: &mut Parser<'_>, max: usize) {
    for _ in 0..max {
        if !p.eat(STRING) {
            break;
        }
    }
}

fn expect_string_or_markdown(p: &mut Parser<'_>) -> bool {
    if p.eat(STRING) || p.eat(MARKDOWN_STRING) {
        return true;
    }
    p.error("expected string");
    false
}

/// Consume an identifier that must be one of `values`. A wrong identifier on the same
/// line is consumed (with an error) so that recovery does not report it twice.
fn expect_enum(p: &mut Parser<'_>, values: &[&str], what: &str) -> bool {
    if p.at(IDENT) && values.contains(&p.current_text()) {
        p.bump(IDENT);
        return true;
    }
    p.error(format!("expected {what} (one of: {})", values.join(", ")));
    if p.at(IDENT) && !p.at_line_start() {
        p.bump(IDENT);
    }
    false
}

fn is_custom_color_id(text: &str) -> bool {
    crate::kind::is_valid_id_terminal(text)
        || SHAPES.contains(&text)
        || ARROW_TYPES.contains(&text)
        || LINE_OPTIONS.contains(&text)
        || CUSTOM_COLOR_EXTRA.contains(&text)
}

// ---------------------------------------------------------------------------
// Imports and lib
// ---------------------------------------------------------------------------

fn imports(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("import");
    if p.eat(L_CURLY) {
        imported(p);
        p.expect(R_CURLY);
    } else {
        imported(p);
    }
    p.expect_kw("from");
    p.expect(STRING);
    p.eat(SEMICOLON);
    m.complete(p, IMPORTS);
}

fn imported(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect_id();
    while p.at(COMMA) {
        p.bump(COMMA);
        p.expect_id();
    }
    p.eat(SEMICOLON);
    m.complete(p, IMPORTED);
}

fn likec4lib(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("likec4lib");
    if p.expect(L_CURLY) {
        if p.expect_kw("icons") && p.expect(L_CURLY) {
            if !p.at(LIB_ICON) {
                p.error("expected at least one icon");
            }
            block_body(p, lib_icon_decl);
        }
        block_body(p, |_| false);
    }
    m.complete(p, LIKEC4LIB);
}

fn lib_icon_decl(p: &mut Parser<'_>) -> bool {
    if !p.at(LIB_ICON) {
        return false;
    }
    let m = p.start();
    p.bump(LIB_ICON);
    m.complete(p, LIB_ICON_DECL);
    true
}

// ---------------------------------------------------------------------------
// Specification
// ---------------------------------------------------------------------------

fn specification(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("specification");
    block(p, spec_statement);
    m.complete(p, SPECIFICATION);
}

fn spec_statement(p: &mut Parser<'_>) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    match p.current_text() {
        "element" => spec_element_kind(p, "element", SPEC_ELEMENT_KIND),
        "deploymentNode" => spec_element_kind(p, "deploymentNode", SPEC_DEPLOYMENT_NODE_KIND),
        "tag" => spec_tag(p),
        "relationship" => spec_relationship_kind(p),
        "color" => spec_color(p),
        _ => return false,
    }
    true
}

fn spec_element_kind(p: &mut Parser<'_>, kw: &str, kind: SyntaxKind) {
    let m = p.start();
    p.bump_kw(kw);
    p.expect_id();
    if p.at(L_CURLY) {
        p.bump(L_CURLY);
        opt_tags(p);
        block_body(p, |p| {
            string_property(p, SPEC_ELEMENT_STRING_KEYS, SPEC_STRING_PROPERTY)
                || link_property(p)
                || style_property(p, StyleContext::Element)
        });
    }
    m.complete(p, kind);
}

fn spec_tag(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("tag");
    p.expect_id();
    if p.at(L_CURLY) {
        p.bump(L_CURLY);
        if p.at_kw("color") {
            p.bump_kw("color");
            color_ref_or_literal(p);
        }
        block_body(p, |_| false);
    }
    m.complete(p, SPEC_TAG);
}

fn spec_relationship_kind(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("relationship");
    p.expect_id();
    if p.at(L_CURLY) {
        p.bump(L_CURLY);
        opt_tags(p);
        block_body(p, |p| {
            link_property(p)
                || relationship_style_leaf(p)
                || string_property(p, SPEC_RELATIONSHIP_STRING_KEYS, SPEC_STRING_PROPERTY)
                || multiple_property(p)
        });
    }
    m.complete(p, SPEC_RELATIONSHIP_KIND);
}

fn spec_color(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("color");
    if p.at(IDENT) {
        if !is_custom_color_id(p.current_text()) {
            p.error(format!("'{}' cannot be used as a custom color name", p.current_text()));
        }
        p.bump(IDENT);
    } else {
        p.error("expected color name");
    }
    color_literal(p);
    m.complete(p, SPEC_COLOR);
}

/// `ColorRef | ColorLiteral`: a theme/custom colour name, `#hex` or `rgb(...)`.
fn color_ref_or_literal(p: &mut Parser<'_>) {
    if p.at(HASH) || p.at_kw("rgb") || p.at_kw("rgba") {
        color_literal(p);
    } else if p.at(IDENT) {
        p.bump(IDENT);
    } else {
        p.error("expected color");
    }
}

fn color_literal(p: &mut Parser<'_>) {
    if p.at(HASH) {
        hex_color(p);
    } else if p.at_kw("rgb") || p.at_kw("rgba") {
        rgba_color(p);
    } else {
        p.error("expected color literal ('#hex' or 'rgb(...)')");
    }
}

fn hex_color(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(HASH);
    match p.current() {
        HEX | NUMBER => p.bump_any(),
        IDENT if p.at_id_terminal() => p.bump(IDENT),
        _ => p.error("expected hex color"),
    }
    m.complete(p, HEX_COLOR);
}

fn rgba_color(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(IDENT);
    if p.expect(L_PAREN) {
        for _ in 0..3 {
            p.expect(NUMBER);
            p.eat(COMMA);
        }
        if matches!(p.current(), FLOAT | NUMBER | PERCENT) {
            p.bump_any();
        }
        p.expect(R_PAREN);
    }
    m.complete(p, RGBA_COLOR);
}

// ---------------------------------------------------------------------------
// Tags and references
// ---------------------------------------------------------------------------

fn opt_tags(p: &mut Parser<'_>) {
    if p.at(HASH) {
        tags(p);
    }
}

/// `TAG_REF+ (',' TAG_REF*)* ';'?`
fn tags(p: &mut Parser<'_>) {
    let m = p.start();
    tag_ref(p);
    loop {
        if p.at(HASH) {
            tag_ref(p);
        } else if p.at(COMMA) {
            p.bump(COMMA);
        } else {
            break;
        }
    }
    p.eat(SEMICOLON);
    m.complete(p, TAGS);
}

fn tag_ref(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(HASH);
    p.expect_id();
    m.complete(p, TAG_REF);
}

fn expect_tag_ref(p: &mut Parser<'_>) {
    if p.at(HASH) {
        tag_ref(p);
    } else {
        p.error("expected tag ('#name')");
    }
}

/// `IDENT ('.' IDENT)*` — the current token must be a valid `Id`.
fn fqn_ref(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect_id();
    while p.at(STICKY_DOT) {
        p.bump(STICKY_DOT);
        p.expect_id();
    }
    m.complete(p, FQN_REF);
}

/// Parse an `FQN_REF` or report an error (without consuming anything).
fn expect_fqn_ref(p: &mut Parser<'_>) -> bool {
    if p.at_id() {
        fqn_ref(p);
        true
    } else {
        p.expect_id()
    }
}

fn view_ref(p: &mut Parser<'_>) {
    if p.at_id() {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, VIEW_REF);
    } else {
        p.expect_id();
    }
}

fn relation_kind_dot_ref(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(DOT);
    p.expect_id();
    m.complete(p, RELATION_KIND_DOT_REF);
}

fn at_connector(p: &Parser<'_>, set: Connectors) -> bool {
    match p.current() {
        ARROW | ARROW_L_BRACK | DOT => true,
        BI_ARROW => matches!(set, Connectors::Model | Connectors::Expression),
        BACK_ARROW => set == Connectors::StepFirst,
        _ => false,
    }
}

/// Consume a relation connector. Returns `false` (after reporting) when absent.
fn connector(p: &mut Parser<'_>, set: Connectors) -> bool {
    if !at_connector(p, set) {
        p.error("expected relation connector ('->', '.kind' or '-[kind]->')");
        return false;
    }
    match p.current() {
        DOT => relation_kind_dot_ref(p),
        ARROW_L_BRACK => {
            p.bump(ARROW_L_BRACK);
            p.expect_id();
            let closed = p.eat(R_BRACK_ARROW) || (set == Connectors::Model && p.eat(R_BRACK_BI_ARROW));
            if !closed {
                p.error("expected ']->'");
            }
        }
        _ => p.bump_any(),
    }
    true
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

fn model(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("model");
    block(p, |p| model_statement(p, true));
    m.complete(p, MODEL);
}

/// Element, relation or extend statement. `source_required` is true directly inside `model`.
fn model_statement(p: &mut Parser<'_>, source_required: bool) -> bool {
    if p.at_kw("extend") {
        extend_element_or_relation(p);
        return true;
    }
    if p.at_id() {
        match p.nth(1) {
            IDENT | EQ => element(p),
            STICKY_DOT | DOT | ARROW | BI_ARROW | ARROW_L_BRACK => relation(p, source_required),
            _ => return false,
        }
        return true;
    }
    if !source_required && at_connector(p, Connectors::Model) {
        relation(p, false);
        return true;
    }
    false
}

/// `kind name | name = kind`, up to four strings, optional body.
fn element(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect_id();
    p.eat(EQ);
    p.expect_id();
    strings(p, 4);
    if p.at(L_CURLY) {
        element_body(p);
    }
    m.complete(p, ELEMENT);
}

fn element_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while element_property(p) {}
    block_body(p, |p| model_statement(p, false));
    m.complete(p, ELEMENT_BODY);
}

fn element_property(p: &mut Parser<'_>) -> bool {
    string_property(p, ELEMENT_STRING_KEYS, STRING_PROPERTY)
        || style_property(p, StyleContext::Element)
        || link_property(p)
        || icon_property(p)
        || metadata_property(p)
}

fn relation(p: &mut Parser<'_>, source_required: bool) {
    let m = p.start();
    if p.at(IDENT) {
        fqn_ref(p);
    } else if source_required {
        p.error("expected relation source");
    }
    if !connector(p, Connectors::Model) {
        m.complete(p, RELATION);
        return;
    }
    expect_fqn_ref(p);
    strings(p, 3);
    opt_tags(p);
    if p.at(L_CURLY) {
        relation_body(p, RELATION_BODY);
    }
    m.complete(p, RELATION);
}

fn relation_body(p: &mut Parser<'_>, kind: SyntaxKind) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    block_body(p, relation_property);
    m.complete(p, kind);
}

fn relation_property(p: &mut Parser<'_>) -> bool {
    string_property(p, RELATION_STRING_KEYS, STRING_PROPERTY)
        || navigate_to_property(p)
        || style_property(p, StyleContext::Relation)
        || link_property(p)
        || metadata_property(p)
}

fn extend_element_or_relation(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("extend");
    expect_fqn_ref(p);
    if at_connector(p, Connectors::Model) {
        connector(p, Connectors::Model);
        expect_fqn_ref(p);
        p.eat(STRING);
        if p.at(L_CURLY) {
            extend_body(p, EXTEND_RELATION_BODY, false);
        } else {
            p.error("expected '{'");
        }
        m.complete(p, EXTEND_RELATION);
    } else {
        if p.at(L_CURLY) {
            extend_body(p, EXTEND_ELEMENT_BODY, true);
        } else {
            p.error("expected '{'");
        }
        m.complete(p, EXTEND_ELEMENT);
    }
}

/// `{ tags? (link | metadata)* nested* }` — `nested` only for extend element.
fn extend_body(p: &mut Parser<'_>, kind: SyntaxKind, nested: bool) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while link_property(p) || metadata_property(p) {}
    if nested {
        block_body(p, |p| model_statement(p, false));
    } else {
        block_body(p, |_| false);
    }
    m.complete(p, kind);
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// `key ':'? (STRING | MARKDOWN_STRING) ';'?` for one of `keys`.
fn string_property(p: &mut Parser<'_>, keys: &[&str], kind: SyntaxKind) -> bool {
    if !p.at(IDENT) || !keys.contains(&p.current_text()) {
        return false;
    }
    let m = p.start();
    let kw = p.current_text();
    p.bump_kw(kw);
    p.eat(COLON);
    expect_string_or_markdown(p);
    p.eat(SEMICOLON);
    m.complete(p, kind);
    true
}

/// `kw ':'? value ';'?`
fn leaf_property(
    p: &mut Parser<'_>,
    kw: &str,
    kind: SyntaxKind,
    value: impl FnOnce(&mut Parser<'_>),
) -> bool {
    if !p.at_kw(kw) {
        return false;
    }
    let m = p.start();
    p.bump_kw(kw);
    p.eat(COLON);
    value(p);
    p.eat(SEMICOLON);
    m.complete(p, kind);
    true
}

fn expect_color_ref(p: &mut Parser<'_>) {
    if !p.eat(IDENT) {
        p.error("expected color");
    }
}

fn color_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "color", COLOR_PROPERTY, expect_color_ref)
}

fn shape_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "shape", SHAPE_PROPERTY, |p| {
        expect_enum(p, SHAPES, "shape");
    })
}

fn border_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "border", BORDER_PROPERTY, |p| {
        expect_enum(p, BORDER_STYLES, "border style");
    })
}

fn opacity_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "opacity", OPACITY_PROPERTY, |p| {
        p.expect(PERCENT);
    })
}

fn multiple_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "multiple", MULTIPLE_PROPERTY, |p| {
        p.expect(BOOLEAN);
    })
}

fn icon_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "icon", ICON_PROPERTY, |p| {
        if p.at(LIB_ICON) || p.at(URI) || p.at_kw("none") {
            p.bump_any();
        } else {
            p.error("expected icon (library icon, uri or 'none')");
        }
    })
}

fn icon_color_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "iconColor", ICON_COLOR_PROPERTY, expect_color_ref)
}

fn icon_size_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "iconSize", ICON_SIZE_PROPERTY, |p| {
        expect_enum(p, SIZE_VALUES, "size");
    })
}

fn icon_position_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "iconPosition", ICON_POSITION_PROPERTY, |p| {
        expect_enum(p, ICON_POSITIONS, "icon position");
    })
}

fn size_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "size", SIZE_PROPERTY, |p| {
        expect_enum(p, SIZE_VALUES, "size");
    })
}

fn padding_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "padding", PADDING_PROPERTY, |p| {
        expect_enum(p, SIZE_VALUES, "size");
    })
}

fn text_size_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "textSize", TEXT_SIZE_PROPERTY, |p| {
        expect_enum(p, SIZE_VALUES, "size");
    })
}

fn line_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "line", LINE_PROPERTY, |p| {
        expect_enum(p, LINE_OPTIONS, "line style");
    })
}

fn arrow_property(p: &mut Parser<'_>) -> bool {
    let kw = if p.at_kw("head") {
        "head"
    } else if p.at_kw("tail") {
        "tail"
    } else {
        return false;
    };
    leaf_property(p, kw, ARROW_PROPERTY, |p| {
        expect_enum(p, ARROW_TYPES, "arrow type");
    })
}

fn include_ancestors_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "includeAncestors", INCLUDE_ANCESTORS_PROPERTY, |p| {
        p.expect(BOOLEAN);
    })
}

/// `link ':'? URI STRING? ';'?`
fn link_property(p: &mut Parser<'_>) -> bool {
    leaf_property(p, "link", LINK_PROPERTY, |p| {
        if p.expect(URI) {
            p.eat(STRING);
        }
    })
}

/// `order NUMBER ';'?`
fn order_property(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("order") {
        return false;
    }
    let m = p.start();
    p.bump_kw("order");
    p.expect(NUMBER);
    p.eat(SEMICOLON);
    m.complete(p, ORDER_PROPERTY);
    true
}

/// `variant (diagram | sequence)`
fn variant_property(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("variant") {
        return false;
    }
    let m = p.start();
    p.bump_kw("variant");
    expect_enum(p, VARIANTS, "variant");
    m.complete(p, VARIANT_PROPERTY);
    true
}

/// `navigateTo VIEW_REF`
fn navigate_to_property(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("navigateTo") {
        return false;
    }
    let m = p.start();
    p.bump_kw("navigateTo");
    view_ref(p);
    m.complete(p, NAVIGATE_TO_PROPERTY);
    true
}

/// `metadata { (key ':'? value ';'?)* }`
fn metadata_property(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("metadata") {
        return false;
    }
    let m = p.start();
    p.bump_kw("metadata");
    if p.at(L_CURLY) {
        metadata_body(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, METADATA_PROPERTY);
    true
}

fn metadata_body(p: &mut Parser<'_>) {
    let m = p.start();
    block(p, metadata_attribute);
    m.complete(p, METADATA_BODY);
}

fn metadata_attribute(p: &mut Parser<'_>) -> bool {
    if !p.at_id() {
        return false;
    }
    let m = p.start();
    p.bump(IDENT);
    p.eat(COLON);
    match p.current() {
        STRING | MARKDOWN_STRING | BOOLEAN => p.bump_any(),
        L_BRACK => metadata_array(p),
        _ => p.error("expected metadata value (string, boolean or array)"),
    }
    p.eat(SEMICOLON);
    m.complete(p, METADATA_ATTRIBUTE);
    true
}

fn metadata_array(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_BRACK);
    expect_string_or_markdown(p);
    while p.eat(COMMA) {
        expect_string_or_markdown(p);
    }
    p.expect(R_BRACK);
    m.complete(p, METADATA_ARRAY);
}

/// `style { leaf* }`
fn style_property(p: &mut Parser<'_>, ctx: StyleContext) -> bool {
    if !p.at_kw("style") {
        return false;
    }
    let m = p.start();
    p.bump_kw("style");
    block(p, |p| match ctx {
        StyleContext::Element => element_style_leaf(p),
        StyleContext::Relation => relationship_style_leaf(p),
    });
    m.complete(p, STYLE_PROPERTY);
    true
}

/// Langium `StyleProperty` (element styles).
fn element_style_leaf(p: &mut Parser<'_>) -> bool {
    color_property(p)
        || shape_property(p)
        || border_property(p)
        || opacity_property(p)
        || icon_property(p)
        || icon_color_property(p)
        || multiple_property(p)
        || size_property(p)
        || padding_property(p)
        || text_size_property(p)
        || icon_size_property(p)
        || icon_position_property(p)
}

/// Langium `RelationshipStyleProperty`.
fn relationship_style_leaf(p: &mut Parser<'_>) -> bool {
    color_property(p) || line_property(p) || arrow_property(p)
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

fn views(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("views");
    p.eat(STRING);
    block(p, views_statement);
    m.complete(p, VIEWS);
}

fn views_statement(p: &mut Parser<'_>) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    match p.current_text() {
        "view" => element_view(p),
        "dynamic" => dynamic_view(p),
        "deployment" => deployment_view(p),
        "style" => view_rule_style(p),
        "global" if p.nth_at_kw(1, "style") => view_rule_global_style(p),
        _ => return false,
    }
    true
}

fn element_view(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("view");
    if p.at_id() {
        p.bump(IDENT);
    }
    if p.at_kw("extends") {
        p.bump_kw("extends");
        view_ref(p);
    } else if p.at_kw("of") {
        p.bump_kw("of");
        expect_fqn_ref(p);
    }
    if p.at(L_CURLY) {
        element_view_body(p);
    }
    m.complete(p, ELEMENT_VIEW);
}

fn element_view_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while view_property(p) {}
    block_body(p, view_rule);
    m.complete(p, ELEMENT_VIEW_BODY);
}

fn view_property(p: &mut Parser<'_>) -> bool {
    string_property(p, VIEW_STRING_KEYS, STRING_PROPERTY) || order_property(p) || link_property(p)
}

fn view_rule(p: &mut Parser<'_>) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    match p.current_text() {
        "include" | "exclude" => view_rule_predicate(p),
        "global" => return global_ref_rule(p),
        "group" => view_rule_group(p),
        "style" => view_rule_style(p),
        "autoLayout" => view_rule_auto_layout(p),
        "rank" => view_rule_rank(p),
        _ => return false,
    }
    true
}

/// `global predicate NAME | global style NAME`
fn global_ref_rule(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("global") {
        return false;
    }
    if p.nth_at_kw(1, "predicate") {
        view_rule_global_predicate_ref(p);
        true
    } else if p.nth_at_kw(1, "style") {
        view_rule_global_style(p);
        true
    } else {
        false
    }
}

fn view_rule_predicate(p: &mut Parser<'_>) {
    let m = p.start();
    if p.at_kw("include") {
        p.bump_kw("include");
    } else {
        p.bump_kw("exclude");
    }
    expressions(p);
    m.complete(p, VIEW_RULE_PREDICATE);
}

/// `include` only (dynamic views and dynamic predicate groups).
fn include_predicate(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("include") {
        return false;
    }
    view_rule_predicate(p);
    true
}

fn include_or_exclude_predicate(p: &mut Parser<'_>) -> bool {
    if !p.at_kw("include") && !p.at_kw("exclude") {
        return false;
    }
    view_rule_predicate(p);
    true
}

fn view_rule_global_predicate_ref(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("global");
    p.bump_kw("predicate");
    p.expect_id_terminal();
    m.complete(p, VIEW_RULE_GLOBAL_PREDICATE_REF);
}

fn view_rule_global_style(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("global");
    p.bump_kw("style");
    p.expect_id_terminal();
    m.complete(p, VIEW_RULE_GLOBAL_STYLE);
}

fn view_rule_group(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("group");
    p.eat(STRING);
    if p.expect(L_CURLY) {
        while color_property(p) || border_property(p) || opacity_property(p) {}
        block_body(p, |p| {
            if p.at_kw("group") {
                view_rule_group(p);
                true
            } else {
                include_or_exclude_predicate(p) || global_predicate_ref_rule(p)
            }
        });
    }
    m.complete(p, VIEW_RULE_GROUP);
}

fn global_predicate_ref_rule(p: &mut Parser<'_>) -> bool {
    if p.at_kw("global") && p.nth_at_kw(1, "predicate") {
        view_rule_global_predicate_ref(p);
        true
    } else {
        false
    }
}

/// `style FQN_EXPRESSIONS { (style leaf | notation)* }`
fn view_rule_style(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("style");
    fqn_expressions(p);
    block(p, style_rule_body_statement);
    m.complete(p, VIEW_RULE_STYLE);
}

fn style_rule_body_statement(p: &mut Parser<'_>) -> bool {
    element_style_leaf(p) || string_property(p, NOTATION_KEYS, STRING_PROPERTY)
}

fn view_rule_auto_layout(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("autoLayout");
    expect_enum(p, LAYOUT_DIRECTIONS, "layout direction");
    if p.eat(NUMBER) {
        p.eat(NUMBER);
    }
    m.complete(p, VIEW_RULE_AUTO_LAYOUT);
}

/// `rank VALUE? { FQN_EXPRESSIONS }`
fn view_rule_rank(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("rank");
    if p.at(IDENT) {
        expect_enum(p, RANK_VALUES, "rank value");
    }
    if p.expect(L_CURLY) {
        fqn_expressions(p);
        block_body(p, |_| false);
    }
    m.complete(p, VIEW_RULE_RANK);
}

fn dynamic_view(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("dynamic");
    p.expect_kw("view");
    p.expect_id();
    if p.at(L_CURLY) {
        dynamic_view_body(p);
    }
    m.complete(p, DYNAMIC_VIEW);
}

fn dynamic_view_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while variant_property(p) || view_property(p) {}
    block_body(p, dynamic_view_statement);
    m.complete(p, DYNAMIC_VIEW_BODY);
}

fn dynamic_view_statement(p: &mut Parser<'_>) -> bool {
    if p.at(IDENT) {
        match p.current_text() {
            "include" => {
                view_rule_predicate(p);
                return true;
            }
            "global" => return global_ref_rule(p),
            "style" => {
                view_rule_style(p);
                return true;
            }
            "autoLayout" => {
                view_rule_auto_layout(p);
                return true;
            }
            _ => {}
        }
    }
    step_statement(p)
}

fn deployment_view(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("deployment");
    p.expect_kw("view");
    p.expect_id();
    if p.at(L_CURLY) {
        deployment_view_body(p);
    }
    m.complete(p, DEPLOYMENT_VIEW);
}

fn deployment_view_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while view_property(p) {}
    block_body(p, |p| {
        if p.at_kw("style") {
            view_rule_style(p);
            true
        } else if p.at_kw("autoLayout") {
            view_rule_auto_layout(p);
            true
        } else {
            include_or_exclude_predicate(p) || include_ancestors_property(p)
        }
    });
    m.complete(p, DEPLOYMENT_VIEW_BODY);
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

fn at_fqn_expr_start(p: &Parser<'_>) -> bool {
    p.at(STAR) || p.at_id()
}

fn at_expression_start(p: &Parser<'_>) -> bool {
    p.at(ARROW) || at_fqn_expr_start(p)
}

/// `EXPRESSION (',' EXPRESSION?)*`
fn expressions(p: &mut Parser<'_>) {
    let m = p.start();
    if at_expression_start(p) {
        expression(p);
    } else {
        p.error("expected expression");
    }
    while p.at(COMMA) {
        p.bump(COMMA);
        if at_expression_start(p) {
            expression(p);
        }
    }
    m.complete(p, EXPRESSIONS);
}

/// `FQN_EXPR (',' FQN_EXPR?)*`
fn fqn_expressions(p: &mut Parser<'_>) {
    let m = p.start();
    if at_fqn_expr_start(p) {
        fqn_expr(p);
    } else {
        p.error("expected element expression");
    }
    while p.at(COMMA) {
        p.bump(COMMA);
        if at_fqn_expr_start(p) {
            fqn_expr(p);
        }
    }
    m.complete(p, FQN_EXPRESSIONS);
}

/// Wildcard, `element.kind = x`, `element.tag = #t` or `fqn(._ | .*)?`.
/// The current token must satisfy [`at_fqn_expr_start`].
fn fqn_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    if p.at(STAR) {
        p.bump(STAR);
        return m.complete(p, WILDCARD_EXPRESSION);
    }
    if p.at_kw("element") && p.nth_at(1, STICKY_DOT) && (p.nth_at_kw(2, "kind") || p.nth_at_kw(2, "tag")) {
        p.bump_kw("element");
        p.bump(STICKY_DOT);
        let is_kind = p.at_kw("kind");
        if is_kind {
            p.bump_kw("kind");
        } else {
            p.bump_kw("tag");
        }
        if !p.eat(EQ) && !p.eat(NOT_EQUAL) {
            p.error("expected '=' or '!='");
        }
        if is_kind {
            p.expect_id();
            return m.complete(p, ELEMENT_KIND_EXPRESSION);
        }
        expect_tag_ref(p);
        return m.complete(p, ELEMENT_TAG_EXPRESSION);
    }
    fqn_ref(p);
    if !p.eat(DOT_UNDERSCORE) {
        p.eat(DOT_WILDCARD);
    }
    m.complete(p, FQN_REF_EXPRESSION)
}

/// One include/exclude expression. The current token must satisfy [`at_expression_start`].
fn expression(p: &mut Parser<'_>) {
    if p.at(ARROW) {
        let m = p.start();
        p.bump(ARROW);
        if at_fqn_expr_start(p) {
            fqn_expr(p);
        } else {
            p.error("expected element expression");
        }
        let mut cm = m.complete(p, INCOMING_RELATION_EXPR);
        if p.at(ARROW) {
            let m = cm.precede(p);
            p.bump(ARROW);
            cm = m.complete(p, IN_OUT_RELATION_EXPR);
        }
        relation_expr_tail(p, cm);
        return;
    }
    let mut cm = fqn_expr(p);
    if at_connector(p, Connectors::Expression) {
        let m = cm.precede(p);
        connector(p, Connectors::Expression);
        cm = m.complete(p, OUTGOING_RELATION_EXPR);
        if at_fqn_expr_start(p) {
            let m = cm.precede(p);
            fqn_expr(p);
            cm = m.complete(p, DIRECTED_RELATION_EXPR);
        }
        relation_expr_tail(p, cm);
    } else {
        fqn_expr_tail(p, cm);
    }
}

/// Optional `where` and `with` clauses of a relation expression.
fn relation_expr_tail(p: &mut Parser<'_>, mut cm: CompletedMarker) {
    if p.at_kw("where") {
        let m = cm.precede(p);
        p.bump_kw("where");
        if at_where_start(p) {
            where_expr(p, true);
        }
        cm = m.complete(p, RELATION_EXPR_WHERE);
    }
    if p.at_kw("with") {
        let m = cm.precede(p);
        p.bump_kw("with");
        if p.at(L_CURLY) {
            custom_relation_properties(p);
        }
        m.complete(p, RELATION_EXPR_WITH);
    }
}

/// Optional `where` and `with` clauses of an element expression.
fn fqn_expr_tail(p: &mut Parser<'_>, mut cm: CompletedMarker) {
    if p.at_kw("where") {
        let m = cm.precede(p);
        p.bump_kw("where");
        if at_where_start(p) {
            where_expr(p, false);
        }
        cm = m.complete(p, FQN_EXPR_WHERE);
    }
    if p.at_kw("with") {
        let m = cm.precede(p);
        p.bump_kw("with");
        if p.at(L_CURLY) {
            custom_element_properties(p);
        }
        m.complete(p, FQN_EXPR_WITH);
    }
}

fn custom_element_properties(p: &mut Parser<'_>) {
    let m = p.start();
    block(p, |p| {
        navigate_to_property(p)
            || string_property(p, CUSTOM_ELEMENT_STRING_KEYS, STRING_PROPERTY)
            || element_style_leaf(p)
    });
    m.complete(p, CUSTOM_ELEMENT_PROPERTIES);
}

fn custom_relation_properties(p: &mut Parser<'_>) {
    let m = p.start();
    block(p, |p| {
        navigate_to_property(p)
            || string_property(p, CUSTOM_RELATION_STRING_KEYS, STRING_PROPERTY)
            || relationship_style_leaf(p)
            || multiple_property(p)
    });
    m.complete(p, CUSTOM_RELATION_PROPERTIES);
}

// ----- where -----

fn at_participant(p: &Parser<'_>) -> bool {
    p.at(IDENT) && PARTICIPANTS.contains(&p.current_text()) && p.nth_at(1, STICKY_DOT)
}

fn at_where_start(p: &Parser<'_>) -> bool {
    p.at(L_PAREN)
        || p.at_kw("not")
        || p.at_kw("tag")
        || p.at_kw("kind")
        || p.at_kw("metadata")
        || at_participant(p)
}

/// `and`-expressions joined by `or` (left nested). `participants` allows `source.`/`target.`.
fn where_expr(p: &mut Parser<'_>, participants: bool) -> Option<CompletedMarker> {
    let mut lhs = where_and(p, participants)?;
    while p.at_kw("or") {
        let m = lhs.precede(p);
        p.bump_kw("or");
        where_and(p, participants);
        lhs = m.complete(p, WHERE_BINARY);
    }
    Some(lhs)
}

fn where_and(p: &mut Parser<'_>, participants: bool) -> Option<CompletedMarker> {
    let mut lhs = where_primary(p, participants)?;
    while p.at_kw("and") {
        let m = lhs.precede(p);
        p.bump_kw("and");
        where_primary(p, participants);
        lhs = m.complete(p, WHERE_BINARY);
    }
    Some(lhs)
}

fn where_primary(p: &mut Parser<'_>, participants: bool) -> Option<CompletedMarker> {
    if p.at(L_PAREN) {
        let m = p.start();
        p.bump(L_PAREN);
        where_expr(p, participants);
        p.expect(R_PAREN);
        return Some(m.complete(p, WHERE_PAREN));
    }
    if p.at_kw("not") {
        // Langium: `'not' value=WhereExpression` — the negation takes a whole expression.
        let m = p.start();
        p.bump_kw("not");
        where_expr(p, participants);
        return Some(m.complete(p, WHERE_NOT));
    }
    where_leaf(p, participants)
}

fn where_leaf(p: &mut Parser<'_>, participants: bool) -> Option<CompletedMarker> {
    let m = p.start();
    let has_participant = at_participant(p);
    if has_participant {
        if !participants {
            p.error("'source.' / 'target.' filters are only allowed in relation predicates");
        }
        p.bump(IDENT);
        p.bump(STICKY_DOT);
    }
    if p.at_kw("kind") {
        p.bump_kw("kind");
        eq_operator(p);
        p.expect_id();
        return Some(m.complete(p, WHERE_KIND));
    }
    if p.at_kw("tag") {
        p.bump_kw("tag");
        eq_operator(p);
        expect_tag_ref(p);
        return Some(m.complete(p, WHERE_TAG));
    }
    if p.at_kw("metadata") {
        p.bump_kw("metadata");
        if p.expect(STICKY_DOT) {
            p.expect_id();
        }
        if at_eq_operator(p) {
            eq_operator(p);
            if matches!(p.current(), STRING | BOOLEAN) {
                p.bump_any();
            } else {
                p.error("expected string or boolean");
            }
        }
        return Some(m.complete(p, WHERE_METADATA));
    }
    p.error("expected filter ('kind', 'tag' or 'metadata')");
    if has_participant {
        m.complete(p, ERROR_NODE);
    } else {
        m.abandon(p);
    }
    None
}

fn at_eq_operator(p: &Parser<'_>) -> bool {
    matches!(p.current(), EQ | NOT_EQUAL) || p.at_kw("is")
}

/// `= | != | is not?`
fn eq_operator(p: &mut Parser<'_>) -> bool {
    if p.eat(EQ) || p.eat(NOT_EQUAL) {
        return true;
    }
    if p.eat_kw("is") {
        p.eat_kw("not");
        return true;
    }
    p.error("expected '=', '!=' or 'is'");
    false
}

// ---------------------------------------------------------------------------
// Dynamic view steps
// ---------------------------------------------------------------------------

fn at_block_keyword(p: &Parser<'_>, kw: &str) -> bool {
    p.at_kw(kw) && matches!(p.nth(1), L_CURLY | STRING)
}

fn at_subflow_step(p: &Parser<'_>) -> bool {
    p.at(IDENT) && SUBFLOW_KINDS.contains(&p.current_text()) && matches!(p.nth(1), L_CURLY | STRING)
}

fn step_statement(p: &mut Parser<'_>) -> bool {
    if at_subflow_step(p) {
        subflow_step(p);
        return true;
    }
    if at_block_keyword(p, "alt") {
        alt_steps(p);
        return true;
    }
    if at_block_keyword(p, "try") {
        try_step(p);
        return true;
    }
    if p.at_id() && matches!(p.nth(1), STICKY_DOT | DOT | ARROW | BACK_ARROW | ARROW_L_BRACK) {
        step_series(p);
        return true;
    }
    false
}

/// `STEP` followed by any number of `STEP_SERIES` continuations (left nested).
fn step_series(p: &mut Parser<'_>) {
    let m = p.start();
    fqn_ref(p);
    if !connector(p, Connectors::StepFirst) {
        m.complete(p, STEP);
        return;
    }
    step_target(p);
    let mut cm = m.complete(p, STEP);
    while at_connector(p, Connectors::StepNext) {
        let m = cm.precede(p);
        connector(p, Connectors::StepNext);
        step_target(p);
        cm = m.complete(p, STEP_SERIES);
    }
}

fn step_target(p: &mut Parser<'_>) {
    expect_fqn_ref(p);
    p.eat(STRING);
    if p.at(L_CURLY) {
        custom_relation_properties(p);
    }
}

fn subflow_step(p: &mut Parser<'_>) {
    let m = p.start();
    let kw = p.current_text();
    p.bump_kw(kw);
    p.eat(STRING);
    block(p, step_statement);
    m.complete(p, SUBFLOW_STEP);
}

fn subflow_step_statement(p: &mut Parser<'_>) -> bool {
    if at_subflow_step(p) {
        subflow_step(p);
        true
    } else {
        false
    }
}

fn alt_steps(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("alt");
    p.eat(STRING);
    block(p, subflow_step_statement);
    m.complete(p, ALT_STEPS);
}

/// `try` block, optionally wrapped into `CATCH_BLOCK` and `FINALLY_BLOCK`.
fn try_step(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("try");
    p.eat(STRING);
    block(p, step_statement);
    let mut cm = m.complete(p, TRY_BLOCK);
    if at_block_keyword(p, "catch") {
        let m = cm.precede(p);
        p.bump_kw("catch");
        p.eat(STRING);
        steps_block(p);
        cm = m.complete(p, CATCH_BLOCK);
    }
    if at_block_keyword(p, "finally") {
        let m = cm.precede(p);
        p.bump_kw("finally");
        p.eat(STRING);
        steps_block(p);
        m.complete(p, FINALLY_BLOCK);
    }
}

fn steps_block(p: &mut Parser<'_>) {
    if !p.at(L_CURLY) {
        p.error("expected '{'");
        return;
    }
    let m = p.start();
    block(p, step_statement);
    m.complete(p, STEPS_BLOCK);
}

// ---------------------------------------------------------------------------
// Deployment
// ---------------------------------------------------------------------------

fn deployment(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("deployment");
    block(p, |p| deployment_statement(p, DeployCtx::Root));
    m.complete(p, DEPLOYMENT);
}

fn deployment_statement(p: &mut Parser<'_>, ctx: DeployCtx) -> bool {
    let source_required = matches!(ctx, DeployCtx::Root | DeployCtx::ExtendBody);
    let nodes_allowed = ctx != DeployCtx::InstanceBody;
    let instances_allowed = matches!(ctx, DeployCtx::ExtendBody | DeployCtx::NodeBody);
    if ctx == DeployCtx::Root && p.at_kw("extend") {
        extend_deployment(p);
        return true;
    }
    if instances_allowed && p.at_kw("instanceOf") {
        deployed_instance(p);
        return true;
    }
    if p.at_id() {
        match p.nth(1) {
            EQ if instances_allowed && p.nth_at_kw(2, "instanceOf") => deployed_instance(p),
            IDENT | EQ if nodes_allowed => deployment_node(p),
            STICKY_DOT | DOT | ARROW | ARROW_L_BRACK => deployment_relation(p, source_required),
            _ => return false,
        }
        return true;
    }
    if !source_required && at_connector(p, Connectors::Deployment) {
        deployment_relation(p, false);
        return true;
    }
    false
}

fn deployment_node(p: &mut Parser<'_>) {
    let m = p.start();
    p.expect_id();
    p.eat(EQ);
    p.expect_id();
    strings(p, 2);
    if p.at(L_CURLY) {
        deployment_node_body(p);
    }
    m.complete(p, DEPLOYMENT_NODE);
}

fn deployment_node_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while element_property(p) {}
    block_body(p, |p| deployment_statement(p, DeployCtx::NodeBody));
    m.complete(p, DEPLOYMENT_NODE_BODY);
}

/// `(name =)? instanceOf FQN_REF STRING{0,2} body? ';'?`
fn deployed_instance(p: &mut Parser<'_>) {
    let m = p.start();
    if !p.at_kw("instanceOf") {
        p.expect_id();
        p.expect(EQ);
    }
    p.expect_kw("instanceOf");
    expect_fqn_ref(p);
    strings(p, 2);
    if p.at(L_CURLY) {
        deployed_instance_body(p);
    }
    p.eat(SEMICOLON);
    m.complete(p, DEPLOYED_INSTANCE);
}

fn deployed_instance_body(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(L_CURLY);
    opt_tags(p);
    while element_property(p) {}
    block_body(p, |p| deployment_statement(p, DeployCtx::InstanceBody));
    m.complete(p, DEPLOYED_INSTANCE_BODY);
}

fn deployment_relation(p: &mut Parser<'_>, source_required: bool) {
    let m = p.start();
    if p.at(IDENT) {
        fqn_ref(p);
    } else if source_required {
        p.error("expected relation source");
    }
    if !connector(p, Connectors::Deployment) {
        m.complete(p, DEPLOYMENT_RELATION);
        return;
    }
    expect_fqn_ref(p);
    strings(p, 3);
    opt_tags(p);
    if p.at(L_CURLY) {
        relation_body(p, DEPLOYMENT_RELATION_BODY);
    }
    m.complete(p, DEPLOYMENT_RELATION);
}

fn extend_deployment(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("extend");
    expect_fqn_ref(p);
    if p.at(L_CURLY) {
        let body = p.start();
        p.bump(L_CURLY);
        opt_tags(p);
        while link_property(p) || metadata_property(p) {}
        block_body(p, |p| deployment_statement(p, DeployCtx::ExtendBody));
        body.complete(p, EXTEND_DEPLOYMENT_BODY);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, EXTEND_DEPLOYMENT);
}

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

fn globals(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump_kw("global");
    block(p, global_statement);
    m.complete(p, GLOBALS);
}

fn global_statement(p: &mut Parser<'_>) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    match p.current_text() {
        "predicateGroup" => {
            let m = p.start();
            p.bump_kw("predicateGroup");
            p.expect_id_terminal();
            block(p, include_or_exclude_predicate);
            m.complete(p, GLOBAL_PREDICATE_GROUP);
        }
        "dynamicPredicateGroup" => {
            let m = p.start();
            p.bump_kw("dynamicPredicateGroup");
            p.expect_id_terminal();
            block(p, include_predicate);
            m.complete(p, GLOBAL_DYNAMIC_PREDICATE_GROUP);
        }
        "style" => {
            let m = p.start();
            p.bump_kw("style");
            p.expect_id_terminal();
            fqn_expressions(p);
            block(p, style_rule_body_statement);
            m.complete(p, GLOBAL_STYLE);
        }
        "styleGroup" => {
            let m = p.start();
            p.bump_kw("styleGroup");
            p.expect_id_terminal();
            block(p, |p| {
                if p.at_kw("style") {
                    view_rule_style(p);
                    true
                } else {
                    false
                }
            });
            m.complete(p, GLOBAL_STYLE_GROUP);
        }
        _ => return false,
    }
    true
}

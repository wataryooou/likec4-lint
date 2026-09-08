//! Port of `LikeC4Formatter.ts` (likec4 v1.59.3) onto the `likec4-syntax` tree.
//!
//! Every method of `LikeC4Formatter.format` is ported in the same order; the Langium
//! `ast.isX` guards become `SyntaxKind` checks and `f.property(...)` selections become
//! positional child lookups (see `docs/DESIGN.md`, "Node shapes").

use likec4_syntax::SyntaxKind::{self, *};
use likec4_syntax::{NodeOrToken, SyntaxElement, SyntaxNode, SyntaxToken};

use crate::engine::{is_multiline, is_same_line, Collector, Formatting, LineIndex};

// ----- FormattingOptions of LikeC4Formatter.ts -----

fn new_line() -> Formatting {
    Formatting::new_line().allow_more()
}
fn one_space() -> Formatting {
    Formatting::one_space()
}
fn no_space() -> Formatting {
    Formatting::no_space()
}
fn indent() -> Formatting {
    Formatting::indent().allow_more()
}
fn no_indent() -> Formatting {
    Formatting::no_indent()
}

// ----- tree helpers (Langium `f.keywords` / `f.property` equivalents) -----

/// Direct children that Langium keeps in its CST (nodes, tokens, comments; no whitespace).
fn elements(node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|el| !matches!(el, NodeOrToken::Token(t) if t.kind().is_whitespace()))
        .collect()
}

fn tokens(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxToken> {
    node.children_with_tokens().filter_map(|el| el.into_token()).filter(|t| t.kind() == kind).collect()
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node, kind).into_iter().next()
}

fn any_token(node: &SyntaxNode, kinds: &[SyntaxKind]) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| kinds.contains(&t.kind()))
        .collect()
}

fn nodes(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    node.children().filter(|n| n.kind() == kind).collect()
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

fn strings(node: &SyntaxNode) -> Vec<SyntaxToken> {
    any_token(node, &[STRING, MARKDOWN_STRING])
}

fn first_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens().filter_map(|el| el.into_token()).find(|t| !t.kind().is_trivia())
}

fn prepend_all<T: Into<SyntaxElement>>(c: &mut Collector, items: impl IntoIterator<Item = T>, f: Formatting) {
    for item in items {
        c.prepend(&item.into(), f.clone());
    }
}

fn append_all<T: Into<SyntaxElement>>(c: &mut Collector, items: impl IntoIterator<Item = T>, f: Formatting) {
    for item in items {
        c.append(&item.into(), f.clone());
    }
}

fn surround_all<T: Into<SyntaxElement>>(
    c: &mut Collector,
    items: impl IntoIterator<Item = T>,
    f: Formatting,
) {
    for item in items {
        let el: SyntaxElement = item.into();
        c.prepend(&el, f.clone());
        c.append(&el, f.clone());
    }
}

/// Collect formattings for every node of the tree (Langium `iterateAstFormatting`).
pub fn collect(root: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    for node in root.descendants() {
        format(&node, lines, c);
    }
}

fn format(node: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    remove_indent_from_top_level_statements(node, c);
    indent_content_in_braces(node, lines, c);
    // normalizeQuotes is a separate pass (see `quotes.rs`).
    format_imports(node, c);
    format_specification_rule(node, c);
    format_globals(node, c);
    format_element_declaration(node, c);
    format_extend_element(node, c);
    format_relation(node, lines, c);
    format_metadata_property(node, c);
    format_deployment_node_declaration(node, c);
    format_deployed_instance(node, c);
    format_deployment_relation(node, c);
    format_extend_deployment(node, c);
    format_view(node, c);
    format_view_rule_group(node, c);
    format_view_rule_global_style(node, c);
    format_view_rule_global_predicate(node, c);
    format_include_exclude_expressions(node, lines, c);
    format_where_expression(node, c);
    format_where_relation_expression(node, c);
    format_where_element_expression(node, c);
    format_relation_expression(node, c);
    format_autolayout_property(node, c);
    format_with_predicate(node, c);
    format_steps(node, lines, c);
    format_view_rule_style(node, c);
    format_leaf_property(node, c);
    format_link_property(node, c);
    format_navigate_to_property(node, c);
    format_tags(node, c);
}

fn remove_indent_from_top_level_statements(node: &SyntaxNode, c: &mut Collector) {
    if node.parent().map(|p| p.kind()) != Some(ROOT) {
        return;
    }
    // Note: the TypeScript source lists 'deployments' (a typo), so `deployment` blocks are
    // deliberately not handled here.
    let kinds = [SPECIFICATION_KW, MODEL_KW, VIEWS_KW, LIKEC4LIB_KW, GLOBAL_KW, IMPORT_KW];
    prepend_all(c, any_token(node, &kinds), no_indent());
}

fn is_brace_node(node: &SyntaxNode) -> bool {
    match node.kind() {
        LIKEC4LIB
        | SPECIFICATION
        | SPEC_ELEMENT_KIND
        | SPEC_RELATIONSHIP_KIND
        | SPEC_DEPLOYMENT_NODE_KIND
        | GLOBALS
        | GLOBAL_STYLE
        | GLOBAL_STYLE_GROUP
        | GLOBAL_PREDICATE_GROUP
        | GLOBAL_DYNAMIC_PREDICATE_GROUP
        | MODEL
        | ELEMENT_BODY
        | EXTEND_ELEMENT_BODY
        | RELATION_BODY
        | STYLE_PROPERTY
        | METADATA_BODY
        | VIEWS
        | ELEMENT_VIEW_BODY
        | DYNAMIC_VIEW_BODY
        | DEPLOYMENT_VIEW_BODY
        | VIEW_RULE_GROUP
        | CUSTOM_ELEMENT_PROPERTIES
        | CUSTOM_RELATION_PROPERTIES
        | SUBFLOW_STEP
        | TRY_BLOCK
        | STEPS_BLOCK
        | ALT_STEPS
        | DEPLOYMENT
        | DEPLOYMENT_NODE_BODY
        | DEPLOYMENT_RELATION_BODY
        | DEPLOYED_INSTANCE_BODY
        | EXTEND_DEPLOYMENT_BODY => true,
        // `ast.isViewRuleStyle` does not match `DeploymentViewRuleStyle`.
        VIEW_RULE_STYLE => node.parent().map(|p| p.kind()) != Some(DEPLOYMENT_VIEW_BODY),
        _ => false,
    }
}

fn indent_content_in_braces(node: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    if !is_brace_node(node) {
        return;
    }
    let open_braces = tokens(node, L_CURLY);
    let close_braces = tokens(node, R_CURLY);
    for brace in &open_braces {
        c.prepend_token(brace, no_indent());
        c.prepend_token(brace, one_space());
    }
    let multiline = is_multiline(lines, node);

    // `interior` yields nothing unless there is exactly one `{` and one `}`.
    if open_braces.len() == 1 && close_braces.len() == 1 {
        let open = &open_braces[0];
        let close = &close_braces[0];
        let interior: Vec<SyntaxElement> = elements(node)
            .into_iter()
            .filter(|el| {
                el.text_range().start() >= open.text_range().end()
                    && el.text_range().end() <= close.text_range().start()
            })
            .collect();
        for el in interior {
            if !multiline {
                c.surround(&el, one_space());
                continue;
            }
            c.prepend(&el, new_line());
            c.prepend(&el, indent());
        }
    }

    for brace in &close_braces {
        if multiline {
            c.prepend_token(brace, no_indent());
            c.prepend_token(brace, new_line());
        } else {
            c.prepend_token(brace, Formatting::one_space().allow_less());
        }
    }
}

fn format_view(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        ELEMENT_VIEW => {
            let has_name = token(node, IDENT).is_some();
            let has_ref = token(node, EXTENDS_KW).is_some() || token(node, OF_KW).is_some();
            if has_name || has_ref {
                append_all(c, tokens(node, VIEW_KW), one_space());
            }
            surround_all(c, any_token(node, &[OF_KW, EXTENDS_KW]), one_space());
        }
        DYNAMIC_VIEW => append_all(c, any_token(node, &[DYNAMIC_KW, VIEW_KW]), one_space()),
        DEPLOYMENT_VIEW => append_all(c, any_token(node, &[DEPLOYMENT_KW, VIEW_KW]), one_space()),
        _ => {}
    }
}

const LEAF_PROPERTY_KINDS: &[SyntaxKind] = &[
    STRING_PROPERTY,
    SPEC_STRING_PROPERTY,
    ORDER_PROPERTY,
    COLOR_PROPERTY,
    LINE_PROPERTY,
    ARROW_PROPERTY,
    ICON_PROPERTY,
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
];

fn format_leaf_property(node: &SyntaxNode, c: &mut Collector) {
    if !LEAF_PROPERTY_KINDS.contains(&node.kind()) {
        return;
    }
    let colon = token(node, COLON);
    let Some(key) = first_token(node) else { return };
    match colon {
        None => c.append_token(&key, one_space()),
        Some(colon) => {
            c.prepend_token(&colon, no_space());
            c.append_token(&colon, one_space());
        }
    }
    if let Some(semi) = token(node, SEMICOLON) {
        c.prepend_token(&semi, no_space());
        c.append_token(&semi, new_line());
    }
}

fn format_link_property(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != LINK_PROPERTY {
        return;
    }
    if let Some(kw) = token(node, LINK_KW) {
        c.append_token(&kw, one_space());
    }
    if let Some(value) = token(node, URI) {
        c.append_token(&value, one_space());
    }
    if let Some(colon) = token(node, COLON) {
        c.prepend_token(&colon, no_space());
        c.append_token(&colon, one_space());
    }
    if let Some(semi) = token(node, SEMICOLON) {
        c.prepend_token(&semi, no_space());
        c.append_token(&semi, new_line());
    }
}

fn format_navigate_to_property(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == NAVIGATE_TO_PROPERTY {
        append_all(c, tokens(node, NAVIGATE_TO_KW), one_space());
    }
}

fn format_autolayout_property(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != VIEW_RULE_AUTO_LAYOUT {
        return;
    }
    append_all(c, tokens(node, AUTO_LAYOUT_KW), one_space());
    // rankSep and nodeSep
    prepend_all(c, tokens(node, NUMBER), one_space());
}

fn format_metadata_property(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != METADATA_ATTRIBUTE {
        return;
    }
    if let Some(key) = first_token(node) {
        c.append_token(&key, one_space());
    }
    if let Some(colon) = token(node, COLON) {
        c.prepend_token(&colon, no_space());
        c.append_token(&colon, one_space());
    }
    if let Some(semi) = token(node, SEMICOLON) {
        c.prepend_token(&semi, no_space());
        c.append_token(&semi, new_line());
    }
}

/// `system sys1` vs `sys1 = system`: returns `(kind, name)` tokens.
fn kind_and_name(node: &SyntaxNode) -> Option<(SyntaxToken, SyntaxToken)> {
    let idents = tokens(node, IDENT);
    if idents.len() < 2 {
        return None;
    }
    let (a, b) = (idents[0].clone(), idents[1].clone());
    if token(node, EQ).is_some() {
        // name = kind
        Some((b, a))
    } else {
        // kind name
        Some((a, b))
    }
}

fn format_element_declaration(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != ELEMENT {
        return;
    }
    if let Some((kind, name)) = kind_and_name(node) {
        if name.text_range().start() > kind.text_range().start() {
            c.append_token(&kind, one_space());
        } else {
            c.append_token(&name, one_space());
            c.prepend_token(&kind, one_space());
        }
    }
    prepend_all(c, strings(node), one_space());
}

fn format_extend_element(node: &SyntaxNode, c: &mut Collector) {
    if matches!(node.kind(), EXTEND_ELEMENT | EXTEND_RELATION) {
        append_all(c, tokens(node, EXTEND_KW), one_space());
    }
}

fn format_globals(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        GLOBAL_STYLE => {
            append_all(c, tokens(node, STYLE_KW), one_space());
            append_all(c, token(node, IDENT), one_space());
        }
        GLOBAL_STYLE_GROUP => append_all(c, tokens(node, STYLE_GROUP_KW), one_space()),
        GLOBAL_PREDICATE_GROUP => append_all(c, tokens(node, PREDICATE_GROUP_KW), one_space()),
        GLOBAL_DYNAMIC_PREDICATE_GROUP => {
            append_all(c, tokens(node, DYNAMIC_PREDICATE_GROUP_KW), one_space())
        }
        _ => {}
    }
}

fn format_imports(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        IMPORTS => {
            append_all(c, tokens(node, IMPORT_KW), one_space());
            surround_all(c, any_token(node, &[L_CURLY, R_CURLY, FROM_KW]), one_space());
        }
        IMPORTED => {
            for comma in tokens(node, COMMA) {
                c.prepend_token(&comma, no_space());
                c.append_token(&comma, one_space());
            }
        }
        _ => {}
    }
}

fn format_specification_rule(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        SPEC_ELEMENT_KIND | SPEC_RELATIONSHIP_KIND | SPEC_TAG | SPEC_DEPLOYMENT_NODE_KIND => {
            append_all(
                c,
                any_token(node, &[ELEMENT_KW, RELATIONSHIP_KW, TAG_KW, DEPLOYMENT_NODE_KW]),
                one_space(),
            );
        }
        SPEC_COLOR => {
            append_all(c, tokens(node, COLOR_KW), one_space());
            append_all(c, token(node, IDENT), one_space());
        }
        _ => {}
    }
}

fn format_with_predicate(node: &SyntaxNode, c: &mut Collector) {
    if matches!(node.kind(), FQN_EXPR_WITH | RELATION_EXPR_WITH) {
        prepend_all(c, tokens(node, WITH_KW), one_space());
    }
}

fn format_deployment_node_declaration(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != DEPLOYMENT_NODE {
        return;
    }
    if let Some((kind, name)) = kind_and_name(node) {
        if name.text_range().start() > kind.text_range().start() {
            c.append_token(&kind, one_space());
        } else {
            c.append_token(&name, one_space());
            c.prepend_token(&kind, one_space());
        }
    }
    // title only (summary has no rule)
    prepend_all(c, strings(node).into_iter().take(1), one_space());
}

fn format_deployed_instance(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != DEPLOYED_INSTANCE {
        return;
    }
    if let Some(eq) = token(node, EQ) {
        c.surround_token(&eq, one_space());
    }
    append_all(c, tokens(node, INSTANCE_OF_KW), one_space());
    prepend_all(c, strings(node).into_iter().take(1), one_space());
}

fn format_view_rule_global_style(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == VIEW_RULE_GLOBAL_STYLE {
        append_all(c, any_token(node, &[GLOBAL_KW, STYLE_KW]), one_space());
    }
}

fn format_view_rule_global_predicate(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == VIEW_RULE_GLOBAL_PREDICATE_REF {
        append_all(c, any_token(node, &[GLOBAL_KW, PREDICATE_KW]), one_space());
    }
}

fn format_view_rule_group(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == VIEW_RULE_GROUP {
        append_all(c, tokens(node, GROUP_KW), one_space());
    }
}

fn format_view_rule_style(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        VIEW_RULE_STYLE => append_all(c, tokens(node, STYLE_KW), one_space()),
        EXPRESSIONS | FQN_EXPRESSIONS => {
            for comma in tokens(node, COMMA) {
                c.prepend_token(&comma, no_space());
                c.append_token(&comma, one_space());
            }
        }
        _ => {}
    }
}

fn format_where_expression(node: &SyntaxNode, c: &mut Collector) {
    if matches!(node.kind(), RELATION_EXPR_WHERE | FQN_EXPR_WHERE) {
        append_all(c, tokens(node, WHERE_KW), one_space());
    }
}

/// Shared by `formatWhereRelationExpression` and `formatWhereElementExpression`; both apply
/// the same formattings to the same node kinds in our tree.
fn format_where_common(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        WHERE_BINARY => surround_all(c, any_token(node, &[AND_KW, OR_KW]), one_space()),
        WHERE_NOT => append_all(c, tokens(node, NOT_KW), one_space()),
        WHERE_KIND | WHERE_TAG | WHERE_METADATA => {
            // `operator` = Eq | NotEqual | "is"; `not` = the optional `not` after `is`.
            surround_all(c, any_token(node, &[EQ, NOT_EQUAL, IS_KW]), one_space());
            surround_all(c, tokens(node, NOT_KW), one_space());
        }
        _ => {}
    }
}

fn format_where_relation_expression(node: &SyntaxNode, c: &mut Collector) {
    format_where_common(node, c);
}

fn format_where_element_expression(node: &SyntaxNode, c: &mut Collector) {
    format_where_common(node, c);
}

fn is_predicate_root(kind: SyntaxKind) -> bool {
    // ViewRulePredicate | DeploymentViewRulePredicate | DynamicViewRule (a union that also
    // covers styles/autoLayout inside dynamic views, which have no include/exclude keyword).
    matches!(kind, VIEW_RULE_PREDICATE)
}

fn find_predicate_expression_root(node: &SyntaxNode) -> Option<SyntaxNode> {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if is_predicate_root(p.kind()) {
            return Some(p);
        }
        parent = p.parent();
    }
    None
}

fn format_include_exclude_expressions(node: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    if is_predicate_root(node.kind()) && !is_multiline(lines, node) {
        append_all(c, any_token(node, &[INCLUDE_KW, EXCLUDE_KW]), one_space());
    }
    if node.kind() == EXPRESSIONS {
        let parent = find_predicate_expression_root(node);
        let multiline = parent.as_ref().is_some_and(|p| is_multiline(lines, p));
        if multiline {
            for value in node.children() {
                c.prepend_node(&value, indent());
            }
        }
        for comma in tokens(node, COMMA) {
            c.prepend_token(&comma, no_space());
            c.append_token(&comma, if multiline { new_line() } else { one_space() });
        }
    }
}

fn format_relation_expression(node: &SyntaxNode, c: &mut Collector) {
    match node.kind() {
        INCOMING_RELATION_EXPR => append_all(c, tokens(node, ARROW), one_space()),
        IN_OUT_RELATION_EXPR => prepend_all(c, tokens(node, ARROW), one_space()),
        OUTGOING_RELATION_EXPR => {
            prepend_all(c, any_token(node, &[ARROW, BI_ARROW]), one_space());
            for t in tokens(node, ARROW_L_BRACK) {
                c.prepend_token(&t, one_space());
                c.append_token(&t, no_space());
            }
            for t in tokens(node, R_BRACK_ARROW) {
                c.prepend_token(&t, no_space());
                c.append_token(&t, one_space());
            }
            for dot_kind in nodes(node, RELATION_KIND_DOT_REF) {
                c.prepend_node(&dot_kind, one_space());
                c.append_node(&dot_kind, one_space());
            }
        }
        DIRECTED_RELATION_EXPR => {
            // target = the second child node (the first is the OUTGOING_RELATION_EXPR)
            if let Some(target) = node.children().nth(1) {
                c.prepend_node(&target, one_space());
            }
        }
        _ => {}
    }
}

fn format_tags(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() != TAGS {
        return;
    }
    // Every tag that directly follows another tag (no comma in between) gets one space;
    // tags after a comma are handled by the comma's `append`.
    let els = elements(node);
    for (i, el) in els.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if let NodeOrToken::Node(n) = el {
            if n.kind() == TAG_REF && matches!(&els[i - 1], NodeOrToken::Node(p) if p.kind() == TAG_REF) {
                c.prepend_node(n, one_space());
            }
        }
    }
    for comma in tokens(node, COMMA) {
        c.prepend_token(&comma, no_space());
        c.append_token(&comma, one_space());
    }
}

fn format_deployment_relation(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == DEPLOYMENT_RELATION {
        format_relation_like(node, c);
    }
}

/// Shared body of `formatRelation` (Relation | DeploymentRelation) and `formatDeploymentRelation`.
fn format_relation_like(node: &SyntaxNode, c: &mut Collector) {
    // The source is present iff the relation starts with an FQN_REF (otherwise it starts
    // with the connector).
    let refs = nodes(node, FQN_REF);
    let starts_with_ref = node
        .children_with_tokens()
        .find(|el| !matches!(el, NodeOrToken::Token(t) if t.kind().is_trivia()))
        .is_some_and(|el| el.as_node().is_some_and(|n| n.kind() == FQN_REF));
    let (source, target) = if starts_with_ref {
        (refs.first().cloned(), refs.get(1).cloned())
    } else {
        (None, refs.first().cloned())
    };
    if let Some(source) = &source {
        c.append_node(source, one_space());
    }
    prepend_all(c, any_token(node, &[R_BRACK_ARROW, R_BRACK_BI_ARROW]), no_space());
    append_all(c, tokens(node, ARROW_L_BRACK), no_space());
    if let Some(target) = &target {
        c.prepend_node(target, one_space());
    }
    if let Some(tags) = child(node, TAGS) {
        c.prepend_node(&tags, one_space());
    }
    prepend_all(c, strings(node), one_space());
}

fn format_relation(node: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    if matches!(node.kind(), RELATION | DEPLOYMENT_RELATION) {
        format_relation_like(node, c);
    }

    if matches!(node.kind(), STEP | STEP_SERIES) {
        surround_all(c, any_token(node, &[ARROW, BACK_ARROW]), one_space());
        for dot_kind in nodes(node, RELATION_KIND_DOT_REF) {
            c.prepend_node(&dot_kind, one_space());
            c.append_node(&dot_kind, one_space());
        }
        for t in tokens(node, R_BRACK_ARROW) {
            c.prepend_token(&t, no_space());
            c.append_token(&t, one_space());
        }
        for t in tokens(node, ARROW_L_BRACK) {
            c.prepend_token(&t, one_space());
            c.append_token(&t, no_space());
        }
        prepend_all(c, strings(node), one_space());

        let parent = node.parent();
        let wrap_to_next_line = (node.kind() == STEP_SERIES && is_multiline(lines, node))
            || (node.kind() == STEP
                && parent.as_ref().is_some_and(|p| p.kind() == STEP_SERIES && is_multiline(lines, p)));
        if !wrap_to_next_line {
            return;
        }
        let wrap = Formatting::indent().allow_more().priority(2);
        for dot_kind in nodes(node, RELATION_KIND_DOT_REF) {
            c.prepend_node(&dot_kind, wrap.clone());
        }
        prepend_all(c, any_token(node, &[ARROW, ARROW_L_BRACK]), wrap);
        if let Some(custom) = child(node, CUSTOM_RELATION_PROPERTIES) {
            if is_multiline(lines, &custom) {
                c.prepend_node(&custom, Formatting::tabs(1).allow_more().priority(2));
            }
        }
    }
}

fn format_extend_deployment(node: &SyntaxNode, c: &mut Collector) {
    if node.kind() == EXTEND_DEPLOYMENT {
        append_all(c, tokens(node, EXTEND_KW), one_space());
    }
}

fn format_steps(node: &SyntaxNode, lines: &LineIndex, c: &mut Collector) {
    match node.kind() {
        ALT_STEPS | TRY_BLOCK | CATCH_BLOCK | FINALLY_BLOCK => {
            append_all(c, any_token(node, &[TRY_KW, ALT_KW]), one_space());
            surround_all(c, strings(node), one_space());
            if node.kind() == CATCH_BLOCK {
                if let (Some(kw), Some(try_block)) = (token(node, CATCH_KW), node.children().next()) {
                    if is_same_line(lines, try_block.text_range(), kw.text_range()) {
                        c.prepend_token(&kw, one_space());
                    } else {
                        c.prepend_token(&kw, Formatting::new_line());
                    }
                    c.append_token(&kw, one_space());
                }
            }
            if node.kind() == FINALLY_BLOCK {
                if let (Some(kw), Some(try_catch)) = (token(node, FINALLY_KW), node.children().next()) {
                    if is_same_line(lines, try_catch.text_range(), kw.text_range()) {
                        c.prepend_token(&kw, one_space());
                    } else {
                        c.prepend_token(&kw, Formatting::new_line());
                    }
                    c.append_token(&kw, one_space());
                }
            }
        }
        SUBFLOW_STEP => {
            if let Some(kind) = first_token(node) {
                c.append_token(&kind, one_space());
            }
            surround_all(c, strings(node), one_space());
        }
        _ => {}
    }
}

//! Parser unit tests: tree shapes, trivia attachment and error recovery.

use crate::kind::SyntaxKind::{self, *};
use crate::{debug_tree, parse, SyntaxNode};
use rowan::{NodeOrToken, WalkEvent};

/// Parse `text`, assert it is error free and lossless, and return the full debug tree.
fn ok_tree(text: &str) -> String {
    let parse = parse(text);
    assert!(
        parse.ok(),
        "unexpected errors for {text:?}: {:?}\n{}",
        parse.errors(),
        debug_tree(&parse.syntax())
    );
    assert_eq!(parse.syntax().text().to_string(), text);
    debug_tree(&parse.syntax())
}

/// Compact rendering: one line per node/token, no ranges, trivia omitted.
fn shape(node: &SyntaxNode) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for event in node.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(NodeOrToken::Node(n)) => {
                out.push_str(&format!("{}{:?}\n", "  ".repeat(depth), n.kind()));
                depth += 1;
            }
            WalkEvent::Enter(NodeOrToken::Token(t)) if !t.kind().is_trivia() => {
                out.push_str(&format!("{}{:?} {:?}\n", "  ".repeat(depth), t.kind(), t.text()));
            }
            WalkEvent::Leave(NodeOrToken::Node(_)) => depth -= 1,
            _ => {}
        }
    }
    out
}

/// Parse `text`, assert it is error free and lossless, and return the compact shape.
fn ok_shape(text: &str) -> String {
    let parse = parse(text);
    assert!(
        parse.ok(),
        "unexpected errors for {text:?}: {:?}\n{}",
        parse.errors(),
        debug_tree(&parse.syntax())
    );
    assert_eq!(parse.syntax().text().to_string(), text);
    shape(&parse.syntax())
}

/// Parse `text` (which must contain errors), assert it is lossless, and return the shape.
fn err_shape(text: &str) -> (String, Vec<String>) {
    let parse = parse(text);
    assert!(!parse.ok(), "expected errors for {text:?}");
    assert_eq!(parse.syntax().text().to_string(), text);
    (shape(&parse.syntax()), parse.errors().iter().map(|e| e.message.clone()).collect())
}

fn kinds(node: &SyntaxNode) -> Vec<SyntaxKind> {
    node.descendants().map(|n| n.kind()).collect()
}

#[test]
fn element_both_forms_and_strings() {
    let shape = ok_shape("model {\n  system a 'A' 'summary' 'tech' 'tags'\n  b = system {\n  }\n}");
    assert_eq!(
        shape,
        r##"ROOT
  MODEL
    MODEL_KW "model"
    L_CURLY "{"
    ELEMENT
      IDENT "system"
      IDENT "a"
      STRING "'A'"
      STRING "'summary'"
      STRING "'tech'"
      STRING "'tags'"
    ELEMENT
      IDENT "b"
      EQ "="
      IDENT "system"
      ELEMENT_BODY
        L_CURLY "{"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn relation_connectors() {
    let shape = ok_shape(
        "model {\n  a -> b\n  a.b <-> c 't' 'd' 'tech' #x, #y\n  a -[uses]-> b { title 'x' }\n  a -[uses]<-> b\n  a .uses b\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  MODEL
    MODEL_KW "model"
    L_CURLY "{"
    RELATION
      FQN_REF
        IDENT "a"
      ARROW "->"
      FQN_REF
        IDENT "b"
    RELATION
      FQN_REF
        IDENT "a"
        STICKY_DOT "."
        IDENT "b"
      BI_ARROW "<->"
      FQN_REF
        IDENT "c"
      STRING "'t'"
      STRING "'d'"
      STRING "'tech'"
      TAGS
        TAG_REF
          HASH "#"
          IDENT "x"
        COMMA ","
        TAG_REF
          HASH "#"
          IDENT "y"
    RELATION
      FQN_REF
        IDENT "a"
      ARROW_L_BRACK "-["
      IDENT "uses"
      R_BRACK_ARROW "]->"
      FQN_REF
        IDENT "b"
      RELATION_BODY
        L_CURLY "{"
        STRING_PROPERTY
          TITLE_KW "title"
          STRING "'x'"
        R_CURLY "}"
    RELATION
      FQN_REF
        IDENT "a"
      ARROW_L_BRACK "-["
      IDENT "uses"
      R_BRACK_BI_ARROW "]<->"
      FQN_REF
        IDENT "b"
    RELATION
      FQN_REF
        IDENT "a"
      RELATION_KIND_DOT_REF
        DOT "."
        IDENT "uses"
      FQN_REF
        IDENT "b"
    R_CURLY "}"
"##
    );
}

#[test]
fn tags_mix_commas_and_spaces() {
    let shape = ok_shape("model {\n  a = system {\n    #t1 #t2, #t3,#t4 ;\n  }\n}");
    assert_eq!(
        shape,
        r##"ROOT
  MODEL
    MODEL_KW "model"
    L_CURLY "{"
    ELEMENT
      IDENT "a"
      EQ "="
      IDENT "system"
      ELEMENT_BODY
        L_CURLY "{"
        TAGS
          TAG_REF
            HASH "#"
            IDENT "t1"
          TAG_REF
            HASH "#"
            IDENT "t2"
          COMMA ","
          TAG_REF
            HASH "#"
            IDENT "t3"
          COMMA ","
          TAG_REF
            HASH "#"
            IDENT "t4"
          SEMICOLON ";"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn include_multiline_with_and_where() {
    let shape = ok_shape(
        "views {\n  view v {\n    include\n      a,\n      b.* with { color red },\n      -> c.d ->\n    exclude * -> * where kind = k\n  }\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    ELEMENT_VIEW
      VIEW_KW "view"
      IDENT "v"
      ELEMENT_VIEW_BODY
        L_CURLY "{"
        VIEW_RULE_PREDICATE
          INCLUDE_KW "include"
          EXPRESSIONS
            FQN_REF_EXPRESSION
              FQN_REF
                IDENT "a"
            COMMA ","
            FQN_EXPR_WITH
              FQN_REF_EXPRESSION
                FQN_REF
                  IDENT "b"
                DOT_WILDCARD ".*"
              WITH_KW "with"
              CUSTOM_ELEMENT_PROPERTIES
                L_CURLY "{"
                COLOR_PROPERTY
                  COLOR_KW "color"
                  IDENT "red"
                R_CURLY "}"
            COMMA ","
            IN_OUT_RELATION_EXPR
              INCOMING_RELATION_EXPR
                ARROW "->"
                FQN_REF_EXPRESSION
                  FQN_REF
                    IDENT "c"
                    STICKY_DOT "."
                    IDENT "d"
              ARROW "->"
        VIEW_RULE_PREDICATE
          EXCLUDE_KW "exclude"
          EXPRESSIONS
            RELATION_EXPR_WHERE
              DIRECTED_RELATION_EXPR
                OUTGOING_RELATION_EXPR
                  WILDCARD_EXPRESSION
                    STAR "*"
                  ARROW "->"
                WILDCARD_EXPRESSION
                  STAR "*"
              WHERE_KW "where"
              WHERE_KIND
                KIND_KW "kind"
                EQ "="
                IDENT "k"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn where_precedence_and_not() {
    // `and` binds tighter than `or`; `not` takes a whole expression (Langium semantics).
    let shape =
        ok_shape("views { view { include * where tag = #a or tag = #b and not kind = c or kind = d } }");
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    ELEMENT_VIEW
      VIEW_KW "view"
      ELEMENT_VIEW_BODY
        L_CURLY "{"
        VIEW_RULE_PREDICATE
          INCLUDE_KW "include"
          EXPRESSIONS
            FQN_EXPR_WHERE
              WILDCARD_EXPRESSION
                STAR "*"
              WHERE_KW "where"
              WHERE_BINARY
                WHERE_TAG
                  TAG_KW "tag"
                  EQ "="
                  TAG_REF
                    HASH "#"
                    IDENT "a"
                OR_KW "or"
                WHERE_BINARY
                  WHERE_TAG
                    TAG_KW "tag"
                    EQ "="
                    TAG_REF
                      HASH "#"
                      IDENT "b"
                  AND_KW "and"
                  WHERE_NOT
                    NOT_KW "not"
                    WHERE_BINARY
                      WHERE_KIND
                        KIND_KW "kind"
                        EQ "="
                        IDENT "c"
                      OR_KW "or"
                      WHERE_KIND
                        KIND_KW "kind"
                        EQ "="
                        IDENT "d"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn dynamic_step_chain_is_left_nested() {
    let shape = ok_shape(
        "views {\n  dynamic view d {\n    a -> b -> c 't' -[k]-> d\n    x <- y { line dotted }\n  }\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    DYNAMIC_VIEW
      DYNAMIC_KW "dynamic"
      VIEW_KW "view"
      IDENT "d"
      DYNAMIC_VIEW_BODY
        L_CURLY "{"
        STEP_SERIES
          STEP_SERIES
            STEP
              FQN_REF
                IDENT "a"
              ARROW "->"
              FQN_REF
                IDENT "b"
            ARROW "->"
            FQN_REF
              IDENT "c"
            STRING "'t'"
          ARROW_L_BRACK "-["
          IDENT "k"
          R_BRACK_ARROW "]->"
          FQN_REF
            IDENT "d"
        STEP
          FQN_REF
            IDENT "x"
          BACK_ARROW "<-"
          FQN_REF
            IDENT "y"
          CUSTOM_RELATION_PROPERTIES
            L_CURLY "{"
            LINE_PROPERTY
              LINE_KW "line"
              IDENT "dotted"
            R_CURLY "}"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn try_catch_finally_nesting() {
    let shape =
        ok_shape("views {\n  dynamic view d {\n    try { a -> b } catch { b -> a } finally 'f' { }\n    try 't' { }\n  }\n}");
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    DYNAMIC_VIEW
      DYNAMIC_KW "dynamic"
      VIEW_KW "view"
      IDENT "d"
      DYNAMIC_VIEW_BODY
        L_CURLY "{"
        FINALLY_BLOCK
          CATCH_BLOCK
            TRY_BLOCK
              TRY_KW "try"
              L_CURLY "{"
              STEP
                FQN_REF
                  IDENT "a"
                ARROW "->"
                FQN_REF
                  IDENT "b"
              R_CURLY "}"
            CATCH_KW "catch"
            STEPS_BLOCK
              L_CURLY "{"
              STEP
                FQN_REF
                  IDENT "b"
                ARROW "->"
                FQN_REF
                  IDENT "a"
              R_CURLY "}"
          FINALLY_KW "finally"
          STRING "'f'"
          STEPS_BLOCK
            L_CURLY "{"
            R_CURLY "}"
        TRY_BLOCK
          TRY_KW "try"
          STRING "'t'"
          L_CURLY "{"
          R_CURLY "}"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn flow_keywords_are_step_sources_unless_followed_by_block() {
    let shape = ok_shape("views { dynamic view d { alt -> try\n opt 'x' { loop { } } } }");
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    DYNAMIC_VIEW
      DYNAMIC_KW "dynamic"
      VIEW_KW "view"
      IDENT "d"
      DYNAMIC_VIEW_BODY
        L_CURLY "{"
        STEP
          FQN_REF
            IDENT "alt"
          ARROW "->"
          FQN_REF
            IDENT "try"
        SUBFLOW_STEP
          OPT_KW "opt"
          STRING "'x'"
          L_CURLY "{"
          SUBFLOW_STEP
            LOOP_KW "loop"
            L_CURLY "{"
            R_CURLY "}"
          R_CURLY "}"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn deployment_instance_of() {
    let shape = ok_shape(
        "deployment {\n  node n {\n    instanceOf a.b\n    i = instanceOf c 'title' { #t }\n    -> zone.i\n  }\n  extend n { instanceOf d; }\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  DEPLOYMENT
    DEPLOYMENT_KW "deployment"
    L_CURLY "{"
    DEPLOYMENT_NODE
      IDENT "node"
      IDENT "n"
      DEPLOYMENT_NODE_BODY
        L_CURLY "{"
        DEPLOYED_INSTANCE
          INSTANCE_OF_KW "instanceOf"
          FQN_REF
            IDENT "a"
            STICKY_DOT "."
            IDENT "b"
        DEPLOYED_INSTANCE
          IDENT "i"
          EQ "="
          INSTANCE_OF_KW "instanceOf"
          FQN_REF
            IDENT "c"
          STRING "'title'"
          DEPLOYED_INSTANCE_BODY
            L_CURLY "{"
            TAGS
              TAG_REF
                HASH "#"
                IDENT "t"
            R_CURLY "}"
        DEPLOYMENT_RELATION
          ARROW "->"
          FQN_REF
            IDENT "zone"
            STICKY_DOT "."
            IDENT "i"
        R_CURLY "}"
    EXTEND_DEPLOYMENT
      EXTEND_KW "extend"
      FQN_REF
        IDENT "n"
      EXTEND_DEPLOYMENT_BODY
        L_CURLY "{"
        DEPLOYED_INSTANCE
          INSTANCE_OF_KW "instanceOf"
          FQN_REF
            IDENT "d"
          SEMICOLON ";"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn globals() {
    let shape = ok_shape(
        "global {\n  predicateGroup pg { include * exclude a }\n  dynamicPredicateGroup dpg { include * }\n  style s *, a { color red notation 'n' }\n  styleGroup sg { style * { opacity 10% } }\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  GLOBALS
    GLOBAL_KW "global"
    L_CURLY "{"
    GLOBAL_PREDICATE_GROUP
      PREDICATE_GROUP_KW "predicateGroup"
      IDENT "pg"
      L_CURLY "{"
      VIEW_RULE_PREDICATE
        INCLUDE_KW "include"
        EXPRESSIONS
          WILDCARD_EXPRESSION
            STAR "*"
      VIEW_RULE_PREDICATE
        EXCLUDE_KW "exclude"
        EXPRESSIONS
          FQN_REF_EXPRESSION
            FQN_REF
              IDENT "a"
      R_CURLY "}"
    GLOBAL_DYNAMIC_PREDICATE_GROUP
      DYNAMIC_PREDICATE_GROUP_KW "dynamicPredicateGroup"
      IDENT "dpg"
      L_CURLY "{"
      VIEW_RULE_PREDICATE
        INCLUDE_KW "include"
        EXPRESSIONS
          WILDCARD_EXPRESSION
            STAR "*"
      R_CURLY "}"
    GLOBAL_STYLE
      STYLE_KW "style"
      IDENT "s"
      FQN_EXPRESSIONS
        WILDCARD_EXPRESSION
          STAR "*"
        COMMA ","
        FQN_REF_EXPRESSION
          FQN_REF
            IDENT "a"
      L_CURLY "{"
      COLOR_PROPERTY
        COLOR_KW "color"
        IDENT "red"
      STRING_PROPERTY
        NOTATION_KW "notation"
        STRING "'n'"
      R_CURLY "}"
    GLOBAL_STYLE_GROUP
      STYLE_GROUP_KW "styleGroup"
      IDENT "sg"
      L_CURLY "{"
      VIEW_RULE_STYLE
        STYLE_KW "style"
        FQN_EXPRESSIONS
          WILDCARD_EXPRESSION
            STAR "*"
        L_CURLY "{"
        OPACITY_PROPERTY
          OPACITY_KW "opacity"
          PERCENT "10%"
        R_CURLY "}"
      R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn element_kind_expression_and_view_rules() {
    let shape = ok_shape(
        "views {\n  view {\n    include element.kind = system, element.tag != #t\n    rank same { a, b }\n    autoLayout LeftRight 10 20\n    global predicate p\n    global style s\n    group 'g' { color red include * }\n  }\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  VIEWS
    VIEWS_KW "views"
    L_CURLY "{"
    ELEMENT_VIEW
      VIEW_KW "view"
      ELEMENT_VIEW_BODY
        L_CURLY "{"
        VIEW_RULE_PREDICATE
          INCLUDE_KW "include"
          EXPRESSIONS
            ELEMENT_KIND_EXPRESSION
              ELEMENT_KW "element"
              STICKY_DOT "."
              KIND_KW "kind"
              EQ "="
              IDENT "system"
            COMMA ","
            ELEMENT_TAG_EXPRESSION
              ELEMENT_KW "element"
              STICKY_DOT "."
              TAG_KW "tag"
              NOT_EQUAL "!="
              TAG_REF
                HASH "#"
                IDENT "t"
        VIEW_RULE_RANK
          RANK_KW "rank"
          IDENT "same"
          L_CURLY "{"
          FQN_EXPRESSIONS
            FQN_REF_EXPRESSION
              FQN_REF
                IDENT "a"
            COMMA ","
            FQN_REF_EXPRESSION
              FQN_REF
                IDENT "b"
          R_CURLY "}"
        VIEW_RULE_AUTO_LAYOUT
          AUTO_LAYOUT_KW "autoLayout"
          IDENT "LeftRight"
          NUMBER "10"
          NUMBER "20"
        VIEW_RULE_GLOBAL_PREDICATE_REF
          GLOBAL_KW "global"
          PREDICATE_KW "predicate"
          IDENT "p"
        VIEW_RULE_GLOBAL_STYLE
          GLOBAL_KW "global"
          STYLE_KW "style"
          IDENT "s"
        VIEW_RULE_GROUP
          GROUP_KW "group"
          STRING "'g'"
          L_CURLY "{"
          COLOR_PROPERTY
            COLOR_KW "color"
            IDENT "red"
          VIEW_RULE_PREDICATE
            INCLUDE_KW "include"
            EXPRESSIONS
              WILDCARD_EXPRESSION
                STAR "*"
          R_CURLY "}"
        R_CURLY "}"
    R_CURLY "}"
"##
    );
}

#[test]
fn specification_imports_and_lib() {
    let shape = ok_shape(
        "import { a, b } from 'p'\nimport c from 'q';\nlikec4lib { icons { aws:x tech:y } }\nspecification {\n  element e { #t\n title: 'x'; style { shape person } }\n  tag t { color rgba(1 2 3) }\n  color c #abc\n  relationship r { line dashed head none }\n  deploymentNode dn\n}",
    );
    assert_eq!(
        shape,
        r##"ROOT
  IMPORTS
    IMPORT_KW "import"
    L_CURLY "{"
    IMPORTED
      IDENT "a"
      COMMA ","
      IDENT "b"
    R_CURLY "}"
    FROM_KW "from"
    STRING "'p'"
  IMPORTS
    IMPORT_KW "import"
    IMPORTED
      IDENT "c"
    FROM_KW "from"
    STRING "'q'"
    SEMICOLON ";"
  LIKEC4LIB
    LIKEC4LIB_KW "likec4lib"
    L_CURLY "{"
    ICONS_KW "icons"
    L_CURLY "{"
    LIB_ICON_DECL
      LIB_ICON "aws:x"
    LIB_ICON_DECL
      LIB_ICON "tech:y"
    R_CURLY "}"
    R_CURLY "}"
  SPECIFICATION
    SPECIFICATION_KW "specification"
    L_CURLY "{"
    SPEC_ELEMENT_KIND
      ELEMENT_KW "element"
      IDENT "e"
      L_CURLY "{"
      TAGS
        TAG_REF
          HASH "#"
          IDENT "t"
      SPEC_STRING_PROPERTY
        TITLE_KW "title"
        COLON ":"
        STRING "'x'"
        SEMICOLON ";"
      STYLE_PROPERTY
        STYLE_KW "style"
        L_CURLY "{"
        SHAPE_PROPERTY
          SHAPE_KW "shape"
          IDENT "person"
        R_CURLY "}"
      R_CURLY "}"
    SPEC_TAG
      TAG_KW "tag"
      IDENT "t"
      L_CURLY "{"
      COLOR_KW "color"
      RGBA_COLOR
        IDENT "rgba"
        L_PAREN "("
        NUMBER "1"
        NUMBER "2"
        NUMBER "3"
        R_PAREN ")"
      R_CURLY "}"
    SPEC_COLOR
      COLOR_KW "color"
      IDENT "c"
      HEX_COLOR
        HASH "#"
        IDENT "abc"
    SPEC_RELATIONSHIP_KIND
      RELATIONSHIP_KW "relationship"
      IDENT "r"
      L_CURLY "{"
      LINE_PROPERTY
        LINE_KW "line"
        IDENT "dashed"
      ARROW_PROPERTY
        HEAD_KW "head"
        IDENT "none"
      R_CURLY "}"
    SPEC_DEPLOYMENT_NODE_KIND
      DEPLOYMENT_NODE_KW "deploymentNode"
      IDENT "dn"
    R_CURLY "}"
"##
    );
}

// ----- trivia attachment -----

#[test]
fn comment_before_statement_belongs_to_the_block() {
    let tree = ok_tree("model {\n  // c\n  a = system\n}");
    assert_eq!(
        tree,
        r##"ROOT@0..29
  MODEL@0..29
    MODEL_KW@0..5 "model"
    WHITESPACE@5..6 " "
    L_CURLY@6..7 "{"
    NEWLINE@7..8 "\n"
    WHITESPACE@8..10 "  "
    LINE_COMMENT@10..14 "// c"
    NEWLINE@14..15 "\n"
    WHITESPACE@15..17 "  "
    ELEMENT@17..27
      IDENT@17..18 "a"
      WHITESPACE@18..19 " "
      EQ@19..20 "="
      WHITESPACE@20..21 " "
      IDENT@21..27 "system"
    NEWLINE@27..28 "\n"
    R_CURLY@28..29 "}"
"##
    );
}

#[test]
fn trailing_comment_precedes_the_next_statement() {
    let tree = ok_tree("model {\n  a = system // t\n  b = system\n}");
    assert_eq!(
        tree,
        r##"ROOT@0..40
  MODEL@0..40
    MODEL_KW@0..5 "model"
    WHITESPACE@5..6 " "
    L_CURLY@6..7 "{"
    NEWLINE@7..8 "\n"
    WHITESPACE@8..10 "  "
    ELEMENT@10..20
      IDENT@10..11 "a"
      WHITESPACE@11..12 " "
      EQ@12..13 "="
      WHITESPACE@13..14 " "
      IDENT@14..20 "system"
    WHITESPACE@20..21 " "
    LINE_COMMENT@21..25 "// t"
    NEWLINE@25..26 "\n"
    WHITESPACE@26..28 "  "
    ELEMENT@28..38
      IDENT@28..29 "b"
      WHITESPACE@29..30 " "
      EQ@30..31 "="
      WHITESPACE@31..32 " "
      IDENT@32..38 "system"
    NEWLINE@38..39 "\n"
    R_CURLY@39..40 "}"
"##
    );
}

#[test]
fn file_edge_trivia_belongs_to_root_and_precede_nests_correctly() {
    let text = "/* head */\nviews { dynamic view d {\n  a -> b // x\n  -> c\n} }\n// tail\n";
    let tree = ok_tree(text);
    let root = parse(text).syntax();
    // Leading and trailing trivia are direct children of ROOT.
    let first = root.first_child_or_token().unwrap();
    assert_eq!(first.kind(), BLOCK_COMMENT);
    let last = root.last_child_or_token().unwrap();
    assert_eq!(last.kind(), NEWLINE);
    assert_eq!(root.children_with_tokens().filter(|el| el.kind() == LINE_COMMENT).count(), 1);
    // The comment before the continuation `-> c` is inside STEP_SERIES, after STEP.
    let series = root.descendants().find(|n| n.kind() == STEP_SERIES).unwrap();
    let series_kinds: Vec<SyntaxKind> =
        series.children_with_tokens().map(|el| el.kind()).filter(|k| !k.is_whitespace()).collect();
    assert_eq!(series_kinds, [STEP, LINE_COMMENT, ARROW, FQN_REF]);
    assert!(tree.contains("LINE_COMMENT@"));
}

// ----- error recovery -----

#[test]
fn property_after_nested_element_is_an_error_but_parsing_continues() {
    let (shape, errors) = err_shape("model { a = system\n  title 'x'\n  b = system }");
    assert_eq!(errors, ["'title' is not allowed here"]);
    assert_eq!(
        shape,
        r##"ROOT
  MODEL
    MODEL_KW "model"
    L_CURLY "{"
    ELEMENT
      IDENT "a"
      EQ "="
      IDENT "system"
    ERROR_NODE
      IDENT "title"
      STRING "'x'"
    ELEMENT
      IDENT "b"
      EQ "="
      IDENT "system"
    R_CURLY "}"
"##
    );
}

#[test]
fn tags_after_properties_are_an_error() {
    let (shape, errors) = err_shape("model {\n  a = system {\n    title 'x'\n    #tag\n  }\n}");
    assert_eq!(errors, ["unexpected '#'"]);
    assert!(shape.contains("ERROR_NODE\n          HASH \"#\"\n          IDENT \"tag\"\n"));
}

#[test]
fn recovery_balances_nested_braces() {
    let (shape, _) = err_shape("model {\n  title { x { y } }\n  b = system\n}");
    assert_eq!(
        shape,
        r##"ROOT
  MODEL
    MODEL_KW "model"
    L_CURLY "{"
    ERROR_NODE
      IDENT "title"
      L_CURLY "{"
      IDENT "x"
      L_CURLY "{"
      IDENT "y"
      R_CURLY "}"
      R_CURLY "}"
    ELEMENT
      IDENT "b"
      EQ "="
      IDENT "system"
    R_CURLY "}"
"##
    );
}

#[test]
fn top_level_garbage_resumes_at_next_block() {
    let text = "model { a = system } }\nfoo bar\nviews { view v { include * } }";
    let (shape, errors) = err_shape(text);
    // Everything up to the next top-level keyword on a new line is one error.
    assert_eq!(errors, ["unexpected '}'"]);
    assert_eq!(kinds(&parse(text).syntax())[..3], [ROOT, MODEL, ELEMENT]);
    assert!(
        shape.contains("  ERROR_NODE\n    R_CURLY \"}\"\n    IDENT \"foo\"\n    IDENT \"bar\"\n  VIEWS\n")
    );
}

#[test]
fn missing_closing_brace_reports_error_without_panic() {
    let parse = parse("model {\n  a = system {\n    b = system\n");
    assert!(!parse.ok());
    assert!(parse.errors().iter().all(|e| e.message == "expected '}'"));
    // Both unclosed blocks want a `}` at EOF; the identical diagnostic is reported once.
    assert_eq!(parse.errors().len(), 1);
}

#[test]
fn identical_diagnostics_at_the_same_offset_are_reported_once() {
    // Three unclosed blocks, one message at EOF.
    let parsed = parse("model {\n  a = system {\n    b = system {\n");
    assert_eq!(parsed.errors().len(), 1, "{:?}", parsed.errors());
    // Different messages at the same offset are both kept.
    let parsed = parse("model { a = system { style { shape\nfoo } } }");
    let at_foo: Vec<&str> = parsed
        .errors()
        .iter()
        .filter(|e| u32::from(e.range.start()) == 35)
        .map(|e| e.message.as_str())
        .collect();
    assert_eq!(at_foo.len(), 2, "{:?}", parsed.errors());
    assert_ne!(at_foo[0], at_foo[1]);
}

#[test]
fn multiline_block_comment_counts_as_a_line_break_for_recovery() {
    // The block comment ends the erroneous statement: `c = system` starts a new line
    // (inside the comment) and must not be swallowed by the ERROR_NODE.
    let text = "model {\n  a = system {\n    b = system\n    title 'x' /* c\n    */ c = system\n  }\n}\n";
    let (shape, errors) = err_shape(text);
    assert_eq!(errors, ["'title' is not allowed here"]);
    assert!(
        shape.contains(
            "        ERROR_NODE\n          IDENT \"title\"\n          STRING \"'x'\"\n        ELEMENT\n          IDENT \"c\"\n"
        ),
        "{shape}"
    );
    // A single-line block comment is not a line break.
    let (shape, _) =
        err_shape("model {\n  a = system {\n    b = system\n    title 'x' /* c */ c = system\n  }\n}\n");
    assert!(shape.contains("STRING \"'x'\"\n          IDENT \"c\"\n"), "{shape}");
}

#[test]
fn extend_relation_requires_a_source() {
    // Langium: `ExtendRelation: 'extend' source=FqnRef RelationConnector target=FqnRef ...`.
    let (shape, errors) = err_shape("model {\n  extend -> ghost { }\n}\n");
    assert_eq!(errors, ["expected identifier"]);
    assert!(shape.contains("    EXTEND_RELATION\n      EXTEND_KW \"extend\"\n      ARROW \"->\"\n      FQN_REF\n        IDENT \"ghost\"\n"), "{shape}");
    ok_shape("model {\n  extend a -> ghost { }\n}\n");
}

/// Depth of the deepest node (`ROOT` has depth 1), computed without recursion.
fn max_depth(node: &SyntaxNode) -> usize {
    let mut depth = 0usize;
    let mut max = 0usize;
    for event in node.preorder() {
        match event {
            WalkEvent::Enter(_) => {
                depth += 1;
                max = max.max(depth);
            }
            WalkEvent::Leave(_) => depth -= 1,
        }
    }
    max
}

/// Deeply nested input must not overflow the stack: the parser caps the tree depth, reports
/// it once and puts the rest of the input into one flat `ERROR_NODE`. The parse (and the
/// drop of the tree) runs on a 256 KiB stack, in debug builds too.
#[test]
fn deep_nesting_is_capped_on_a_small_stack() {
    const N: usize = 100_000;
    let mut elements = String::from("model {\n");
    for _ in 0..N {
        elements.push_str("a = system {\n");
    }
    for _ in 0..N {
        elements.push_str("}\n");
    }
    elements.push_str("}\n");
    let mut steps = String::from("views {\n  dynamic view d {\n    a");
    for _ in 0..N {
        steps.push_str(" -> b");
    }
    steps.push_str("\n  }\n}\n");
    for input in [elements, steps] {
        parse_capped_on_stack(input, 256 * 1024);
    }

    // `where` parentheses are the most expensive nesting (three recursive calls per level,
    // about 550 bytes per level in debug builds), so 512 levels need a little more than
    // 256 KiB in debug builds; 1 MiB is still half of a rayon worker stack.
    let mut parens = String::from("views { view { include * where ");
    parens.push_str(&"(".repeat(N));
    parens.push_str("kind = k");
    parens.push_str(&")".repeat(N));
    parens.push_str(" } }");
    parse_capped_on_stack(parens, 1024 * 1024);
}

/// Parse `input` on a thread with `stack_size` bytes of stack and check the depth cap.
fn parse_capped_on_stack(input: String, stack_size: usize) {
    use crate::MAX_NODE_DEPTH;
    let worker = std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            let parsed = parse(&input);
            let messages: Vec<&str> = parsed.errors().iter().map(|e| e.message.as_str()).collect();
            assert_eq!(messages, [format!("nesting too deep (limit {MAX_NODE_DEPTH})")]);
            let tree = parsed.syntax();
            assert_eq!(tree.text().to_string(), input);
            assert!(max_depth(&tree) <= MAX_NODE_DEPTH, "depth {}", max_depth(&tree));
            let error_nodes = tree.descendants().filter(|n| n.kind() == ERROR_NODE).count();
            assert_eq!(error_nodes, 1);
            drop(tree);
            drop(parsed);
        })
        .expect("spawn parser thread");
    worker.join().expect("parser thread panicked");
}

/// The limit is exact: `MAX_NODE_DEPTH` counts nodes from `ROOT` (depth 1) and the flat
/// `ERROR_NODE` is the only node allowed at the last level.
#[test]
fn nesting_limit_is_exact() {
    use crate::MAX_NODE_DEPTH;
    // ROOT, VIEWS, ELEMENT_VIEW, ELEMENT_VIEW_BODY, VIEW_RULE_PREDICATE, EXPRESSIONS,
    // FQN_EXPR_WHERE, WHERE_PAREN * n, WHERE_KIND: the deepest regular node is at depth n + 8.
    let where_parens = |n: usize| {
        format!("views {{ view {{ include * where {}kind = k{} }} }}", "(".repeat(n), ")".repeat(n))
    };
    let deepest_ok = MAX_NODE_DEPTH - 9;
    let tree = parse(&where_parens(deepest_ok));
    assert!(tree.ok(), "{:?}", tree.errors());
    assert_eq!(max_depth(&tree.syntax()), MAX_NODE_DEPTH - 1);

    let (_, errors) = err_shape(&where_parens(deepest_ok + 1));
    assert_eq!(errors, [format!("nesting too deep (limit {MAX_NODE_DEPTH})")]);
    let tree = parse(&where_parens(deepest_ok + 1)).syntax();
    assert_eq!(max_depth(&tree), MAX_NODE_DEPTH);
    // `kind = k`, all closing parens and braces end up in one flat error node.
    let error_nodes: Vec<SyntaxNode> = tree.descendants().filter(|n| n.kind() == ERROR_NODE).collect();
    assert_eq!(error_nodes.len(), 1);
    assert!(error_nodes[0].children().next().is_none(), "flat error node");
    assert_eq!(error_nodes[0].ancestors().count(), MAX_NODE_DEPTH);
    assert!(error_nodes[0].text().to_string().starts_with("kind = k"));
    assert!(error_nodes[0].text().to_string().ends_with(" } }"));
}

#[test]
fn describe_uses_human_readable_names() {
    use super::describe;
    for (kind, expected) in [
        (HEX, "hex literal"),
        (STRING, "string"),
        (MARKDOWN_STRING, "markdown string"),
        (NUMBER, "number"),
        (FLOAT, "decimal number"),
        (PERCENT, "percentage"),
        (BOOLEAN, "boolean"),
        (IDENT, "identifier"),
        (URI, "uri"),
        (LIB_ICON, "library icon"),
        (ERROR, "invalid token"),
        (L_CURLY, "'{'"),
        (R_BRACK_BI_ARROW, "']<->'"),
        (DOT_UNDERSCORE, "'._'"),
        (DOT_WILDCARD, "'.*'"),
        (BLOCK_COMMENT, "block comment"),
        (MODEL_KW, "'model'"),
        (AUTO_LAYOUT_KW, "'autoLayout'"),
        (ELEMENT_BODY, "element body"),
        (ERROR_NODE, "error node"),
        (__LAST, "end of file"),
    ] {
        assert_eq!(describe(kind), expected, "{kind:?}");
    }
    // Every kind has a description that is not the raw enum name.
    for raw in 0..(__LAST as u16) {
        let kind = SyntaxKind::from(rowan::SyntaxKind(raw));
        let description = describe(kind);
        if description.starts_with('\'') {
            assert!(description.ends_with('\''), "{kind:?}: {description}");
        } else {
            assert!(
                !description.contains('_') && description == description.to_lowercase(),
                "{kind:?}: {description}"
            );
        }
    }
}

#[test]
fn reserved_words_are_rejected_as_identifiers() {
    let reserved = parse("model {\n  system title\n}");
    assert!(reserved.errors().iter().any(|e| e.message.contains("reserved word")));
    let keyword = parse("global { predicateGroup element { } }");
    assert!(keyword.errors().iter().any(|e| e.message.contains("keyword")));
}

#[test]
fn broken_inputs_never_panic() {
    let inputs = [
        "",
        "{{{",
        "}}}",
        "-> ->",
        "'unterminated",
        "model { a = system 'unterminated\n b = system }",
        "model",
        "model {",
        "model { a",
        "model { a =",
        "model { a -> }",
        "model { -> b }",
        "model { extend }",
        "model { extend a -> }",
        "views { view { include } }",
        "views { view { include * where source. } }",
        "views { view { include * where ( } }",
        "views { view { style } }",
        "views { view { rank } }",
        "views { view { autoLayout } }",
        "views { dynamic view d { a -> } }",
        "views { dynamic view d { try } }",
        "views { dynamic view d { try { } catch } }",
        "views { dynamic view d { alt { a -> b } } }",
        "views { dynamic view d { -> } }",
        "views { dynamic",
        "views { deployment",
        "deployment { instanceOf }",
        "deployment { node n { i = } }",
        "deployment { extend }",
        "global { style }",
        "global { styleGroup s { include * } }",
        "specification { element }",
        "specification { tag t { color } }",
        "specification { color x }",
        "specification { color x rgba( }",
        "likec4lib { }",
        "likec4lib { icons { } }",
        "import",
        "import { a from 'x'",
        "model { a = system { metadata { k } } }",
        "model { a = system { metadata { k [ } } }",
        "model { a = system { link } }",
        "model { a = system { icon } }",
        "model { a = system { style { shape } } }",
        "model { a = system { style { opacity } } }",
        "#",
        "model { a = system { # } }",
        "model { a -[ b }",
        "model { a -[uses]<- b }",
        "日本語",
        "model { a = system 'x' 'y' 'z' 'w' 'v' }",
    ];
    for input in inputs {
        let parse = parse(input);
        assert_eq!(parse.syntax().text().to_string(), input, "lossless: {input:?}");
        assert!(!parse.ok() || input.is_empty(), "expected errors for {input:?}");
    }
}

#[test]
fn grammar_oddities_accepted_like_langium() {
    // Trailing commas, `where`/`with` without bodies, tags separated by commas only.
    ok_shape("views { view { include *, \n exclude a, b, } }");
    ok_shape("views { view { include * where \n exclude * with } }");
    ok_shape("model { a = system { #a, , #b, } }");
    ok_shape("views { view { style *, { } } }");
}

#[test]
fn every_prefix_of_a_complex_document_parses() {
    let text = "specification { element e; tag t }\nmodel {\n  a = e 'A' { #t\n    description 'd'\n    -> b.c 'x' #t { metadata { k 'v' } }\n  }\n}\nviews {\n  view v of a { include *, a -> * where tag = #t with { color red } }\n  dynamic view d { a -> b -> c\n    parallel { a -> b }\n    try { a -> b } catch { } finally { } }\n  deployment view dv { include * includeAncestors true }\n}\ndeployment { node n { instanceOf a } }\nglobal { predicateGroup g { include * } }\n";
    for end in 0..=text.len() {
        if !text.is_char_boundary(end) {
            continue;
        }
        let prefix = &text[..end];
        let parse = parse(prefix);
        assert_eq!(parse.syntax().text().to_string(), prefix);
    }
}

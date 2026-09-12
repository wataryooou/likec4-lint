# likec4-lint design

`likec4-lint` is a Rust toolchain for the [LikeC4](https://likec4.dev) DSL:
a lossless parser, a formatter that is output-compatible with the official
Langium formatter (`likec4 format`), and a rule-based linter.

Reference implementation (read-only, used as the behavioural oracle):

- Grammar: `<likec4>/packages/language-server/src/like-c4.langium`
- Formatter rules: `<likec4>/packages/language-server/src/formatting/LikeC4Formatter.ts`
- Formatter engine (Langium): `langium/lib/lsp/formatter.js` (version 3.5.0)
- Validation checks (inspiration for lint rules): `<likec4>/packages/language-server/src/validation/`

`<likec4>` is a checkout of `https://github.com/likec4/likec4` (v1.59.3).

## Crates

| crate | purpose |
| --- | --- |
| `likec4-syntax` | lexer, parser, `rowan` syntax tree, typed AST accessors, syntax diagnostics |
| `likec4-fmt` | formatter (port of Langium `AbstractFormatter` + `LikeC4Formatter` rules) |
| `likec4-rules` (formerly `likec4-lint`) | project model (multi-file), lint rules, configuration |
| `likec4-lint` (formerly `likec4-cli`) | binary `likec4-lint` with `lint`, `format`, `check` subcommands |

Dependency direction: `syntax <- fmt`, `syntax <- rules`, `{syntax, fmt, rules} <- likec4-lint`.

## Syntax tree

We use `rowan` (lossless green/red trees). Every byte of the source is in the
tree. `SyntaxKind` is a single flat enum covering tokens and nodes
(`crates/likec4-syntax/src/kind.rs`).

### Tokens

Trivia (never consumed by grammar rules, attached as described below):

- `WHITESPACE` (`[\t ]+`), `NEWLINE` (`[\r\n]+`, one token per run),
  `LINE_COMMENT` (`//...` up to but excluding the newline),
  `BLOCK_COMMENT` (`/* ... */`, may span lines, unterminated at EOF is allowed and reported).

Words: the lexer emits `IDENT` for every identifier-like word
(`([a-zA-Z]|_+[a-zA-Z0-9])[-\w]*`, ASCII only, hyphen allowed inside), except:

- `true` / `false` → `BOOLEAN`.
- `(aws|azure|bootstrap|gcp|tech):[-\w]*` → `LIB_ICON` (checked before IDENT).
- `\w+://\S+` → `URI` (checked before IDENT).

The parser remaps an `IDENT` to a keyword kind (`MODEL_KW`, `INCLUDE_KW`, ...) when it
consumes it as a keyword. Keywords are contextual: most keywords are also valid
identifiers in the `Id` rule (see "Reserved words").

Symbols: `L_CURLY {`, `R_CURLY }`, `L_PAREN (`, `R_PAREN )`, `L_BRACK [`, `R_BRACK ]`,
`COMMA`, `COLON`, `SEMICOLON`, `STAR *`, `HASH #`,
`DOT .` (dot NOT immediately preceded by a word char, e.g. `customer .uses cloud`),
`STICKY_DOT .` (dot immediately preceded by a word char `[A-Za-z0-9_]`, e.g. `a.b`),
`DOT_UNDERSCORE ._` (sticky, `._` not followed by `[_a-zA-Z]`),
`DOT_WILDCARD .*` / `.**` (sticky),
`EQ` (`=` or `==`), `NOT_EQUAL` (`!=` or `!==`),
`ARROW ->`, `BACK_ARROW <-`, `BI_ARROW <->`, `ARROW_L_BRACK -[`, `R_BRACK_ARROW ]->`,
`R_BRACK_BI_ARROW ]<->`.

Literals: `STRING` (`"..."` or `'...'`, backslash escapes, may contain raw newlines; a
backslash immediately followed by a line terminator (`\n`, `\r`, U+2028 or U+2029) is not
an escape and makes the string a lexical error. The token boundary is the same as in
Langium/Chevrotain; only the error granularity differs: this lexer emits a single `ERROR`
token that swallows the rest of the line),
`MARKDOWN_STRING` (`'''...'''` or `"""..."""`), `NUMBER` (`\d+` followed by a non-word char),
`FLOAT` (`\d+\.\d+`), `PERCENT` (`\d+%`), `HEX` (`[a-fA-F0-9]{3,}` that is not a NUMBER/IDENT,
only meaningful after `#`), `URI` (`\w+://\S+`, `\.{0,2}/[^/]\S+`, `@[\w-]*/\S+`),
`ERROR` (any byte sequence the lexer cannot classify).

Lexer priority mirrors Langium/Chevrotain token order. Notable consequences that we
reproduce on purpose:

- `a->b` without spaces lexes as `IDENT("a-")` followed by an error (`>`), exactly like Langium.
- `tech:` lexes as `LIB_ICON` even when used as a metadata key.
- `1a2b3c` is `HEX`; `123456` is `NUMBER`; `abcdef` is `IDENT`. `HexColor` accepts all three after `#`.

### Nodes

Node kinds mirror the Langium AST type names so that formatter rules can be ported
one-to-one. Property-like children are positional; a node never wraps a single
token unless the Langium AST has a node there that formatter rules address.

Top level: `ROOT`, `LIKEC4LIB`, `LIB_ICON_DECL`, `IMPORTS`, `IMPORTED`, `SPECIFICATION`,
`MODEL`, `VIEWS`, `DEPLOYMENT`, `GLOBALS`.

Specification: `SPEC_ELEMENT_KIND`, `SPEC_DEPLOYMENT_NODE_KIND`, `SPEC_RELATIONSHIP_KIND`,
`SPEC_TAG`, `SPEC_COLOR`, `SPEC_STRING_PROPERTY`.

Model: `ELEMENT`, `ELEMENT_BODY`, `RELATION`, `RELATION_BODY`, `EXTEND_ELEMENT`,
`EXTEND_ELEMENT_BODY`, `EXTEND_RELATION`, `EXTEND_RELATION_BODY`, `TAGS`, `TAG_REF`,
`FQN_REF`, `RELATION_KIND_DOT_REF`.

Properties (each is `key [:] value [;]` unless noted): `STRING_PROPERTY`
(title/description/technology/summary/notation/notes in any context), `LINK_PROPERTY`
(`link [:] uri [title] [;]`), `ICON_PROPERTY`, `METADATA_PROPERTY` (`metadata METADATA_BODY`),
`METADATA_BODY`, `METADATA_ATTRIBUTE`, `METADATA_ARRAY`, `STYLE_PROPERTY` (`style { ... }`),
`COLOR_PROPERTY`, `SHAPE_PROPERTY`, `BORDER_PROPERTY`, `OPACITY_PROPERTY`, `MULTIPLE_PROPERTY`,
`ICON_COLOR_PROPERTY`, `ICON_SIZE_PROPERTY`, `ICON_POSITION_PROPERTY`, `SIZE_PROPERTY`,
`PADDING_PROPERTY`, `TEXT_SIZE_PROPERTY`, `LINE_PROPERTY`, `ARROW_PROPERTY` (head/tail),
`ORDER_PROPERTY`, `VARIANT_PROPERTY`, `NAVIGATE_TO_PROPERTY`, `INCLUDE_ANCESTORS_PROPERTY`,
`RGBA_COLOR`, `HEX_COLOR`.

Views: `ELEMENT_VIEW`, `DYNAMIC_VIEW`, `DEPLOYMENT_VIEW`, `ELEMENT_VIEW_BODY`,
`DYNAMIC_VIEW_BODY`, `DEPLOYMENT_VIEW_BODY`, `VIEW_REF`, `VIEW_RULE_PREDICATE`
(include/exclude in every view kind), `VIEW_RULE_GLOBAL_PREDICATE_REF`, `VIEW_RULE_GROUP`,
`VIEW_RULE_STYLE` (also deployment view style), `VIEW_RULE_GLOBAL_STYLE`,
`VIEW_RULE_AUTO_LAYOUT`, `VIEW_RULE_RANK`, `EXPRESSIONS`, `FQN_EXPRESSIONS`,
`WILDCARD_EXPRESSION`, `ELEMENT_KIND_EXPRESSION`, `ELEMENT_TAG_EXPRESSION`,
`FQN_REF_EXPRESSION`, `FQN_EXPR_WHERE`, `FQN_EXPR_WITH`, `RELATION_EXPR_WHERE`,
`RELATION_EXPR_WITH`, `INCOMING_RELATION_EXPR`, `OUTGOING_RELATION_EXPR`,
`IN_OUT_RELATION_EXPR`, `DIRECTED_RELATION_EXPR`, `WHERE_BINARY`, `WHERE_NOT`, `WHERE_PAREN`,
`WHERE_KIND`, `WHERE_TAG`, `WHERE_METADATA`, `CUSTOM_ELEMENT_PROPERTIES`,
`CUSTOM_RELATION_PROPERTIES`.

Dynamic view steps: `STEP`, `STEP_SERIES`, `SUBFLOW_STEP`, `ALT_STEPS`, `TRY_BLOCK`,
`CATCH_BLOCK`, `FINALLY_BLOCK`, `STEPS_BLOCK`.

Deployment: `DEPLOYMENT_NODE`, `DEPLOYMENT_NODE_BODY`, `DEPLOYED_INSTANCE`,
`DEPLOYED_INSTANCE_BODY`, `DEPLOYMENT_RELATION`, `DEPLOYMENT_RELATION_BODY`,
`EXTEND_DEPLOYMENT`, `EXTEND_DEPLOYMENT_BODY`.

Globals: `GLOBAL_PREDICATE_GROUP`, `GLOBAL_DYNAMIC_PREDICATE_GROUP`, `GLOBAL_STYLE`,
`GLOBAL_STYLE_GROUP`.

Recovery: `ERROR_NODE` wrapping skipped tokens (written `ERROR` in the shapes below).

### Node shapes (children in order, `?` optional, `*` repeated)

```
ROOT                 := (IMPORTS | SPECIFICATION | MODEL | VIEWS | GLOBALS | DEPLOYMENT | LIKEC4LIB | ERROR)*
LIKEC4LIB            := LIKEC4LIB_KW L_CURLY ICONS_KW L_CURLY LIB_ICON_DECL+ R_CURLY R_CURLY
LIB_ICON_DECL        := LIB_ICON
IMPORTS              := IMPORT_KW (L_CURLY IMPORTED R_CURLY | IMPORTED) FROM_KW STRING SEMICOLON?
IMPORTED             := IDENT (COMMA IDENT)* SEMICOLON?
SPECIFICATION        := SPECIFICATION_KW L_CURLY (SPEC_ELEMENT_KIND | SPEC_TAG | SPEC_RELATIONSHIP_KIND | SPEC_COLOR | SPEC_DEPLOYMENT_NODE_KIND)* R_CURLY
SPEC_ELEMENT_KIND    := ELEMENT_KW IDENT (L_CURLY TAGS? (SPEC_STRING_PROPERTY | LINK_PROPERTY | STYLE_PROPERTY)* R_CURLY)?
SPEC_DEPLOYMENT_NODE_KIND := DEPLOYMENT_NODE_KW IDENT (L_CURLY TAGS? (SPEC_STRING_PROPERTY | LINK_PROPERTY | STYLE_PROPERTY)* R_CURLY)?
SPEC_TAG             := TAG_KW IDENT (L_CURLY (COLOR_KW (IDENT | RGBA_COLOR | HEX_COLOR))? R_CURLY)?
SPEC_COLOR           := COLOR_KW IDENT (RGBA_COLOR | HEX_COLOR)      (name is `CustomColorId`, see below)
SPEC_RELATIONSHIP_KIND := RELATIONSHIP_KW IDENT (L_CURLY TAGS? (LINK_PROPERTY | COLOR_PROPERTY | LINE_PROPERTY | ARROW_PROPERTY | SPEC_STRING_PROPERTY | MULTIPLE_PROPERTY)* R_CURLY)?
SPEC_STRING_PROPERTY := (TITLE_KW|DESCRIPTION_KW|TECHNOLOGY_KW|NOTATION_KW|SUMMARY_KW) COLON? (STRING|MARKDOWN_STRING) SEMICOLON?
                        (SUMMARY_KW only in element / deploymentNode specs, not in relationship specs)
RGBA_COLOR           := IDENT("rgb"|"rgba") L_PAREN NUMBER COMMA? NUMBER COMMA? NUMBER COMMA? (FLOAT|NUMBER|PERCENT)? R_PAREN
HEX_COLOR            := HASH (HEX|NUMBER|IDENT)      (bare IDENT must be an IdTerminal, i.e. not a reserved keyword)
TAGS                 := TAG_REF+ (COMMA TAG_REF*)* SEMICOLON?
TAG_REF              := HASH IDENT
MODEL                := MODEL_KW L_CURLY (EXTEND_ELEMENT | EXTEND_RELATION | RELATION | ELEMENT | ERROR)* R_CURLY
ELEMENT              := (IDENT IDENT | IDENT EQ IDENT) STRING{0,4} ELEMENT_BODY?
ELEMENT_BODY         := L_CURLY TAGS? ELEMENT_PROPERTY* (RELATION | ELEMENT | ERROR)* R_CURLY
ELEMENT_PROPERTY      = STRING_PROPERTY | STYLE_PROPERTY | LINK_PROPERTY | ICON_PROPERTY | METADATA_PROPERTY
STRING_PROPERTY      := (TITLE_KW|DESCRIPTION_KW|TECHNOLOGY_KW|SUMMARY_KW|NOTATION_KW|NOTES_KW) COLON? (STRING|MARKDOWN_STRING) SEMICOLON?
                        (the accepted keys depend on the context, see "String property keys" below)
STYLE_PROPERTY       := STYLE_KW L_CURLY STYLE_LEAF* R_CURLY
STYLE_LEAF            = COLOR_PROPERTY | SHAPE_PROPERTY | BORDER_PROPERTY | OPACITY_PROPERTY | ICON_PROPERTY | ICON_COLOR_PROPERTY | MULTIPLE_PROPERTY | SIZE_PROPERTY | PADDING_PROPERTY | TEXT_SIZE_PROPERTY | ICON_SIZE_PROPERTY | ICON_POSITION_PROPERTY
                        (in RELATION_BODY/CUSTOM_RELATION_PROPERTIES/spec relationship: COLOR_PROPERTY | LINE_PROPERTY | ARROW_PROPERTY)
COLOR_PROPERTY       := COLOR_KW COLON? IDENT SEMICOLON?
SHAPE_PROPERTY       := SHAPE_KW COLON? IDENT SEMICOLON?
BORDER_PROPERTY      := BORDER_KW COLON? IDENT SEMICOLON?
OPACITY_PROPERTY     := OPACITY_KW COLON? PERCENT SEMICOLON?
MULTIPLE_PROPERTY    := MULTIPLE_KW COLON? BOOLEAN SEMICOLON?
ICON_PROPERTY        := ICON_KW COLON? (LIB_ICON | IDENT("none") | URI) SEMICOLON?
ICON_COLOR_PROPERTY  := ICON_COLOR_KW COLON? IDENT SEMICOLON?
ICON_SIZE_PROPERTY   := ICON_SIZE_KW COLON? IDENT SEMICOLON?
ICON_POSITION_PROPERTY := ICON_POSITION_KW COLON? IDENT SEMICOLON?
SIZE_PROPERTY        := SIZE_KW COLON? IDENT SEMICOLON?
PADDING_PROPERTY     := PADDING_KW COLON? IDENT SEMICOLON?
TEXT_SIZE_PROPERTY   := TEXT_SIZE_KW COLON? IDENT SEMICOLON?
LINE_PROPERTY        := LINE_KW COLON? IDENT SEMICOLON?
ARROW_PROPERTY       := (HEAD_KW|TAIL_KW) COLON? IDENT SEMICOLON?
LINK_PROPERTY        := LINK_KW COLON? URI STRING? SEMICOLON?
METADATA_PROPERTY    := METADATA_KW METADATA_BODY
METADATA_BODY        := L_CURLY METADATA_ATTRIBUTE* R_CURLY
METADATA_ATTRIBUTE   := IDENT COLON? (STRING | MARKDOWN_STRING | METADATA_ARRAY | BOOLEAN) SEMICOLON?
METADATA_ARRAY       := L_BRACK (STRING|MARKDOWN_STRING) (COMMA (STRING|MARKDOWN_STRING))* R_BRACK
RELATION             := FQN_REF? CONNECTOR FQN_REF STRING{0,3} TAGS? RELATION_BODY?
                        (source FQN_REF is required directly inside MODEL / EXTEND_DEPLOYMENT_BODY / DEPLOYMENT)
CONNECTOR             = RELATION_KIND_DOT_REF | ARROW_L_BRACK IDENT (R_BRACK_ARROW | R_BRACK_BI_ARROW) | BI_ARROW | ARROW
                        (`ast::Relation::source`/`target` are positional — the FQN_REF before/after the
                        connector — not semantic; `a <- b` still reports `source() == a`, `target() == b`)
RELATION_KIND_DOT_REF := DOT IDENT
RELATION_BODY        := L_CURLY TAGS? (STRING_PROPERTY | NAVIGATE_TO_PROPERTY | STYLE_PROPERTY | LINK_PROPERTY | METADATA_PROPERTY)* R_CURLY
NAVIGATE_TO_PROPERTY := NAVIGATE_TO_KW VIEW_REF
VIEW_REF             := IDENT
EXTEND_ELEMENT       := EXTEND_KW FQN_REF EXTEND_ELEMENT_BODY
EXTEND_ELEMENT_BODY  := L_CURLY TAGS? (LINK_PROPERTY | METADATA_PROPERTY)* (RELATION | ELEMENT | ERROR)* R_CURLY
EXTEND_RELATION      := EXTEND_KW FQN_REF CONNECTOR FQN_REF STRING? EXTEND_RELATION_BODY
EXTEND_RELATION_BODY := L_CURLY TAGS? (LINK_PROPERTY | METADATA_PROPERTY)* R_CURLY
FQN_REF              := IDENT (STICKY_DOT IDENT)*
VIEWS                := VIEWS_KW STRING? L_CURLY (ELEMENT_VIEW | DYNAMIC_VIEW | DEPLOYMENT_VIEW | VIEW_RULE_STYLE | VIEW_RULE_GLOBAL_STYLE | ERROR)* R_CURLY
ELEMENT_VIEW         := VIEW_KW IDENT? (EXTENDS_KW VIEW_REF | OF_KW FQN_REF)? ELEMENT_VIEW_BODY?
DYNAMIC_VIEW         := DYNAMIC_KW VIEW_KW IDENT DYNAMIC_VIEW_BODY?
DEPLOYMENT_VIEW      := DEPLOYMENT_KW VIEW_KW IDENT DEPLOYMENT_VIEW_BODY?
ELEMENT_VIEW_BODY    := L_CURLY TAGS? VIEW_PROPERTY* VIEW_RULE* R_CURLY
VIEW_PROPERTY         = STRING_PROPERTY(title|description) | ORDER_PROPERTY | LINK_PROPERTY
ORDER_PROPERTY       := ORDER_KW NUMBER SEMICOLON?
VIEW_RULE             = VIEW_RULE_PREDICATE | VIEW_RULE_GLOBAL_PREDICATE_REF | VIEW_RULE_GROUP | VIEW_RULE_STYLE | VIEW_RULE_GLOBAL_STYLE | VIEW_RULE_AUTO_LAYOUT | VIEW_RULE_RANK
VIEW_RULE_PREDICATE  := (INCLUDE_KW | EXCLUDE_KW) EXPRESSIONS
VIEW_RULE_GLOBAL_PREDICATE_REF := GLOBAL_KW PREDICATE_KW IDENT
VIEW_RULE_GROUP      := GROUP_KW STRING? L_CURLY (COLOR_PROPERTY | BORDER_PROPERTY | OPACITY_PROPERTY)* (VIEW_RULE_PREDICATE | VIEW_RULE_GLOBAL_PREDICATE_REF | VIEW_RULE_GROUP)* R_CURLY
VIEW_RULE_STYLE      := STYLE_KW FQN_EXPRESSIONS L_CURLY (STYLE_LEAF | STRING_PROPERTY(notation))* R_CURLY
VIEW_RULE_GLOBAL_STYLE := GLOBAL_KW STYLE_KW IDENT
VIEW_RULE_AUTO_LAYOUT := AUTO_LAYOUT_KW IDENT NUMBER? NUMBER?
VIEW_RULE_RANK       := RANK_KW IDENT? L_CURLY FQN_EXPRESSIONS R_CURLY
DYNAMIC_VIEW_BODY    := L_CURLY TAGS? (VARIANT_PROPERTY | VIEW_PROPERTY)* (STEP_STATEMENT | VIEW_RULE_PREDICATE(include only) | VIEW_RULE_GLOBAL_PREDICATE_REF | VIEW_RULE_STYLE | VIEW_RULE_GLOBAL_STYLE | VIEW_RULE_AUTO_LAYOUT)* R_CURLY
VARIANT_PROPERTY     := VARIANT_KW IDENT
STEP_STATEMENT        = SUBFLOW_STEP | ALT_STEPS | TRY_STEP | STEP_OR_SERIES
SUBFLOW_STEP         := (OPT_KW|PAR_KW|PARALLEL_KW|LOOP_KW|WHEN_KW|IF_KW|ELSE_KW|BREAK_KW) STRING? L_CURLY STEP_STATEMENT* R_CURLY
ALT_STEPS            := ALT_KW STRING? L_CURLY SUBFLOW_STEP* R_CURLY
TRY_STEP              = TRY_BLOCK | CATCH_BLOCK | FINALLY_BLOCK        (Langium nesting, see below)
TRY_BLOCK            := TRY_KW STRING? L_CURLY STEP_STATEMENT* R_CURLY
CATCH_BLOCK          := TRY_BLOCK CATCH_KW STRING? STEPS_BLOCK
FINALLY_BLOCK        := (CATCH_BLOCK | TRY_BLOCK) FINALLY_KW STRING? STEPS_BLOCK
STEPS_BLOCK          := L_CURLY STEP_STATEMENT* R_CURLY
STEP                 := FQN_REF (BACK_ARROW | RELATION_KIND_DOT_REF | ARROW | ARROW_L_BRACK IDENT R_BRACK_ARROW) FQN_REF STRING? CUSTOM_RELATION_PROPERTIES?
STEP_SERIES          := (STEP | STEP_SERIES) (RELATION_KIND_DOT_REF | ARROW | ARROW_L_BRACK IDENT R_BRACK_ARROW) FQN_REF STRING? CUSTOM_RELATION_PROPERTIES?
                        (`a -> b -> c -> d` = STEP_SERIES(STEP_SERIES(STEP(a->b) -> c) -> d), exactly like Langium)
CUSTOM_ELEMENT_PROPERTIES  := L_CURLY (NAVIGATE_TO_PROPERTY | STRING_PROPERTY | STYLE_LEAF)* R_CURLY
CUSTOM_RELATION_PROPERTIES := L_CURLY (NAVIGATE_TO_PROPERTY | STRING_PROPERTY | COLOR_PROPERTY | LINE_PROPERTY | ARROW_PROPERTY | MULTIPLE_PROPERTY)* R_CURLY
EXPRESSIONS          := EXPRESSION? (COMMA EXPRESSION?)*      (first expression is required)
FQN_EXPRESSIONS      := FQN_EXPR (COMMA FQN_EXPR?)*
EXPRESSION            = RELATION_EXPR_WITH | RELATION_EXPR_WHERE | RELATION_EXPR | FQN_EXPR_WITH | FQN_EXPR_WHERE | FQN_EXPR
FQN_EXPR_WHERE       := FQN_EXPR WHERE_KW WHERE_EXPR?
FQN_EXPR_WITH        := (FQN_EXPR_WHERE | FQN_EXPR) WITH_KW CUSTOM_ELEMENT_PROPERTIES?
RELATION_EXPR_WHERE  := RELATION_EXPR WHERE_KW WHERE_EXPR?
RELATION_EXPR_WITH   := (RELATION_EXPR_WHERE | RELATION_EXPR) WITH_KW CUSTOM_RELATION_PROPERTIES?
FQN_EXPR              = WILDCARD_EXPRESSION | ELEMENT_KIND_EXPRESSION | ELEMENT_TAG_EXPRESSION | FQN_REF_EXPRESSION
WILDCARD_EXPRESSION  := STAR
ELEMENT_KIND_EXPRESSION := ELEMENT_KW STICKY_DOT KIND_KW (EQ | NOT_EQUAL) IDENT
ELEMENT_TAG_EXPRESSION  := ELEMENT_KW STICKY_DOT TAG_KW (EQ | NOT_EQUAL) TAG_REF
FQN_REF_EXPRESSION   := FQN_REF (DOT_UNDERSCORE | DOT_WILDCARD)?
RELATION_EXPR         = IN_OUT_RELATION_EXPR | INCOMING_RELATION_EXPR | DIRECTED_RELATION_EXPR | OUTGOING_RELATION_EXPR
INCOMING_RELATION_EXPR := ARROW FQN_EXPR
IN_OUT_RELATION_EXPR := INCOMING_RELATION_EXPR ARROW
OUTGOING_RELATION_EXPR := FQN_EXPR (BI_ARROW | RELATION_KIND_DOT_REF | ARROW | ARROW_L_BRACK IDENT R_BRACK_ARROW)
DIRECTED_RELATION_EXPR := OUTGOING_RELATION_EXPR FQN_EXPR
WHERE_EXPR            = WHERE_BINARY | WHERE_NOT | WHERE_PAREN | WHERE_KIND | WHERE_TAG | WHERE_METADATA
WHERE_BINARY         := WHERE_EXPR (AND_KW | OR_KW) WHERE_EXPR     (left-assoc, `and` binds tighter than `or`)
WHERE_NOT            := NOT_KW WHERE_EXPR                 (operand is a whole expression: `not a and b` = `not (a and b)`)
WHERE_PAREN          := L_PAREN WHERE_EXPR R_PAREN
WHERE_KIND           := (IDENT("source"|"target") STICKY_DOT)? KIND_KW OPERATOR IDENT
WHERE_TAG            := (IDENT("source"|"target") STICKY_DOT)? TAG_KW OPERATOR TAG_REF
WHERE_METADATA       := (IDENT("source"|"target") STICKY_DOT)? METADATA_KW STICKY_DOT IDENT (OPERATOR (STRING | BOOLEAN))?
OPERATOR              = EQ | NOT_EQUAL | IS_KW NOT_KW?
DEPLOYMENT           := DEPLOYMENT_KW L_CURLY (DEPLOYMENT_NODE | DEPLOYMENT_RELATION | EXTEND_DEPLOYMENT | ERROR)* R_CURLY
DEPLOYMENT_NODE      := (IDENT IDENT | IDENT EQ IDENT) STRING{0,2} DEPLOYMENT_NODE_BODY?
DEPLOYMENT_NODE_BODY := L_CURLY TAGS? ELEMENT_PROPERTY* (DEPLOYED_INSTANCE | DEPLOYMENT_RELATION | DEPLOYMENT_NODE | ERROR)* R_CURLY
DEPLOYED_INSTANCE    := (IDENT EQ)? INSTANCE_OF_KW FQN_REF STRING{0,2} DEPLOYED_INSTANCE_BODY? SEMICOLON?
DEPLOYED_INSTANCE_BODY := L_CURLY TAGS? ELEMENT_PROPERTY* (DEPLOYMENT_RELATION | ERROR)* R_CURLY
EXTEND_DEPLOYMENT    := EXTEND_KW FQN_REF EXTEND_DEPLOYMENT_BODY
EXTEND_DEPLOYMENT_BODY := L_CURLY TAGS? (LINK_PROPERTY | METADATA_PROPERTY)* (DEPLOYED_INSTANCE | DEPLOYMENT_RELATION | DEPLOYMENT_NODE | ERROR)* R_CURLY
DEPLOYMENT_RELATION  := FQN_REF? (RELATION_KIND_DOT_REF | ARROW_L_BRACK IDENT R_BRACK_ARROW | ARROW) FQN_REF STRING{0,3} TAGS? DEPLOYMENT_RELATION_BODY?
DEPLOYMENT_RELATION_BODY := L_CURLY TAGS? (STRING_PROPERTY | NAVIGATE_TO_PROPERTY | STYLE_PROPERTY | LINK_PROPERTY | METADATA_PROPERTY)* R_CURLY
DEPLOYMENT_VIEW_BODY := L_CURLY TAGS? VIEW_PROPERTY* (VIEW_RULE_PREDICATE | VIEW_RULE_STYLE | VIEW_RULE_AUTO_LAYOUT | INCLUDE_ANCESTORS_PROPERTY)* R_CURLY
INCLUDE_ANCESTORS_PROPERTY := INCLUDE_ANCESTORS_KW COLON? BOOLEAN SEMICOLON?
GLOBALS              := GLOBAL_KW L_CURLY (GLOBAL_PREDICATE_GROUP | GLOBAL_DYNAMIC_PREDICATE_GROUP | GLOBAL_STYLE | GLOBAL_STYLE_GROUP | ERROR)* R_CURLY
GLOBAL_PREDICATE_GROUP := PREDICATE_GROUP_KW IDENT L_CURLY VIEW_RULE_PREDICATE* R_CURLY
GLOBAL_DYNAMIC_PREDICATE_GROUP := DYNAMIC_PREDICATE_GROUP_KW IDENT L_CURLY VIEW_RULE_PREDICATE(include only)* R_CURLY
GLOBAL_STYLE         := STYLE_KW IDENT FQN_EXPRESSIONS L_CURLY (STYLE_LEAF | STRING_PROPERTY(notation))* R_CURLY
GLOBAL_STYLE_GROUP   := STYLE_GROUP_KW IDENT L_CURLY VIEW_RULE_STYLE* R_CURLY
```

Ordering is enforced like Langium: inside a body, tags come first, then properties,
then nested elements/relations. A property after a nested element is a syntax error.

String property keys (`STRING_PROPERTY` / `SPEC_STRING_PROPERTY`) are context dependent,
exactly as in the Langium grammar; a key outside its context is a syntax error:

| context | keys |
| --- | --- |
| `ELEMENT_BODY`, `DEPLOYMENT_NODE_BODY`, `DEPLOYED_INSTANCE_BODY` | title, description, technology, summary |
| `SPEC_ELEMENT_KIND`, `SPEC_DEPLOYMENT_NODE_KIND` | title, description, technology, notation, summary |
| `SPEC_RELATIONSHIP_KIND` | title, description, technology, notation |
| `RELATION_BODY`, `DEPLOYMENT_RELATION_BODY` | title, technology, description |
| view bodies (`VIEW_PROPERTY`) | title, description |
| `CUSTOM_ELEMENT_PROPERTIES` | title, description, technology, summary, notation, notes |
| `CUSTOM_RELATION_PROPERTIES` | title, technology, description, notation, notes |
| `VIEW_RULE_STYLE`, `GLOBAL_STYLE` | notation |

`SPEC_COLOR` names use Langium's `CustomColorId`: an `IdTerminal` (no keyword at all) or one
of the shape / arrow-type / line-option words, `source`, `target`, `element`, `model`.
Theme colours (`red`, `primary`, ...) and size words are rejected as custom colour names.

Enumerated values (`shape`, `border`, `line`, `head`/`tail`, `size`/`padding`/`textSize`/`iconSize`,
`iconPosition`, `variant`, `autoLayout` direction, `rank` value) are checked by the parser and
reported as syntax errors when the word is not one of the grammar's alternatives.

### Reserved words

`Id` accepts any `IDENT` except the hard-reserved keywords below (these are keyword
tokens in Langium that are not listed as alternatives of the `Id` rule):

```
and autoLayout border color deploymentNode description dynamic dynamicPredicateGroup
exclude extend extends from global head icon iconColor iconPosition iconSize icons import
include includeAncestors instanceOf is kind likec4lib line link metadata multiple navigateTo
not notation notes of opacity or order padding predicate predicateGroup rank rgb rgba shape
size specification style styleGroup summary tag tail technology textSize title variant view
views where with TopBottom LeftRight BottomTop RightLeft true false
```

Everything else (including `element`, `model`, `group`, `node`, `deployment`, `instance`,
`relationship`, shapes, colours, sizes, flow keywords such as `loop`, `alt`, `try`) is a
valid `Id`. `IdTerminal` (used for global predicate group / style names) additionally
rejects every keyword.

### Trivia attachment (must match Langium for formatter compatibility)

Langium attaches comments (hidden tokens) *before* the token that follows them, as
siblings inserted into the innermost CST node that already has content. Whitespace is
not in the Langium CST at all. We reproduce that:

- All trivia (whitespace, newlines, comments) preceding token `T` is emitted as leading
  siblings before every node that starts at `T`, i.e. into the innermost node that
  has already started and has at least one child.
- Trivia at the very beginning of the file becomes the first children of `ROOT`.
- Trivia after the last token becomes the last children of `ROOT`.

Consequently a node never starts with trivia (except `ROOT`) and never ends with trivia
unless the trivia precedes its closing `}`.

### Parser

Hand-written recursive descent over a token vector, producing events
(`Start(kind)`, `Token`, `Finish`) that are replayed into `rowan::GreenNodeBuilder`
(rust-analyzer style with `Marker`/`CompletedMarker` and `precede()` for left-nested
rules such as `STEP_SERIES`, `IN_OUT_RELATION_EXPR`, `DIRECTED_RELATION_EXPR`,
`FQN_EXPR_WHERE`, `WHERE_BINARY`, `CATCH_BLOCK`, `FINALLY_BLOCK`).

Lookahead skips trivia. Keyword tests are `p.at_kw("model")` (IDENT with that text);
consuming a keyword remaps the token to its `*_KW` kind.

Decision points:

- Block statement starting with `IDENT`: 2nd token `IDENT` or `EQ` → element/deployment
  node (`IDENT EQ INSTANCE_OF_KW` → deployed instance); `STICKY_DOT`, `DOT`, `ARROW`,
  `BI_ARROW`, `ARROW_L_BRACK` → relation.
- `extend`: after the `FQN_REF`, a connector → `EXTEND_RELATION`, `{` → `EXTEND_ELEMENT`.
- Dynamic view body: subflow keyword followed by `{` or `STRING {` → `SUBFLOW_STEP`;
  otherwise it is a step source.
- `element.kind` / `element.tag` followed by `=`/`!=` → the special expressions.
- Expression: `->` first → incoming (+ optional trailing `->`); otherwise parse a
  `FQN_EXPR`, then a connector makes it outgoing (+ optional target).
- `where` binds tighter than `with`; both bodies are optional.

Node depth: `MAX_NODE_DEPTH = 512`, counting `ROOT` as depth 1 and including left-nested
markers created via `precede()` (`STEP_SERIES`, `WHERE_BINARY`, ...). When a node would push
the tree past the limit, the parser reports `"nesting too deep (limit 512)"` once and wraps
the remaining tokens into a single flat `ERROR_NODE` instead of nesting further; later parser
diagnostics are suppressed for the rest of the file (lexer errors are unaffected).

Error recovery: on an unexpected token inside a block, wrap tokens into an `ERROR` node
until a token that can start the next statement (a keyword/identifier at the start of
a line) or the matching `}` and continue. Diagnostics carry byte ranges. A block comment
that spans multiple lines counts as being at the start of a line for this purpose.
Diagnostics that would repeat the same `(offset, message)` pair are collapsed into one.

Public API of `likec4-syntax`:

```rust
pub fn parse(text: &str) -> Parse;               // never fails
pub struct Parse { green: GreenNode, errors: Vec<SyntaxError> }
impl Parse { pub fn syntax(&self) -> SyntaxNode; pub fn errors(&self) -> &[SyntaxError]; pub fn ok(&self) -> bool }
pub struct SyntaxError { pub message: String, pub range: TextRange }
pub type SyntaxNode = rowan::SyntaxNode<LikeC4Language>; // + SyntaxToken, SyntaxElement
pub mod ast;   // thin typed wrappers used by fmt/lint (Element, Relation, View, ...)
```

Tests: unit tests for the lexer; parser tests that (1) parse every `.c4` under
`tests/corpus/examples`, `tests/corpus/examples-formatted`, `tests/fixtures/formatter`,
`tests/fixtures/formatter-cli` and `tests/fixtures/formatter-quirks` (290 files) and assert a
lossless round trip (`tree.text() == input`) for all but two expected failures — both files of
the `45-preserves-empty-lines` fixture, whose `metadata` property has no body, a genuine
Langium syntax error that the upstream formatter spec test applies edits to without checking
diagnostics — (2) snapshot a few trees, (3) check error recovery on broken input.

## Formatter (`likec4-fmt`)

Goal: byte-identical output with `likec4 format` (Langium 3.5.0 + LikeC4Formatter) for
every input that parses without errors. Inputs with syntax errors are returned unchanged
(the official formatter refuses to format them).

### Engine (port of Langium `AbstractFormatter`)

1. Collect: walk AST nodes in pre-order (root first, then `descendants`). For each
   node run every rule that applies (rules are listed in the same order as
   `LikeC4Formatter.format`). A rule produces `(target, mode, Formatting)` where
   `target` is a node or token, `mode` is `Prepend` or `Append`.
   Store into a map keyed by `(target.text_range(), mode)`; a later entry replaces an
   earlier one when `new.priority >= existing.priority` (default priority 0).
   `Formatting { moves: Vec<Move>, allow_more, allow_less, priority }`,
   `Move { characters: Option<u32>, lines: Option<u32>, tabs: Option<u32> }`:
   `noSpace`=chars 0, `oneSpace`=chars 1, `newLine`=lines 1, `indent`=lines 1 + tabs 1,
   `noIndent`=tabs 0, custom `{tabs:1}`.
2. Apply: walk the CST in pre-order over all elements (nodes, tokens, comments),
   skipping `WHITESPACE`/`NEWLINE` tokens entirely (they are not Langium CST nodes).
   Keep `last_leaf` (the previous token *or comment*) and `indentation: u32`.
   For each element `e`:
   - if a `Prepend` entry exists for `e`: `edit(last_leaf, e, formatting)`.
   - if an `Append` entry exists for `e`: `edit(e, next_leaf_or_node_after(e), formatting)`
     where the next element is the next sibling (comments included) walking up the tree
     when at the end of a parent.
   - if no `Prepend` entry and `e` is a comment: `hidden_edit(last_leaf, e, None)`.
   - if `e` is a token or comment: `last_leaf = e`.
   - if `e` is a node: remember `indentation` on entry and restore it after its children.
3. `edit(a, b, f)`:
   - if `b` is a comment → `hidden_edit(a, b, Some(f))`.
   - if `a` ends after `b` starts → nothing.
   - gap = `[a.end, b.start)` (or `[0, b.start)` when `a` is none).
   - `move` = `f.moves[0]` (single move always in this code base).
   - `existing_indent = indentation; indentation += move.tabs.unwrap_or(0)`.
   - if `move.characters` is set: unless `a` is a comment, emit a replacement of the gap
     with `" " * n` where, when the gap contains no newline, `n = fit(n, existing_spaces)`.
   - else if `move.lines` is set: `n = fit(lines, existing_newlines_in_gap)`; emit
     `"\n" * n + indent_str * indentation`.
   - else if `move.tabs` is set: `n = max(existing_newlines, if a.is_some() {1} else {0})`;
     emit `"\n" * n + indent_str * indentation`.
   - if `b` is a token or comment: `indentation = existing_indent`.
   - `fit(v, existing)`: `allow_more → max(existing, v)`, else `allow_less → min(existing, v)`, else `v`.
4. `hidden_edit(prev, comment, f)`: if `prev` ends on the comment's start line → nothing.
   Otherwise compute the existing indentation of the comment's line (tabs count as
   `tab_size` columns) and the expected one `(indentation + f.tabs.unwrap_or(0)) * tab_size`;
   if they differ, adjust the indentation of every line of the comment (add spaces, or
   remove up to the difference of leading whitespace). Deviation (tab-indent mode only): when
   reducing a comment's indentation, we remove the shortest whitespace prefix whose measured
   width already reaches the target width, rather than removing raw characters one at a time.
   Langium always strips exactly one raw character per unit of excess indentation, so a
   space-indented comment shrinks by one column per formatting pass instead of reaching the
   target in one pass — see the known quirk below.
5. Overlap resolution: edits are produced in traversal order; when a new edit starts
   before the end of the previous edit, drop the previous one (Langium
   `avoidOverlappingEdits`). Edits whose text equals the existing text (after removing
   `\r`) are dropped. Apply the remaining edits to the source. When two edits land on the
   same empty gap — `key: value` (a metadata attribute or `link:`), `import{a}from`,
   `try'x'{`, `-[uses]->b` — the official formatter's real traversal
   applies both, printing `import  { a }  from` on its first pass and only collapsing to one
   space on a second pass; we let the later edit win outright, so our single pass lands
   directly on the official formatter's fixed point (its second pass) rather than reproducing
   its unstable first pass (fixture: `tests/fixtures/formatter-quirks/same-gap-last-wins`).
6. Quote normalisation (`quoteStyle`: `auto` (default) | `single` | `double` | `ignore`)
   runs after the whitespace pass on the *original* token ranges: collect string tokens
   from the rule list in `LikeC4Formatter.normalizeQuotes`; `auto` picks `double` when
   `count(starts with '"') * 2 >= total` else `single`; replace the fence and escape
   unescaped occurrences of the new quote inside (`escapeQuotesInternalQuotes`). The
   exact set of targeted strings matters: e.g. `Relation.description` and
   `DeploymentNode.summary` are *not* normalised. A markdown string (`'''...'''` /
   `"""..."""`) is left alone when its content ends with the quote character
   normalisation would switch to, because escaping it there would still let the fence and
   content merge into something the official lexer cannot re-tokenise (it would print
   `"""...\""""`, which does not round-trip); the official formatter emits that broken output,
   we don't.

Indent string: `tab_size` spaces (default 2) or a tab when `insert_spaces = false`.
Output uses `\n` only inside edited regions; untouched regions keep their bytes.
`format()` re-parses its own output before returning it; if the reparsed tree carries any
diagnostic, `FormatError::Unstable` is returned instead of text — nothing unformattable is
ever handed back to a caller. `LineIndex` (used for line/column reporting) treats `\n`,
`\r\n` and a lone `\r` all as line breaks.

### Rules

Port every method of `LikeC4Formatter.format` in order, using the node kinds above
instead of `ast.isX` guards and positional children instead of `f.property(...)`.
`f.keywords('x')` selects tokens of that kind that belong to the node itself (not to
nested nodes; in our tree a nested node is a different `SyntaxNode`, so select direct
children only, except for the few Langium data-type composites that we flattened,
which never contain keywords).

`indentContentInBraces` skips an interior node that overlaps the node before it
(`utils.areOverlap`, whose range check is inclusive at both ends, so a node that starts
exactly where the previous one ends counts as overlapping): a statement immediately abutting
the previous one with no whitespace between them (`title 'x'description 'y'`,
`b = system {}c = system`) is left on one line rather than pushed to its own
(`tests/fixtures/formatter-quirks/adjacent-nodes`).

Known quirks to reproduce (they come from the TypeScript source):

- `removeIndentFromTopLevelStatements` lists `'deployments'` (typo), so a top-level
  `deployment { }` block keeps its indentation and preceding newlines.
- `SpecificationTag` bodies (`tag foo { color red }`) are not in `indentContentInBraces`
  and have no leaf-property rule, so their interior whitespace is untouched.
- `variant`, `includeAncestors`, `rank`, `SpecificationColor` values, `autoLayout`
  direction have no or only partial rules.
- Single-line braces: interior nodes get `oneSpace` on both sides and the closing brace
  gets `oneSpace({allowLess})`, so `{a}` becomes `{ a}`.
- A comment indented with tabs where spaces are expected is not idempotent in space mode:
  Langium measures the existing indentation in columns (a tab counts as `tab_size`) but
  removes one raw character per column of excess, so the first pass strips both tabs and
  leaves the comment at column 0, and only the second pass indents it correctly with spaces
  (`tests/fixtures/formatter-quirks/comment-tab-indent-non-idempotent`; this is the same
  first-pass behaviour as the official formatter, reproduced on purpose — it only affects
  space indentation, `--use-tabs` mode removes whole indentation units and is a fixed point).
- A file with a UTF-8 BOM fails to lex (same as the official formatter) and is returned
  unchanged rather than formatted.

## Linter (`likec4-rules`)

Project discovery: a directory tree; any of `.likec4rc`, `.likec4.config.json`,
`likec4.config.json`, `likec4.config.js`, `likec4.config.cjs`, `likec4.config.mjs`,
`likec4.config.ts`, `likec4.config.cts` or `likec4.config.mts` marks a project root
(`PROJECT_CONFIG_FILENAMES` in `crates/likec4-rules/src/lib.rs`; only the file's presence is
checked, its content is never read), files under `node_modules` are skipped, `*.c4`,
`*.likec4`, `*.like-c4` (case-insensitive) are documents. All documents of a project are
parsed and a `ProjectModel` is built:

- specification kinds (element, deploymentNode, relationship), tags, custom colours,
  global predicate groups / styles (with declaration sites),
- elements (FQN → declaration site, kind, tags), views (name → site, kind),
- deployment nodes and instances,
- relations (source/target FQN text, site).

Rules (id, severity, description):

| id | default | what |
| --- | --- | --- |
| `syntax-error` | error | parser diagnostics |
| `unknown-element-kind` | error | element kind not declared in any `specification` of the project (also `element.kind =` and `where kind is` operands; the union of all projects is accepted when the document imports another project) |
| `unknown-deployment-node-kind` | error | same for deployment nodes |
| `unknown-relationship-kind` | error | `-[kind]->` / `.kind` not declared |
| `unknown-tag` | error | `#tag` not declared |
| `unknown-custom-color` | error | `color foo` that is neither a theme colour nor a declared custom colour |
| `duplicate-element` | error | the same FQN declared twice; model elements and the deployment namespace (deployment nodes and deployed instances) are checked separately, so an element and a deployment node may share a name |
| `duplicate-view` | error | same view name declared twice |
| `duplicate-spec` | error | same kind, tag or colour declared twice across the `specification` blocks of a project, or the same predicate group or style name declared twice across its `global` blocks |
| `unresolved-reference` | error | relation endpoint / `extend` target / `instanceOf` target / `navigateTo` / view `of` / `extends`, or a view rule element (`include` / `exclude` expressions, `style` targets, `rank`), `global predicate` or `global style` reference whose name is unknown (conservative: the last FQN segment must exist somewhere; `this` / `it` resolve to the enclosing element) |
| `self-relation` | warning | a relation whose source and target resolve to the same element, comparing both endpoints via the model's scoping rules from the enclosing element (a name resolves to a child of the enclosing element first, then an ancestor's child, then a root element; `this` / `it` and an omitted source are the enclosing element itself); an endpoint that does not resolve in the project is skipped (`unresolved-reference` covers it), and dynamic view steps are exempt (`a -> a` is a valid self-message) |
| `unused-element-kind` | warning | declared kind never used, as seen from the declaring project: a use in one of its own documents or in an importing document counts |
| `unused-tag` | warning | declared tag never used (same visibility as `unused-element-kind`) |
| `unused-relationship-kind` | warning | declared relationship kind never used (same visibility as `unused-element-kind`) |
| `empty-body` | warning | `{ }` with nothing but whitespace or comments inside, checked for every brace-owning node kind (top-level blocks, specification kind/tag bodies, element/relation/extend/deployment-node/instance bodies, view bodies, `style`/`metadata`/`with` blocks, view rule `group`/`rank`, global predicate/style groups, dynamic-view sub-flows); deliberately skipped: `import { ... }` (a list, not a body), `likec4lib` and `ERROR_NODE` |
| `view-without-rules` | warning | an element or deployment view with no `include`/`exclude` or global predicate rule; a view with `extends` and any dynamic view are exempt |
| `deprecated-element-predicate` | warning | `element.kind` / `element.tag` expressions (grammar marks them as backwards compatibility) |
| `reserved-name` | warning | element / view / deployment node / instance named `element`, `model`, `group`, `node`, `deployment`, `instance`, `relationship` |
| `multiple-specifications` | warning | more than one `specification` block in a document |
| `invalid-color` | error | bad hex length / rgb range / alpha range |
| `invalid-opacity` | warning | opacity outside 0..100% |
| `naming-convention` | off | element/view names must match the configured `pattern` regex (default `^[a-z][a-zA-Z0-9]*$`); `targets` selects which of `element`, `view`, `deployment-node` are checked (default `["element", "view"]`); the diagnostic message names only the offending value, the expected pattern is in `help` |
| `require-title` | off | elements must have a title (inline string or `title` property) |
| `require-tags` | off | elements of the configured `kinds` must carry the configured `tags` (tags added through `extend` count); `kinds` empty or missing means every kind |

Configuration file `likec4-lint.toml` (searched upwards from cwd):

```toml
[format]
quote_style = "auto"   # auto | single | double | ignore
indent_width = 2       # 1..=16
use_tabs = false

[lint]
exclude = ["**/generated/**"]

[lint.rules]
unused-tag = "off"
naming-convention = { level = "warning", pattern = "^[a-z][a-zA-Z0-9]*$" }
require-tags = { level = "error", kinds = ["system"], tags = ["owner"] }
```

Both `[format]` and `ConfigFile` reject unknown keys/sections (`deny_unknown_fields`); an
unrecognised `[format]` / `[lint]` key or section is a configuration error (exit `2`), as is a `level`
that is not one of `off`/`info`/`warning`/`error` (`"unknown level 'warn' (expected one of:
off, info, warning, error)"`) or an `indent_width` outside `1..=16`. Two rule-configuration
problems are reported as diagnostics rather than failing the run: an id under `[lint.rules]`
that is not a known rule (`unknown-rule`) and an option a rule does not declare
(`unknown-rule-option`); both are `warning`, carry no `file`, and are always emitted even when
there are no other findings. An option of the wrong type (for example a number where
`naming-convention`'s `pattern` wants a string) is reported under the rule's own id and the
rule falls back to that option's default.

Diagnostics: `{ rule, severity, message, file, range, help, related }`, where `related` is an
optional second location (`{ file, range, message }`, used by the `duplicate-*` rules to point
at the first declaration). Rendered with `annotate-snippets` (pretty) or as JSON (`--format
json`):

```jsonc
{
  "version": 1,
  "diagnostics": [
    {
      "rule": "...", "severity": "error" | "warning" | "info", "message": "...",
      "file": "cwd-relative path, absolute if outside cwd" | null,  // null for a config diagnostic
      "range": { "start": 0, "end": 0 },      // byte offsets
      "line": 1, "column": 1,                 // 1-based; column counts characters
      "help": "..." | null,
      "related": { "file": "...", "range": { "start": 0, "end": 0 }, "line": 1, "column": 1, "message": "..." } | null
    }
  ],
  "summary": { "errors": 0, "warnings": 0, "infos": 0, "files": 0, "config": "relative path" | null }
}
```

The JSON envelope is emitted for `lint`, `check`, `format --check` and `format --write` even
when there are zero diagnostics; a bare `format <file>` without `--check`/`--write` prints the
formatted text on success and only switches to JSON when there is something to report. Exit
codes: `0` no error diagnostic (a warning-only run is `0`), `1` at least one error diagnostic
(`needs-formatting`, `format-error` and `io-error` are errors for this purpose — `format
--write` writing files does not itself affect the exit code), `2` a usage, configuration or
I/O error.

## CLI

```
likec4-lint lint [PATHS...] [--format pretty|json] [--json] [--color auto|always|never] [--quiet] [--list-rules] [--config FILE]
likec4-lint format [PATHS...] [--check] [--write] [--stdin] [--stdin-filepath PATH] [--diff]
                    [--quote-style auto|single|double|ignore] [--indent-width N] [--use-tabs]
                    [--format pretty|json] [--json] [--color auto|always|never] [--quiet] [--config FILE]
likec4-lint check [PATHS...]          # lint + format --check, same shared flags as lint
```

Files are discovered with the `ignore` crate (respects `.gitignore` even outside a git
repository, and the ancestor `.gitignore`s / global excludes above a non-repository path;
`node_modules` and hidden directories are skipped); files are read in parallel with `rayon`
(parsing and rule evaluation run on the collected set, not per-file in the walk). `format`
without `--write`/`--check` prints the formatted text of exactly one file to stdout; zero
matching files is a usage error (`no LikeC4 documents found under <paths>`) and more than one
is also a usage error (`pass --write or --check`) — both exit `2`.

`[lint] exclude` globs (resolved against the config file's directory) only suppress
reporting for `lint`/`check`: an excluded document is still parsed and contributes to the
`ProjectModel` (so other documents can still reference it), it is just not itself walked for
diagnostics. `format` neither reads nor writes an excluded document. A path named explicitly
on the command line is always processed, excluded or not.

Project context for a request path follows the same discovery as the linter (marker files
above the path), with a fallback order when no marker is found: (1) the adopted config file's
directory, but only when the request path is under it; (2) a directory argument is its own
root; (3) a file argument's root is the nearest ancestor containing `.git`, else the current
directory (if the file is under it), else the file's parent directory. A marker found strictly
below the chosen root starts a separate project. Because a `.git`-rooted fallback walks the
whole repository, linting one file in a large monorepo with no project markers can walk far
more than that file — fast, thanks to the `ignore` crate, but worth knowing.

Config search stops at the first directory containing `.git` (file or directory) or at
`$HOME`, whichever comes first; project-marker search upwards uses the same boundary. The home directory itself is never a project root or a context origin, even when it holds a config file, a marker or `.git`.

`--write` replaces a file atomically: write to a new file in the same directory, then rename
over the original; a symlink target has its real file rewritten while the link itself is
preserved, and permissions are copied to the new file. A write failure is reported as an
`io-error` diagnostic and the run continues, exiting `1` at the end rather than aborting.

`--stdin` reads one document from stdin, named `<stdin>` in messages unless
`--stdin-filepath PATH` overrides it, and is mutually exclusive with `--write` and with
`PATHS`; `--check` and `--write` are mutually exclusive; `--json` is mutually exclusive with
`--format`. Without `--check`, `--stdin` prints the formatted result to stdout. With
`--check`: nothing is printed to stdout; formatted-but-different input prints `needs
formatting <name>` to stderr and exits `1`; a syntax error prints `error: <msg>
(<name>:line:col)` to stderr and exits `1`; an unstable-formatting result prints `error:
<Display> (<name>)` to stderr and exits `1`.

`--quiet` suppresses the per-file status line and the final summary line; diagnostics, a bare
`format`'s printed text, and `--diff` output are unaffected. `--diff` (pretty output only)
shows a unified diff for files that need formatting, used with `format --check`.

The summary line reads `N error(s), M warning(s), K info(s) in F file(s)`, with ` (config:
<relative path>)` appended when a config file was picked up.

Pretty rendering (`annotate-snippets`): only the two lines before and after the diagnostic
span are sliced out of the file; a line longer than 1024 bytes is windowed to 80 bytes on each
side of the span plus `...`; `related` renders as a second snippet ("first declared here" for
the `duplicate-*` rules); control characters in file names and in `--diff` bodies are escaped
as `\u{XX}`.

A document reachable under more than one name (for example through a symlink) is processed
once, under the first name seen. Document extensions (`.c4`, `.likec4`, `.like-c4`) are
matched case-insensitively. Deep nesting (past `MAX_NODE_DEPTH = 512`) is a `syntax-error`
("nesting too deep (limit 512)"), not a crash.

MSRV is Rust 1.88 (`rust-version` in the workspace `Cargo.toml`). Crates: `likec4-syntax`,
`likec4-fmt`, `likec4-rules` (formerly the `likec4-lint` library crate), and `likec4-lint`
(the CLI, formerly `likec4-cli`, directory `crates/likec4-lint`) — installed with `cargo
install --path crates/likec4-lint`.

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
| `likec4-lint` | project model (multi-file), lint rules, configuration |
| `likec4-cli` | binary `likec4-lint` with `lint`, `format`, `check` subcommands |

Dependency direction: `syntax <- fmt`, `syntax <- lint`, `{syntax, fmt, lint} <- cli`.

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

Literals: `STRING` (`"..."` or `'...'`, backslash escapes, may contain raw newlines),
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
HEX_COLOR            := HASH (HEX|NUMBER|IDENT)
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
  siblings **before every node that starts at `T`**, i.e. into the innermost node that
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

Error recovery: on an unexpected token inside a block, wrap tokens into an `ERROR` node
until a token that can start the next statement (a keyword/identifier at the start of
a line) or the matching `}` and continue. Diagnostics carry byte ranges.

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
`tests/corpus/` without errors and assert lossless round trip (`tree.text() == input`),
(2) snapshot a few trees, (3) check error recovery on broken input.

## Formatter (`likec4-fmt`)

Goal: byte-identical output with `likec4 format` (Langium 3.5.0 + LikeC4Formatter) for
every input that parses without errors. Inputs with syntax errors are returned unchanged
(the official formatter refuses to format them).

### Engine (port of Langium `AbstractFormatter`)

1. **Collect.** Walk AST nodes in pre-order (root first, then `descendants`). For each
   node run every rule that applies (rules are listed in the same order as
   `LikeC4Formatter.format`). A rule produces `(target, mode, Formatting)` where
   `target` is a node or token, `mode` is `Prepend` or `Append`.
   Store into a map keyed by `(target.text_range(), mode)`; a later entry replaces an
   earlier one when `new.priority >= existing.priority` (default priority 0).
   `Formatting { moves: Vec<Move>, allow_more, allow_less, priority }`,
   `Move { characters: Option<u32>, lines: Option<u32>, tabs: Option<u32> }`:
   `noSpace`=chars 0, `oneSpace`=chars 1, `newLine`=lines 1, `indent`=lines 1 + tabs 1,
   `noIndent`=tabs 0, custom `{tabs:1}`.
2. **Apply.** Walk the CST in pre-order over all elements (nodes, tokens, comments),
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
   remove up to the difference of leading whitespace).
5. **Overlap resolution.** Edits are produced in traversal order; when a new edit starts
   before the end of the previous edit, drop the previous one (Langium
   `avoidOverlappingEdits`). Edits whose text equals the existing text (after removing
   `\r`) are dropped. Apply the remaining edits to the source.
6. **Quote normalisation** (`quoteStyle`: `auto` (default) | `single` | `double` | `ignore`)
   runs after the whitespace pass on the *original* token ranges: collect string tokens
   from the rule list in `LikeC4Formatter.normalizeQuotes`; `auto` picks `double` when
   `count(starts with '"') * 2 >= total` else `single`; replace the fence and escape
   unescaped occurrences of the new quote inside (`escapeQuotesInternalQuotes`). The
   exact set of targeted strings matters: e.g. `Relation.description` and
   `DeploymentNode.summary` are *not* normalised.

Indent string: `tab_size` spaces (default 2) or a tab when `insert_spaces = false`.
Output uses `\n` only inside edited regions; untouched regions keep their bytes.

### Rules

Port every method of `LikeC4Formatter.format` in order, using the node kinds above
instead of `ast.isX` guards and positional children instead of `f.property(...)`.
`f.keywords('x')` selects tokens of that kind that belong to the node itself (not to
nested nodes; in our tree a nested node is a different `SyntaxNode`, so select direct
children only, except for the few Langium data-type composites that we flattened,
which never contain keywords).

Known quirks to reproduce (they come from the TypeScript source):

- `removeIndentFromTopLevelStatements` lists `'deployments'` (typo), so a top-level
  `deployment { }` block keeps its indentation and preceding newlines.
- `SpecificationTag` bodies (`tag foo { color red }`) are not in `indentContentInBraces`
  and have no leaf-property rule, so their interior whitespace is untouched.
- `variant`, `includeAncestors`, `rank`, `SpecificationColor` values, `autoLayout`
  direction have no or only partial rules.
- Single-line braces: interior nodes get `oneSpace` on both sides and the closing brace
  gets `oneSpace({allowLess})`, so `{a}` becomes `{ a}`.

## Linter (`likec4-lint`)

Project discovery: a directory tree; `likec4.config.{json,mjs,js,ts}` or
`.likec4rc` marks a project root (only JSON is parsed for `exclude`), files under
`node_modules` are skipped, `*.c4`, `*.likec4`, `*.like-c4` are documents. All documents
of a project are parsed and a `ProjectModel` is built:

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
| `duplicate-element` | error | same FQN declared twice |
| `duplicate-view` | error | same view name declared twice |
| `duplicate-spec` | error | same kind/tag/colour declared twice in specifications |
| `unresolved-reference` | error | relation endpoint / `extend` target / `instanceOf` target / `navigateTo` / `of` / `extends` / `global predicate` / `global style` whose name is unknown (conservative: the last FQN segment must exist somewhere; `this` / `it` resolve to the enclosing element) |
| `self-relation` | warning | `a -> a` in model or deployment relations (dynamic view steps are exempt: `a -> a` is a valid self-message) |
| `unused-element-kind` | warning | declared kind never used anywhere in the workspace |
| `unused-tag` | warning | declared tag never used |
| `unused-relationship-kind` | warning | declared relationship kind never used |
| `empty-body` | warning | `{ }` with nothing inside |
| `view-without-rules` | warning | view whose body has no include/exclude/global predicate |
| `deprecated-element-predicate` | warning | `element.kind` / `element.tag` expressions (grammar marks them as backwards compatibility) |
| `reserved-name` | warning | element / view / deployment node / instance named `element`, `model`, `group`, `node`, `deployment`, `instance`, `relationship` |
| `multiple-specifications` | warning | more than one `specification` block in a document |
| `invalid-color` | error | bad hex length / rgb range / alpha range |
| `invalid-opacity` | warning | opacity outside 0..100% |
| `naming-convention` | off | element names must match a configured regex |
| `require-title` | off | elements must have a title |
| `require-tags` | off | elements of configured kinds must carry configured tags |

Configuration file `likec4-lint.toml` (searched upwards from cwd):

```toml
[format]
quote_style = "auto"   # auto | single | double | ignore
indent_width = 2

[lint]
exclude = ["**/generated/**"]

[lint.rules]
unused-tag = "off"
naming-convention = { level = "warning", pattern = "^[a-z][a-zA-Z0-9]*$" }
require-tags = { level = "error", kinds = ["system"], tags = ["owner"] }
```

Diagnostics: `{ rule, severity, message, file, range, help }` rendered with
`annotate-snippets` (pretty) or as JSON (`--format json`). Exit code 1 when any error.

## CLI

```
likec4-lint lint [PATHS...] [--format pretty|json] [--config FILE]
likec4-lint format [PATHS...] [--check] [--write] [--stdin] [--quote-style auto|single|double|ignore]
likec4-lint check [PATHS...]          # lint + format --check
```

Files are discovered with the `ignore` crate (respects `.gitignore`), processed in
parallel with `rayon`. `format` without `--write`/`--check` prints the formatted text
of a single file or a summary for many.

//! Syntax kinds for tokens and nodes of the LikeC4 DSL.
//!
//! The enum is flat (rowan style). Node kinds mirror the Langium AST type names
//! of the reference grammar so that formatter rules can be ported one-to-one.
//!
//! Keywords are declared once, in the `keywords` section of the `syntax_kinds!` invocation:
//! the `*_KW` variants, [`SyntaxKind::is_keyword`], the text <-> kind conversions and the
//! reserved-word lists ([`HARD_RESERVED_WORDS`], [`SOFT_RESERVED_WORDS`], [`ALL_KEYWORDS`])
//! are all derived from that table (plus the private `VALUE_KEYWORDS` table for keyword
//! literals that never become a kind), so nothing depends on the order of the enum variants.

#![allow(non_camel_case_types)]

/// How a keyword may be used as a name (the `Id` / `IdTerminal` rules of the grammar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reserved {
    /// Keyword token that is not an alternative of the `Id` rule: never an identifier.
    Hard,
    /// Valid `Id`, but flagged by the linter (`reserved-name`).
    Soft,
    /// Valid `Id`; only `IdTerminal` rejects it.
    Free,
}

macro_rules! syntax_kinds {
    (
        tokens { $($(#[$token_attr:meta])* $token:ident,)* }
        keywords { $($kw:ident = $text:literal $reserved:ident,)* }
        nodes { $($(#[$node_attr:meta])* $node:ident,)* }
    ) => {
        /// All token and node kinds.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$token_attr])* $token,)*
            $($kw,)*
            $($(#[$node_attr])* $node,)*
            /// Marker: must stay last (bounds the `u16` -> `SyntaxKind` conversion).
            #[doc(hidden)]
            __LAST,
        }

        impl SyntaxKind {
            /// True for keyword kinds produced by remapping an identifier.
            pub fn is_keyword(self) -> bool {
                matches!(self, $($kw)|*)
            }

            /// True for token kinds (as opposed to node kinds).
            pub fn is_token(self) -> bool {
                matches!(self, $($token)|*) || self.is_keyword()
            }

            /// The keyword kind for `text`, if `text` is a keyword that has one.
            pub fn from_keyword(text: &str) -> Option<SyntaxKind> {
                match text {
                    $($text => Some($kw),)*
                    _ => None,
                }
            }

            /// Source text of a keyword kind.
            pub fn keyword_text(self) -> Option<&'static str> {
                match self {
                    $($kw => Some($text),)*
                    _ => None,
                }
            }
        }

        /// Text of every keyword kind, in enum order, with how it is reserved.
        const KEYWORD_KINDS: &[(&str, Reserved)] = &[$(($text, Reserved::$reserved),)*];
    };
}

syntax_kinds! {
    tokens {
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
    }

    // Keywords remapped from `IDENT` by the parser: `KIND = "text" reservation`.
    // `Hard`: keyword token not listed in the `Id` rule; `Soft`: valid `Id` flagged by the
    // linter; `Free`: valid `Id` (only `IdTerminal` rejects it).
    keywords {
        LIKEC4LIB_KW = "likec4lib" Hard,
        ICONS_KW = "icons" Hard,
        SPECIFICATION_KW = "specification" Hard,
        ELEMENT_KW = "element" Soft,
        DEPLOYMENT_NODE_KW = "deploymentNode" Hard,
        TAG_KW = "tag" Hard,
        COLOR_KW = "color" Hard,
        RELATIONSHIP_KW = "relationship" Soft,
        TITLE_KW = "title" Hard,
        DESCRIPTION_KW = "description" Hard,
        TECHNOLOGY_KW = "technology" Hard,
        NOTATION_KW = "notation" Hard,
        SUMMARY_KW = "summary" Hard,
        NOTES_KW = "notes" Hard,
        IMPORT_KW = "import" Hard,
        FROM_KW = "from" Hard,
        MODEL_KW = "model" Soft,
        EXTEND_KW = "extend" Hard,
        STYLE_KW = "style" Hard,
        METADATA_KW = "metadata" Hard,
        VIEWS_KW = "views" Hard,
        VIEW_KW = "view" Hard,
        EXTENDS_KW = "extends" Hard,
        OF_KW = "of" Hard,
        DYNAMIC_KW = "dynamic" Hard,
        VARIANT_KW = "variant" Hard,
        ORDER_KW = "order" Hard,
        RANK_KW = "rank" Hard,
        GROUP_KW = "group" Soft,
        INCLUDE_KW = "include" Hard,
        EXCLUDE_KW = "exclude" Hard,
        GLOBAL_KW = "global" Hard,
        PREDICATE_KW = "predicate" Hard,
        AUTO_LAYOUT_KW = "autoLayout" Hard,
        NAVIGATE_TO_KW = "navigateTo" Hard,
        WITH_KW = "with" Hard,
        WHERE_KW = "where" Hard,
        AND_KW = "and" Hard,
        OR_KW = "or" Hard,
        NOT_KW = "not" Hard,
        IS_KW = "is" Hard,
        KIND_KW = "kind" Hard,
        OPT_KW = "opt" Free,
        PAR_KW = "par" Free,
        PARALLEL_KW = "parallel" Free,
        LOOP_KW = "loop" Free,
        WHEN_KW = "when" Free,
        IF_KW = "if" Free,
        ELSE_KW = "else" Free,
        BREAK_KW = "break" Free,
        TRY_KW = "try" Free,
        CATCH_KW = "catch" Free,
        FINALLY_KW = "finally" Free,
        ALT_KW = "alt" Free,
        DEPLOYMENT_KW = "deployment" Soft,
        INSTANCE_OF_KW = "instanceOf" Hard,
        INCLUDE_ANCESTORS_KW = "includeAncestors" Hard,
        PREDICATE_GROUP_KW = "predicateGroup" Hard,
        DYNAMIC_PREDICATE_GROUP_KW = "dynamicPredicateGroup" Hard,
        STYLE_GROUP_KW = "styleGroup" Hard,
        LINK_KW = "link" Hard,
        OPACITY_KW = "opacity" Hard,
        MULTIPLE_KW = "multiple" Hard,
        ICON_KW = "icon" Hard,
        ICON_COLOR_KW = "iconColor" Hard,
        ICON_SIZE_KW = "iconSize" Hard,
        ICON_POSITION_KW = "iconPosition" Hard,
        SHAPE_KW = "shape" Hard,
        BORDER_KW = "border" Hard,
        SIZE_KW = "size" Hard,
        PADDING_KW = "padding" Hard,
        TEXT_SIZE_KW = "textSize" Hard,
        LINE_KW = "line" Hard,
        HEAD_KW = "head" Hard,
        TAIL_KW = "tail" Hard,
    }

    nodes {
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
    }
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

    /// True for `STRING` and `MARKDOWN_STRING`.
    pub fn is_string(self) -> bool {
        matches!(self, STRING | MARKDOWN_STRING)
    }
}

/// Keyword literals of the grammar that never become a `*_KW` kind (the parser checks them
/// by text: colour functions, enumerated values, ...), with how they are reserved.
#[rustfmt::skip]
const VALUE_KEYWORDS: &[(&str, Reserved)] = &[
    ("rgb", Reserved::Hard), ("rgba", Reserved::Hard),
    ("node", Reserved::Soft), ("instance", Reserved::Soft),
    // enumerated values
    ("TopBottom", Reserved::Hard), ("LeftRight", Reserved::Hard),
    ("BottomTop", Reserved::Hard), ("RightLeft", Reserved::Hard),
    ("same", Reserved::Free), ("min", Reserved::Free), ("max", Reserved::Free),
    ("source", Reserved::Free), ("sink", Reserved::Free), ("target", Reserved::Free),
    ("none", Reserved::Free), ("left", Reserved::Free), ("right", Reserved::Free),
    ("top", Reserved::Free), ("bottom", Reserved::Free),
    ("xs", Reserved::Free), ("sm", Reserved::Free), ("md", Reserved::Free),
    ("lg", Reserved::Free), ("xl", Reserved::Free), ("xsmall", Reserved::Free),
    ("small", Reserved::Free), ("medium", Reserved::Free), ("large", Reserved::Free),
    ("xlarge", Reserved::Free),
    ("solid", Reserved::Free), ("dashed", Reserved::Free), ("dotted", Reserved::Free),
    ("normal", Reserved::Free), ("onormal", Reserved::Free), ("dot", Reserved::Free),
    ("odot", Reserved::Free), ("diamond", Reserved::Free), ("odiamond", Reserved::Free),
    ("crow", Reserved::Free), ("open", Reserved::Free), ("vee", Reserved::Free),
    ("diagram", Reserved::Free), ("sequence", Reserved::Free),
    ("primary", Reserved::Free), ("secondary", Reserved::Free), ("muted", Reserved::Free),
    ("slate", Reserved::Free), ("blue", Reserved::Free), ("indigo", Reserved::Free),
    ("sky", Reserved::Free), ("red", Reserved::Free), ("gray", Reserved::Free),
    ("green", Reserved::Free), ("amber", Reserved::Free),
    ("rectangle", Reserved::Free), ("component", Reserved::Free), ("person", Reserved::Free),
    ("browser", Reserved::Free), ("mobile", Reserved::Free), ("cylinder", Reserved::Free),
    ("storage", Reserved::Free), ("queue", Reserved::Free), ("bucket", Reserved::Free),
    ("document", Reserved::Free),
];

/// `BOOLEAN` literals: terminals rather than keywords, but the `Id` rule cannot use them.
const BOOLEAN_LITERALS: &[&str] = &["true", "false"];

/// Number of words [`collect_words`] yields for the same arguments.
const fn count_words(reserved: Option<Reserved>, booleans: bool) -> usize {
    let tables = [KEYWORD_KINDS, VALUE_KEYWORDS];
    let mut n = 0;
    let mut t = 0;
    while t < tables.len() {
        let mut i = 0;
        while i < tables[t].len() {
            if wanted(tables[t][i].1, reserved) {
                n += 1;
            }
            i += 1;
        }
        t += 1;
    }
    if booleans {
        n += BOOLEAN_LITERALS.len();
    }
    n
}

/// Words of both keyword tables reserved as `reserved` (every word for `None`), kinds first,
/// followed by the boolean literals when `booleans` is set.
const fn collect_words<const N: usize>(reserved: Option<Reserved>, booleans: bool) -> [&'static str; N] {
    let tables = [KEYWORD_KINDS, VALUE_KEYWORDS];
    let mut out = [""; N];
    let mut n = 0;
    let mut t = 0;
    while t < tables.len() {
        let mut i = 0;
        while i < tables[t].len() {
            let (word, how) = tables[t][i];
            if wanted(how, reserved) {
                out[n] = word;
                n += 1;
            }
            i += 1;
        }
        t += 1;
    }
    if booleans {
        let mut i = 0;
        while i < BOOLEAN_LITERALS.len() {
            out[n] = BOOLEAN_LITERALS[i];
            n += 1;
            i += 1;
        }
    }
    assert!(n == N, "keyword table size mismatch");
    out
}

const fn wanted(how: Reserved, reserved: Option<Reserved>) -> bool {
    match reserved {
        None => true,
        Some(r) => r as u8 == how as u8,
    }
}

const HARD: [&str; count_words(Some(Reserved::Hard), true)] = collect_words(Some(Reserved::Hard), true);
const SOFT: [&str; count_words(Some(Reserved::Soft), false)] = collect_words(Some(Reserved::Soft), false);
const ALL: [&str; count_words(None, false)] = collect_words(None, false);

/// Words that are keyword tokens in the Langium grammar and are *not* listed as
/// alternatives of the `Id` rule (plus `true` / `false`). They cannot be used as identifiers.
pub const HARD_RESERVED_WORDS: &[&str] = &HARD;

/// Words that parse as identifiers but are flagged by the linter (`reserved-name`).
pub const SOFT_RESERVED_WORDS: &[&str] = &SOFT;

/// Every alphanumeric keyword literal of the grammar (keyword tokens in Langium).
/// `IdTerminal` rejects all of these; `Id` rejects only [`HARD_RESERVED_WORDS`].
pub const ALL_KEYWORDS: &[&str] = &ALL;

/// Whether `text` may be used where the grammar expects `Id`.
pub fn is_valid_id(text: &str) -> bool {
    !HARD_RESERVED_WORDS.contains(&text)
}

/// Whether `text` may be used where the grammar expects `IdTerminal`.
pub fn is_valid_id_terminal(text: &str) -> bool {
    !ALL_KEYWORDS.contains(&text) && !BOOLEAN_LITERALS.contains(&text)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn all_kinds() -> impl Iterator<Item = SyntaxKind> {
        (0..(__LAST as u16)).map(|raw| SyntaxKind::from(rowan::SyntaxKind(raw)))
    }

    fn sorted<'a>(words: &[&'a str]) -> Vec<&'a str> {
        let mut out = words.to_vec();
        out.sort_unstable();
        out
    }

    /// The word lists derived from the keyword table are exactly the ones the grammar
    /// (and `docs/DESIGN.md`, "Reserved words") specify.
    #[test]
    fn reserved_word_lists_match_the_grammar() {
        #[rustfmt::skip]
        let hard = [
            "and", "autoLayout", "border", "color", "deploymentNode", "description", "dynamic",
            "dynamicPredicateGroup", "exclude", "extend", "extends", "from", "global", "head", "icon",
            "iconColor", "iconPosition", "iconSize", "icons", "import", "include", "includeAncestors",
            "instanceOf", "is", "kind", "likec4lib", "line", "link", "metadata", "multiple",
            "navigateTo", "not", "notation", "notes", "of", "opacity", "or", "order", "padding",
            "predicate", "predicateGroup", "rank", "rgb", "rgba", "shape", "size", "specification",
            "style", "styleGroup", "summary", "tag", "tail", "technology", "textSize", "title",
            "variant", "view", "views", "where", "with", "TopBottom", "LeftRight", "BottomTop",
            "RightLeft", "true", "false",
        ];
        assert_eq!(sorted(HARD_RESERVED_WORDS), sorted(&hard));
        let soft = ["element", "model", "group", "node", "deployment", "instance", "relationship"];
        assert_eq!(sorted(SOFT_RESERVED_WORDS), sorted(&soft));
        #[rustfmt::skip]
        let all = [
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
            "TopBottom", "LeftRight", "BottomTop", "RightLeft", "same", "min", "max", "source", "sink",
            "target", "none", "left", "right", "top", "bottom", "xs", "sm", "md", "lg", "xl", "xsmall",
            "small", "medium", "large", "xlarge", "solid", "dashed", "dotted", "normal", "onormal", "dot",
            "odot", "diamond", "odiamond", "crow", "open", "vee", "diagram", "sequence", "primary",
            "secondary", "muted", "slate", "blue", "indigo", "sky", "red", "gray", "green", "amber",
            "rectangle", "component", "person", "browser", "mobile", "cylinder", "storage", "queue",
            "bucket", "document",
        ];
        assert_eq!(ALL_KEYWORDS, &all[..]);

        // No duplicates, soft and hard are disjoint, and everything (except the boolean
        // literals) is a keyword.
        let unique: std::collections::HashSet<&str> = ALL_KEYWORDS.iter().copied().collect();
        assert_eq!(unique.len(), ALL_KEYWORDS.len());
        assert!(HARD_RESERVED_WORDS.iter().all(|w| !SOFT_RESERVED_WORDS.contains(w)));
        assert!(SOFT_RESERVED_WORDS.iter().all(|w| ALL_KEYWORDS.contains(w)));
        assert!(HARD_RESERVED_WORDS.iter().all(|w| ALL_KEYWORDS.contains(w) || BOOLEAN_LITERALS.contains(w)));
        assert!(BOOLEAN_LITERALS.iter().all(|w| !ALL_KEYWORDS.contains(w)));

        assert!(is_valid_id("element") && is_valid_id("opt") && is_valid_id("red"));
        assert!(!is_valid_id("title") && !is_valid_id("rgb") && !is_valid_id("true"));
        assert!(is_valid_id_terminal("customer"));
        assert!(
            !is_valid_id_terminal("element")
                && !is_valid_id_terminal("red")
                && !is_valid_id_terminal("false")
        );
    }

    /// `is_keyword`, `keyword_text` and `from_keyword` agree with each other and with
    /// `ALL_KEYWORDS`, for every kind, without relying on the enum order.
    #[test]
    fn keyword_kinds_round_trip() {
        let mut keyword_kinds = 0;
        for kind in all_kinds() {
            match kind.keyword_text() {
                Some(text) => {
                    assert!(kind.is_keyword(), "{kind:?}");
                    assert!(kind.is_token(), "{kind:?}");
                    assert_eq!(SyntaxKind::from_keyword(text), Some(kind));
                    assert!(ALL_KEYWORDS.contains(&text), "{text}");
                    assert!(format!("{kind:?}").ends_with("_KW"), "{kind:?}");
                    keyword_kinds += 1;
                }
                None => {
                    assert!(!kind.is_keyword(), "{kind:?}");
                    assert!(!format!("{kind:?}").ends_with("_KW"), "{kind:?}");
                }
            }
        }
        assert_eq!(keyword_kinds, KEYWORD_KINDS.len());
        assert_eq!(SyntaxKind::from_keyword("rgb"), None);
        assert_eq!(SyntaxKind::from_keyword("customer"), None);
        assert!(!__LAST.is_keyword() && !__LAST.is_token());
        assert!(IDENT.is_token() && ERROR.is_token() && MODEL_KW.is_token());
        assert!(!ROOT.is_token() && !ERROR_NODE.is_token());
    }
}

//! Markdown → Telegram-supported HTML subset converter.
//!
//! Ported from files.md (`server/pkg/txt/md.go` lines 262–432, `str.go` lines 122–170)
//! by Artem Zakirullin.
//!
//! Uses parser combinators (open/close/or/and/some) for inline markup.
//! Supported tags: `*`/`_` → `<i>`, `**`/`__` → `<b>`,
//! `` ` `` → `<code>`, ` ``` ` → `<pre>`, `#` → `<b>`.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::rc::Rc;

// Pre-compiled regexes used on hot paths (F15). Compiling per call was
// visible in profiles, especially when markdown_to_html ran over each
// chat block during nightly cleanup.
static RE_STRIP_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]*>").unwrap());
static RE_NEWLINES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{2,}").unwrap());
static RE_CODE_BLOCK: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)```(.+?)```").unwrap());
static RE_INLINE_CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`([^`]+?)`").unwrap());
static RE_HEADER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^#+\s*(.+)").unwrap());

// ---------------------------------------------------------------------------
// Public API — utility functions
// ---------------------------------------------------------------------------
/// Escape HTML special characters (`&`, `<`, `>`).
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Strip all HTML tags from a string.
pub fn strip_html_tags(s: &str) -> String {
    RE_STRIP_TAGS.replace_all(s, "").to_string()
}

/// Replace regex matches with placeholders, returning the modified string
/// and a map of placeholder → original.
pub fn replace_with_placeholders(
    s: &str,
    pattern: &str,
    placeholder: &str,
) -> (String, HashMap<String, String>) {
    let re = Regex::new(pattern).unwrap();
    let mut placeholders = HashMap::new();
    let mut counter: usize = 0;

    let result = re
        .replace_all(s, |caps: &regex::Captures<'_>| {
            let full = caps.get(0).unwrap().as_str().to_string();
            // Wrap with NUL bytes (\x00 … \x00) so user-typed content can
            // never collide with the placeholder and overwrite restored
            // text (F21). NUL is illegal in well-formed markdown.
            let ph = format!("\x00{placeholder}{counter}\x00");
            counter += 1;
            placeholders.insert(ph.clone(), full);
            ph
        })
        .to_string();

    (result, placeholders)
}

/// Restore placeholders back to their original values.
pub fn restore_from_placeholders(s: &str, placeholders: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    for (ph, original) in placeholders {
        result = result.replace(ph, original);
    }
    result
}

// ---------------------------------------------------------------------------
// Parser-combinator infrastructure
// ---------------------------------------------------------------------------

/// A single parse result: `consumed` is the matched/transformed text,
/// `left` is the unconsumed remainder.
#[derive(Clone, Debug)]
struct ParseResult {
    consumed: String,
    left: String,
}

/// The open-tag mapping: markdown token → HTML open tag.
static OPEN_TAGS: &[(&str, &str)] = &[("*", "<i>"), ("**", "<b>"), ("_", "<i>"), ("__", "<b>")];

/// The close-tag mapping: markdown token → HTML close tag.
static CLOSE_TAGS: &[(&str, &str)] =
    &[("*", "</i>"), ("**", "</b>"), ("_", "</i>"), ("__", "</b>")];

fn open_tag(token: &str) -> &'static str {
    OPEN_TAGS
        .iter()
        .find(|(k, _)| *k == token)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

fn close_tag(token: &str) -> &'static str {
    CLOSE_TAGS
        .iter()
        .find(|(k, _)| *k == token)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

/// Using `Rc<dyn Fn>` so that parsers can be cloned (needed for grammar reuse).
type Parser = Rc<dyn Fn(&str) -> Vec<ParseResult>>;

/// `open(tag)` — recognises the opening markdown token and, on success,
/// produces the corresponding HTML open tag.
fn parse_open(token: &'static str) -> Parser {
    Rc::new(move |input: &str| {
        if let Some(rest) = input.strip_prefix(token) {
            vec![ParseResult {
                consumed: open_tag(token).to_string(),
                left: rest.to_string(),
            }]
        } else {
            vec![]
        }
    })
}

/// `close(tag)` — recognises the closing markdown token and, on success,
/// produces the corresponding HTML close tag.
fn parse_close(token: &'static str) -> Parser {
    Rc::new(move |input: &str| {
        if let Some(rest) = input.strip_prefix(token) {
            vec![ParseResult {
                consumed: close_tag(token).to_string(),
                left: rest.to_string(),
            }]
        } else {
            vec![]
        }
    })
}

/// `not_markdown()` — consumes plain text up to the next `*` or `_` character.
fn parse_not_markdown() -> Parser {
    Rc::new(|input: &str| {
        for (i, ch) in input.char_indices() {
            if ch == '*' || ch == '_' {
                return vec![ParseResult {
                    consumed: input[..i].to_string(),
                    left: input[i..].to_string(),
                }];
            }
        }
        if !input.is_empty() {
            vec![ParseResult {
                consumed: input.to_string(),
                left: String::new(),
            }]
        } else {
            vec![]
        }
    })
}

/// `or` — try parsers in order; return the first non-empty result (PEG).
///
/// Earlier combinators collected every successful parse and the caller
/// iterated all of them, which made ambiguous grammars like ours explode
/// exponentially on inputs such as `*_**__…`. Switching to PEG
/// first-match-wins keeps the parse linear in the input length.
fn parse_or(parsers: Vec<Parser>) -> Parser {
    Rc::new(move |input: &str| {
        for p in &parsers {
            if let Some(first) = p(input).into_iter().next() {
                return vec![first];
            }
        }
        vec![]
    })
}

/// `and` — apply parsers in sequence; every parser must consume something.
///
/// Uses PEG semantics: only the first successful result is kept at each
/// step, avoiding the cartesian-product explosion of the original
/// combinator that collected every alternative.
fn parse_and(parsers: Vec<Parser>) -> Parser {
    Rc::new(move |input: &str| {
        let mut current = ParseResult {
            consumed: String::new(),
            left: input.to_string(),
        };

        for p in &parsers {
            let Some(parsed) = p(&current.left)
                .into_iter()
                .find(|x| !x.consumed.is_empty())
            else {
                return vec![];
            };
            current = ParseResult {
                consumed: format!("{}{}", current.consumed, parsed.consumed),
                left: parsed.left,
            };
        }
        vec![current]
    })
}

/// `some` — apply a parser one or more times (recursive).
fn parse_some(parser: Parser) -> Parser {
    Rc::new(move |input: &str| recursive(input, &parser, 0))
}

fn recursive(input: &str, parser: &Parser, depth: usize) -> Vec<ParseResult> {
    // Hard depth bound as a safety net. The single-result invariant from
    // parse_or/parse_and already gives linear-time behaviour; this cap
    // guards against pathological inputs that could still produce deep
    // recursion (e.g. very long plain-text runs that consume one char
    // per step).
    const MAX_RECURSION_DEPTH: usize = 4096;
    if depth >= MAX_RECURSION_DEPTH {
        return vec![ParseResult {
            consumed: String::new(),
            left: input.to_string(),
        }];
    }

    let Some(item) = parser(input).into_iter().find(|x| !x.consumed.is_empty()) else {
        // No match: at top level the whole parse failed; deeper levels
        // return an identity (zero-consumed) result so the parent chain
        // can include whatever was consumed so far.
        if depth == 0 {
            return vec![];
        }
        return vec![ParseResult {
            consumed: String::new(),
            left: input.to_string(),
        }];
    };

    // Try to extend by recursing on the remainder.
    let children = recursive(&item.left, parser, depth + 1);
    if children.is_empty() {
        return vec![item];
    }
    children
        .into_iter()
        .map(|child| ParseResult {
            consumed: format!("{}{}", item.consumed, child.consumed),
            left: child.left,
        })
        .collect()
}

/// Build the top-level inline markdown parser.
fn markdown_parser() -> Parser {
    // text = notMarkdown
    let text = parse_not_markdown();

    // italicNoBold = or(
    //     and(open("*"), text, close("*")),
    //     and(open("_"), text, close("_")),
    // )
    let italic_no_bold = parse_or(vec![
        parse_and(vec![
            parse_open("*"),
            parse_not_markdown(),
            parse_close("*"),
        ]),
        parse_and(vec![
            parse_open("_"),
            parse_not_markdown(),
            parse_close("_"),
        ]),
    ]);

    // bold = or(
    //     and(open("**"), some(or(text, italicNoBold)), close("**")),
    //     and(open("__"), some(or(text, italicNoBold)), close("__")),
    // )
    let bold = parse_or(vec![
        parse_and(vec![
            parse_open("**"),
            parse_some(parse_or(vec![parse_not_markdown(), italic_no_bold.clone()])),
            parse_close("**"),
        ]),
        parse_and(vec![
            parse_open("__"),
            parse_some(parse_or(vec![parse_not_markdown(), italic_no_bold])),
            parse_close("__"),
        ]),
    ]);

    // italic = or(
    //     and(open("*"), some(or(text, bold)), close("*")),
    //     and(open("_"), some(or(text, bold)), close("_")),
    // )
    let italic = parse_or(vec![
        parse_and(vec![
            parse_open("*"),
            parse_some(parse_or(vec![parse_not_markdown(), bold.clone()])),
            parse_close("*"),
        ]),
        parse_and(vec![
            parse_open("_"),
            parse_some(parse_or(vec![parse_not_markdown(), bold.clone()])),
            parse_close("_"),
        ]),
    ]);

    // span = or(bold, italic, text)
    // result = some(span)
    parse_some(parse_or(vec![bold, italic, text]))
}

// ---------------------------------------------------------------------------
// Public API — MarkdownToHTML
// ---------------------------------------------------------------------------

/// Convert markdown to Telegram-supported HTML subset.
///
/// Handles inline `*`/`_` → `<i>`, `**`/`__` → `<b>`, backtick code blocks,
/// and `#` headers.
///
/// Inputs larger than [`MAX_MARKDOWN_HTML_INPUT`] bypass the parser and
/// are returned HTML-escaped only — this is a hard ceiling to bound work
/// on attacker-controlled content (F2). The parser itself is linear-time
/// under PEG semantics, so the cap is a secondary guard.
pub const MAX_MARKDOWN_HTML_INPUT: usize = 64 * 1024;

/// Convert a markdown string to sanitized HTML. Inputs larger than
/// [`MAX_MARKDOWN_HTML_INPUT`] are HTML-escaped without parsing to bound work
/// on hostile content.
pub fn markdown_to_html(md: &str) -> String {
    if md.len() > MAX_MARKDOWN_HTML_INPUT {
        return escape_html(md);
    }

    let md_without_code = escape_html(md);

    // Protect code blocks (```...```) and inline code (`...`)
    let (md_without_code, code_placeholders) =
        replace_with_placeholders(&md_without_code, r"(?s)```.*?```", "c0debl0ck");
    let (md_without_code, inline_placeholders) =
        replace_with_placeholders(&md_without_code, r"`[^`]+`", "inl1ne");

    // Split by double-newline; each segment is parsed independently.
    let segments = RE_NEWLINES.split(&md_without_code);
    let processed: Vec<String> = segments
        .map(|segment| {
            let parser = markdown_parser();
            let docs = parser(segment);
            if !docs.is_empty() {
                format!("{}{}", docs[0].consumed, docs[0].left)
            } else {
                segment.to_string()
            }
        })
        .collect();
    let md_without_code = processed.join("\n\n");

    // Restore code blocks
    let mut result = restore_from_placeholders(&md_without_code, &code_placeholders);
    result = restore_from_placeholders(&result, &inline_placeholders);

    // Convert ```...``` → <pre>...</pre>
    result = RE_CODE_BLOCK
        .replace_all(&result, |caps: &regex::Captures<'_>| {
            let inner = caps.get(1).unwrap().as_str().trim();
            format!("<pre>{inner}</pre>")
        })
        .to_string();

    // Convert `...` → <code>...</code>
    result = RE_INLINE_CODE
        .replace_all(&result, "<code>$1</code>")
        .to_string();

    // Convert #+ heading → <b>heading</b>
    result = RE_HEADER.replace_all(&result, "<b>$1</b>").to_string();

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("a & b < c > d"), "a &amp; b &lt; c &gt; d");
        assert_eq!(escape_html("plain"), "plain");
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b>"), "hello");
        assert_eq!(strip_html_tags("no tags"), "no tags");
        assert_eq!(
            strip_html_tags("<b>bold</b> and <i>italic</i>"),
            "bold and italic"
        );
    }

    #[test]
    fn test_replace_and_restore_placeholders() {
        let input = "some ```code``` here";
        let (modified, phs) = replace_with_placeholders(input, r"(?s)```.*?```", "c0de");
        assert!(modified.contains("c0de"));
        let restored = restore_from_placeholders(&modified, &phs);
        assert_eq!(restored, input);
    }

    #[test]
    fn test_markdown_to_html_italic() {
        let result = markdown_to_html("hello *world*");
        assert!(result.contains("<i>world</i>"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_markdown_to_html_bold() {
        let result = markdown_to_html("hello **world**");
        assert!(result.contains("<b>world</b>"));
    }

    #[test]
    fn test_markdown_to_html_bold_underscore() {
        let result = markdown_to_html("hello __world__");
        assert!(result.contains("<b>world</b>"));
    }

    #[test]
    fn test_markdown_to_html_italic_underscore() {
        let result = markdown_to_html("hello _world_");
        assert!(result.contains("<i>world</i>"));
    }

    #[test]
    fn test_markdown_to_html_code_block() {
        let result = markdown_to_html("```\ncode\n```");
        assert!(result.contains("<pre>code</pre>"));
    }

    #[test]
    fn test_markdown_to_html_inline_code() {
        let result = markdown_to_html("use `foo` here");
        assert!(result.contains("<code>foo</code>"));
    }

    #[test]
    fn test_markdown_to_html_header() {
        let result = markdown_to_html("# Title");
        assert!(result.contains("<b>Title</b>"));
    }

    #[test]
    fn test_markdown_to_html_header_h3() {
        let result = markdown_to_html("### Subtitle");
        assert!(result.contains("<b>Subtitle</b>"));
    }

    #[test]
    fn test_markdown_to_html_plain_text_unchanged() {
        let result = markdown_to_html("just plain text");
        assert_eq!(result, "just plain text");
    }

    #[test]
    fn test_markdown_to_html_html_chars_escaped() {
        let result = markdown_to_html("a < b & c > d");
        assert!(result.contains("&lt;"));
        assert!(result.contains("&gt;"));
        assert!(result.contains("&amp;"));
    }

    #[test]
    fn test_markdown_to_html_mixed() {
        let result = markdown_to_html("**bold** and *italic* and `code`");
        assert!(result.contains("<b>bold</b>"));
        assert!(result.contains("<i>italic</i>"));
        assert!(result.contains("<code>code</code>"));
    }

    #[test]
    fn test_parser_not_markdown() {
        let p = parse_not_markdown();
        let results = p("hello*world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "hello");
        assert_eq!(results[0].left, "*world");
    }

    #[test]
    fn test_parser_not_markdown_no_special() {
        let p = parse_not_markdown();
        let results = p("hello world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "hello world");
        assert_eq!(results[0].left, "");
    }

    #[test]
    fn test_parser_open_close() {
        let p = parse_open("**");
        let results = p("**bold**");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "<b>");
        assert_eq!(results[0].left, "bold**");

        let p = parse_close("**");
        let results = p("**rest");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "</b>");
        assert_eq!(results[0].left, "rest");
    }

    #[test]
    fn test_parser_and() {
        let p = parse_and(vec![
            parse_open("*"),
            parse_not_markdown(),
            parse_close("*"),
        ]);
        let results = p("*hello*");
        assert!(!results.is_empty());
        assert_eq!(results[0].consumed, "<i>hello</i>");
    }
}

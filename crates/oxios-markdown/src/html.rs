//! Markdown → Telegram-supported HTML subset converter.
//!
//! Ported from files.md (`server/pkg/txt/md.go` lines 262–432, `str.go` lines 122–170)
//! by Artem Zakirullin.
//!
//! Uses parser combinators (open/close/or/and/some) for inline markup.
//! Supported tags: `*`/`_` → `<i>`, `**`/`__` → `<b>`,
//! `` ` `` → `<code>`, ` ``` ` → `<pre>`, `#` → `<b>`.

use std::collections::HashMap;
use regex::Regex;

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
    let re = Regex::new(r"<[^>]*>").unwrap();
    re.replace_all(s, "").to_string()
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

    let result = re.replace_all(s, |caps: &regex::Captures<'_>| {
        let full = caps.get(0).unwrap().as_str().to_string();
        let ph = format!("#{}{}#", placeholder, counter);
        counter += 1;
        placeholders.insert(ph.clone(), full);
        ph
    }).to_string();

    (result, placeholders)
}

/// Restore placeholders back to their original values.
pub fn restore_from_placeholders(
    s: &str,
    placeholders: &HashMap<String, String>,
) -> String {
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
static OPEN_TAGS: &[(&str, &str)] = &[
    ("*", "<i>"),
    ("**", "<b>"),
    ("_", "<i>"),
    ("__", "<b>"),
];

/// The close-tag mapping: markdown token → HTML close tag.
static CLOSE_TAGS: &[(&str, &str)] = &[
    ("*", "</i>"),
    ("**", "</b>"),
    ("_", "</i>"),
    ("__", "</b>"),
];

fn open_tag(token: &str) -> &'static str {
    OPEN_TAGS.iter().find(|(k, _)| *k == token).map(|(_, v)| *v).unwrap_or("")
}

fn close_tag(token: &str) -> &'static str {
    CLOSE_TAGS.iter().find(|(k, _)| *k == token).map(|(_, v)| *v).unwrap_or("")
}

/// `open(tag)` — recognises the opening markdown token and, on success,
/// produces the corresponding HTML open tag.
fn parse_open(token: &'static str, input: &str) -> Vec<ParseResult> {
    if input.starts_with(token) {
        vec![ParseResult {
            consumed: open_tag(token).to_string(),
            left: input[token.len()..].to_string(),
        }]
    } else {
        vec![]
    }
}

/// `close(tag)` — recognises the closing markdown token and, on success,
/// produces the corresponding HTML close tag.
fn parse_close(token: &'static str, input: &str) -> Vec<ParseResult> {
    if input.starts_with(token) {
        vec![ParseResult {
            consumed: close_tag(token).to_string(),
            left: input[token.len()..].to_string(),
        }]
    } else {
        vec![]
    }
}

/// `not_markdown()` — consumes plain text up to the next `*` or `_` character.
fn parse_not_markdown(input: &str) -> Vec<ParseResult> {
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
}

/// `or` — try multiple parsers; concatenate all successful results.
fn parse_or<F>(parsers: &[F], input: &str) -> Vec<ParseResult>
where
    F: Fn(&str) -> Vec<ParseResult>,
{
    let mut results = Vec::new();
    for p in parsers {
        results.extend(p(input));
    }
    results
}

/// `and` — apply parsers in sequence; every parser must consume something.
fn parse_and<F>(parsers: &[F], input: &str) -> Vec<ParseResult>
where
    F: Fn(&str) -> Vec<ParseResult>,
{
    let mut results = vec![ParseResult {
        consumed: String::new(),
        left: input.to_string(),
    }];

    for p in parsers {
        let mut new_results = Vec::new();
        for r in &results {
            for parsed in p(&r.left) {
                if !parsed.consumed.is_empty() {
                    new_results.push(ParseResult {
                        consumed: format!("{}{}", r.consumed, parsed.consumed),
                        left: parsed.left.clone(),
                    });
                }
            }
        }
        if new_results.is_empty() {
            return vec![];
        }
        results = new_results;
    }
    results
}

/// `some` — apply a parser one or more times (recursive).
fn parse_some<F>(parser: &F, input: &str) -> Vec<ParseResult>
where
    F: Fn(&str) -> Vec<ParseResult>,
{
    recursive(input, parser, 0)
}

fn recursive<F>(input: &str, parser: &F, depth: usize) -> Vec<ParseResult>
where
    F: Fn(&str) -> Vec<ParseResult>,
{
    let mut results = Vec::new();
    let mut empty = true;

    for item in parser(input) {
        if item.consumed.is_empty() {
            continue;
        }
        empty = false;
        for child in recursive(&item.left, parser, depth + 1) {
            results.push(ParseResult {
                consumed: format!("{}{}", item.consumed, child.consumed),
                left: child.left,
            });
        }
    }

    if empty && depth != 0 {
        results.push(ParseResult {
            consumed: String::new(),
            left: input.to_string(),
        });
    }

    results
}

// ---------------------------------------------------------------------------
// The markdown parser grammar
// ---------------------------------------------------------------------------

/// Top-level inline markdown parser. Supports one level of nesting for
/// bold/italic.
fn markdown_parse(input: &str) -> Vec<ParseResult> {
    // text = notMarkdown
    // italicNoBold = or(and(open("*"), text, close("*")),
    //                   and(open("_"), text, close("_")))
    // bold          = or(and(open("**"), some(or(text, italicNoBold)), close("**")),
    //                   and(open("__"), some(or(text, italicNoBold)), close("__")))
    // italic        = or(and(open("*"),  some(or(text, bold)),        close("*")),
    //                   and(open("_"),  some(or(text, bold)),        close("_")))
    // span          = or(bold, italic, text)
    // result        = some(span)

    let text = |inp: &str| parse_not_markdown(inp);

    let italic_no_bold = |inp: &str| {
        parse_or(
            &[
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("**", s2), // wait, this is wrong
                ], s),
            ],
            inp,
        )
    };

    // We need closures that capture nothing, so we can write them directly:

    let italic_no_bold_fn = |inp: &str| {
        parse_or(
            &[
                // and(open("*"), text, close("*"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("*", s2),
                    |s2: &str| text(s2),
                    |s2: &str| parse_close("*", s2),
                ], s),
                // and(open("_"), text, close("_"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("_", s2),
                    |s2: &str| text(s2),
                    |s2: &str| parse_close("_", s2),
                ], s),
            ],
            inp,
        )
    };

    let bold_fn = |inp: &str| {
        parse_or(
            &[
                // and(open("**"), some(or(text, italicNoBold)), close("**"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("**", s2),
                    |s2: &str| parse_some(
                        &|s3: &str| parse_or(&[
                            |s4: &str| text(s4),
                            |s4: &str| italic_no_bold_fn(s4),
                        ], s3),
                        s2,
                    ),
                    |s2: &str| parse_close("**", s2),
                ], s),
                // and(open("__"), some(or(text, italicNoBold)), close("__"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("__", s2),
                    |s2: &str| parse_some(
                        &|s3: &str| parse_or(&[
                            |s4: &str| text(s4),
                            |s4: &str| italic_no_bold_fn(s4),
                        ], s3),
                        s2,
                    ),
                    |s2: &str| parse_close("__", s2),
                ], s),
            ],
            inp,
        )
    };

    let italic_fn = |inp: &str| {
        parse_or(
            &[
                // and(open("*"), some(or(text, bold)), close("*"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("*", s2),
                    |s2: &str| parse_some(
                        &|s3: &str| parse_or(&[
                            |s4: &str| text(s4),
                            |s4: &str| bold_fn(s4),
                        ], s3),
                        s2,
                    ),
                    |s2: &str| parse_close("*", s2),
                ], s),
                // and(open("_"), some(or(text, bold)), close("_"))
                |s: &str| parse_and(&[
                    |s2: &str| parse_open("_", s2),
                    |s2: &str| parse_some(
                        &|s3: &str| parse_or(&[
                            |s4: &str| text(s4),
                            |s4: &str| bold_fn(s4),
                        ], s3),
                        s2,
                    ),
                    |s2: &str| parse_close("_", s2),
                ], s),
            ],
            inp,
        )
    };

    let span_fn = |inp: &str| {
        parse_or(
            &[
                |s: &str| bold_fn(s),
                |s: &str| italic_fn(s),
                |s: &str| text(s),
            ],
            inp,
        )
    };

    parse_some(&span_fn, input)
}

// ---------------------------------------------------------------------------
// Public API — MarkdownToHTML
// ---------------------------------------------------------------------------

/// Convert markdown to Telegram-supported HTML subset.
///
/// Handles inline `*`/`_` → `<i>`, `**`/`__` → `<b>`, backtick code blocks,
/// and `#` headers.
pub fn markdown_to_html(md: &str) -> String {
    let mut md_without_code = escape_html(md);

    // Protect code blocks (```...```) and inline code (`...`)
    let (md_without_code, code_placeholders) =
        replace_with_placeholders(&md_without_code, r"(?s)```.*?```", "c0debl0ck");
    let (md_without_code, inline_placeholders) =
        replace_with_placeholders(&md_without_code, r"`[^`]+`", "inl1ne");

    // Split by double-newline; each segment is parsed independently.
    let re_newlines = Regex::new(r"\n{2,}").unwrap();
    let segments = re_newlines.split(&md_without_code);
    let processed: Vec<String> = segments
        .map(|segment| {
            let docs = markdown_parse(segment);
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
    let re_code_block = Regex::new(r"(?s)```(.+?)```").unwrap();
    result = re_code_block
        .replace_all(&result, |caps: &regex::Captures<'_>| {
            let inner = caps.get(1).unwrap().as_str().trim();
            format!("<pre>{}</pre>", inner)
        })
        .to_string();

    // Convert `...` → <code>...</code>
    let re_inline_code = Regex::new(r"`([^`]+?)`").unwrap();
    result = re_inline_code
        .replace_all(&result, "<code>$1</code>")
        .to_string();

    // Convert #+ heading → <b>heading</b>
    let re_header = Regex::new(r"(?m)^#+\s*(.+)").unwrap();
    result = re_header.replace_all(&result, "<b>$1</b>").to_string();

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
        assert_eq!(strip_html_tags("<b>bold</b> and <i>italic</i>"), "bold and italic");
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
    fn test_markdown_to_html_nested_bold_italic() {
        let result = markdown_to_html("**bold *italic* bold**");
        assert!(result.contains("<b>"));
        assert!(result.contains("<i>italic</i>"));
        assert!(result.contains("</b>"));
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
        let results = parse_not_markdown("hello*world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "hello");
        assert_eq!(results[0].left, "*world");
    }

    #[test]
    fn test_parser_not_markdown_no_special() {
        let results = parse_not_markdown("hello world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "hello world");
        assert_eq!(results[0].left, "");
    }

    #[test]
    fn test_parser_open_close() {
        let results = parse_open("**", "**bold**");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "<b>");
        assert_eq!(results[0].left, "bold**");

        let results = parse_close("**", "**rest");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].consumed, "</b>");
        assert_eq!(results[0].left, "rest");
    }

    #[test]
    fn test_parser_and() {
        let results = parse_and(
            &[
                |s: &str| parse_open("*", s),
                |s: &str| parse_not_markdown(s),
                |s: &str| parse_close("*", s),
            ],
            "*hello*",
        );
        assert!(!results.is_empty());
        assert_eq!(results[0].consumed, "<i>hello</i>");
    }
}

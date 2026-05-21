//! tgtxt: Telegram text processing — extract text, images, and links from markdown.
//!
//! Ported from files.md (`server/pkg/txt/tgtxt.go`) by Artem Zakirullin.
//!
//! Note: `TelegramEntitiesToMarkdown` is not ported because it depends on
//! Oxios Telegram channel types. Only `extract_text_imgs_links` is provided.

use std::collections::HashMap;

use regex::Regex;

/// Result of extracting text, images, and links from markdown.
#[derive(Debug, Clone)]
pub struct ExtractResult {
    /// The cleaned text (images replaced with 🖼, links replaced with backtick labels).
    pub text: String,
    /// Image IDs extracted from `![...](tg_ID.ext)` patterns.
    pub images: Vec<String>,
    /// Links extracted from `[...](path)` and `[[path]]` patterns (label → path).
    pub links: HashMap<String, String>,
}

/// Extract text, images, and links from markdown content.
///
/// Processing order:
/// 1. Lines containing only a link or wiki-link are removed (link info goes to `links`)
/// 2. Image markdown `![...](tg_ID.ext)` → 🖼, ID goes to `images`
/// 3. Inline links `[label](path)` → `` `label` ``, path goes to `links`
/// 4. Wiki links `[[path]]` / `[[path|label]]` → `` `label` ``, path goes to `links`
pub fn extract_text_imgs_links(text: &str) -> ExtractResult {
    let text = crate::parser::norm_new_lines(text);
    let mut images: Vec<String> = Vec::new();
    let mut links: HashMap<String, String> = HashMap::new();

    let img_re = Regex::new(r"!\[.*?\]\(.*?tg_([^.]+)\..*?\)").unwrap();
    let link_re = Regex::new(r"\[.*?\]\((.+?)\)").unwrap();
    let wiki_re = Regex::new(r"\[\[(.+?)\]\]").unwrap();

    // Phase 1: remove lines that contain only a link
    let lines: Vec<&str> = text.split('\n').collect();
    let mut kept_lines: Vec<String> = Vec::new();
    for line in &lines {
        let trimmed = line.trim();

        // Link-only line
        if link_re.is_match(trimmed) && link_re.find(trimmed).map(|m| m.as_str()) == Some(trimmed) {
            if let Some(caps) = link_re.captures(line) {
                let content = caps.get(1).unwrap().as_str();
                let (link_path, link_label) = split_link_content(content, false);
                links.insert(link_label, link_path);
            }
            continue;
        }

        // Wiki-link-only line
        if wiki_re.is_match(trimmed) && wiki_re.find(trimmed).map(|m| m.as_str()) == Some(trimmed) {
            if let Some(caps) = wiki_re.captures(line) {
                let content = caps.get(1).unwrap().as_str();
                let (link_path, link_label) = split_link_content(content, true);
                links.insert(link_label, link_path);
            }
            continue;
        }

        kept_lines.push((*line).to_string());
    }
    let mut text = kept_lines.join("\n");

    // Phase 2: replace images
    text = img_re
        .replace_all(&text, |caps: &regex::Captures| {
            if let Some(id) = caps.get(1) {
                images.push(id.as_str().to_string());
            }
            "🖼"
        })
        .to_string();

    // Phase 3: replace inline links
    text = link_re
        .replace_all(&text, |caps: &regex::Captures| {
            if let Some(m) = caps.get(1) {
                let content = m.as_str();
                let (link_path, link_label) = split_link_content(content, false);
                links.insert(link_label.clone(), link_path);
                format!("`{}`", link_label)
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    // Phase 4: replace wiki links
    text = wiki_re
        .replace_all(&text, |caps: &regex::Captures| {
            if let Some(m) = caps.get(1) {
                let content = m.as_str();
                let (link_path, link_label) = split_link_content(content, true);
                links.insert(link_label.clone(), link_path);
                format!("`{}`", link_label)
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    ExtractResult {
        text: text.trim().to_string(),
        images,
        links,
    }
}

/// Split a link content into (path, label).
/// For wiki links, if there's a `|`, the part after is the label.
/// For markdown links, the content is the URL/path.
fn split_link_content(content: &str, is_wiki: bool) -> (String, String) {
    if is_wiki {
        let parts: Vec<&str> = content.splitn(2, '|').collect();
        let path = format!("{}.md", parts[0]);
        let label = parts.get(1).map(|s| (*s).to_string()).unwrap_or_else(|| {
            std::path::Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
        (path, label)
    } else {
        let path = content.to_string();
        let label = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        (path, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_images() {
        let md = "Hello ![photo](photos/tg_abc123.jpg) world";
        let result = extract_text_imgs_links(md);
        assert_eq!(result.images, vec!["abc123"]);
        assert!(result.text.contains("🖼"));
    }

    #[test]
    fn test_extract_inline_links() {
        let md = "See [Rust](brain/Rust.md) for details";
        let result = extract_text_imgs_links(md);
        assert!(result.text.contains("`Rust`"));
        assert_eq!(result.links.get("Rust"), Some(&"brain/Rust.md".to_string()));
    }

    #[test]
    fn test_extract_wiki_links() {
        let md = "See [[brain/Rust]] for details";
        let result = extract_text_imgs_links(md);
        assert!(result.text.contains("`Rust`"));
        assert_eq!(result.links.get("Rust"), Some(&"brain/Rust.md".to_string()));
    }

    #[test]
    fn test_extract_wiki_links_with_label() {
        let md = "See [[brain/Rust|The Rust Page]] for details";
        let result = extract_text_imgs_links(md);
        assert!(result.text.contains("`The Rust Page`"));
        assert_eq!(
            result.links.get("The Rust Page"),
            Some(&"brain/Rust.md".to_string())
        );
    }

    #[test]
    fn test_link_only_line_removed() {
        let md = "Some text\n[My Note](notes/MyNote.md)\nMore text";
        let result = extract_text_imgs_links(md);
        assert!(result.text.contains("Some text"));
        assert!(result.text.contains("More text"));
        assert!(!result.text.contains("My Note"));
        assert_eq!(
            result.links.get("MyNote"),
            Some(&"notes/MyNote.md".to_string())
        );
    }

    #[test]
    fn test_wiki_link_only_line_removed() {
        let md = "Some text\n[[notes/MyNote]]\nMore text";
        let result = extract_text_imgs_links(md);
        assert!(!result.text.contains("[[notes/MyNote]]"));
        assert_eq!(
            result.links.get("MyNote"),
            Some(&"notes/MyNote.md".to_string())
        );
    }

    #[test]
    fn test_empty_input() {
        let result = extract_text_imgs_links("");
        assert!(result.text.is_empty());
        assert!(result.images.is_empty());
        assert!(result.links.is_empty());
    }

    #[test]
    fn test_plain_text_unchanged() {
        let md = "Just some plain text\nwith no links or images";
        let result = extract_text_imgs_links(md);
        assert_eq!(result.text, md);
        assert!(result.images.is_empty());
        assert!(result.links.is_empty());
    }
}

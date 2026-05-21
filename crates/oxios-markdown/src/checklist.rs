//! Checklist engine for markdown content.
//!
//! Ported from files.md (`server/pkg/txt/md.go` lines 95–261) by Artem Zakirullin.
//! Provides functions to parse, add, complete, and remove `- [ ]` / `- [x]` checklist items.

use std::collections::HashMap;

use crate::fs::hash_filename;
use crate::parser::norm_new_lines;

/// Parse checklist items from markdown.
///
/// Returns an ordered list of item texts and a map indicating which items are completed.
///
/// ```
/// use oxios_markdown::checklist::checklist_items;
/// let md = "- [ ] Buy milk\n- [x] Write code\n";
/// let (items, completed) = checklist_items(md);
/// assert_eq!(items, vec!["Buy milk", "Write code"]);
/// assert!(!completed["Buy milk"]);
/// assert!(completed["Write code"]);
/// ```
pub fn checklist_items(md: &str) -> (Vec<String>, HashMap<String, bool>) {
    let mut items = Vec::new();
    let mut is_completed = HashMap::new();

    for line in md.lines() {
        let line = line.trim();
        if let Some(item) = line.strip_prefix("- [ ] ") {
            items.push(item.to_string());
            is_completed.insert(item.to_string(), false);
        } else if let Some(item) = line.strip_prefix("- [x] ") {
            items.push(item.to_string());
            is_completed.insert(item.to_string(), true);
        }
    }

    (items, is_completed)
}

/// Get incomplete checklist items only.
///
/// ```
/// use oxios_markdown::checklist::incomplete_checklist_items;
/// let md = "- [ ] Task A\n- [x] Task B\n- [ ] Task C\n";
/// let incomplete = incomplete_checklist_items(md);
/// assert_eq!(incomplete, vec!["Task A", "Task C"]);
/// ```
pub fn incomplete_checklist_items(md: &str) -> Vec<String> {
    let (items, is_completed) = checklist_items(md);
    items
        .into_iter()
        .filter(|item| !is_completed[item])
        .collect()
}

/// Add a checklist item. Removes duplicate if exists.
///
/// When `checked` is false the item is inserted *before* the first existing
/// incomplete item, keeping all incomplete items grouped at the bottom.
/// Newlines in `item` are converted to spaces.
///
/// ```
/// use oxios_markdown::checklist::add_checklist_item;
/// let md = "- [ ] Existing task\n";
/// let result = add_checklist_item(md, "New task", false);
/// assert!(result.contains("- [ ] New task"));
/// assert!(result.contains("- [ ] Existing task"));
/// ```
pub fn add_checklist_item(md: &str, item: &str, checked: bool) -> String {
    // Normalise newlines → spaces
    let item = norm_new_lines(item).replace('\n', " ");
    let item = item.trim();

    // Remove existing copy (if any)
    let (md, _) = remove_checklist_item(md, item);
    let mut lines: Vec<String> = md.lines().map(|l| l.to_string()).collect();

    if checked {
        lines.push(format!("- [x] {}", item));
    } else {
        // Find the first incomplete item and insert before it
        let mut insert_index = lines.len();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ] ") {
                insert_index = i;
                break;
            }
        }

        if insert_index == lines.len() {
            lines.push(format!("- [ ] {}", item));
        } else {
            lines.insert(insert_index, format!("- [ ] {}", item));
        }
    }

    lines.join("\n").trim().to_string()
}

/// Complete a checklist item by hash. Returns `(new_md, completed_item_text)`.
///
/// The marker stays in place (does not relocate to the bottom) to avoid
/// breaking multi-line records.
///
/// ```
/// use oxios_markdown::checklist::complete_checklist_item;
/// use oxios_markdown::fs::hash_filename;
/// let md = "- [ ] Buy milk\n- [ ] Write code\n";
/// let hash = hash_filename("Buy milk");
/// let (new_md, completed) = complete_checklist_item(md, &hash);
/// assert_eq!(completed, "Buy milk");
/// assert!(new_md.contains("- [x] Buy milk"));
/// ```
pub fn complete_checklist_item(md: &str, item_hash: &str) -> (String, String) {
    let mut found_item = String::new();
    let mut lines: Vec<String> = md.lines().map(|l| l.to_string()).collect();
    let mut found_index: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.len() < 6 {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            if hash_filename(rest) == item_hash {
                found_item = rest.to_string();
                found_index = Some(i);
                break;
            }
        }
    }

    if let Some(idx) = found_index {
        lines[idx] = format!("- [x] {}", found_item);
    }

    (lines.join("\n"), found_item)
}

/// Remove a checklist item by item text or hash. Returns `(new_md, removed_item)`.
///
/// ```
/// use oxios_markdown::checklist::remove_checklist_item;
/// let md = "- [ ] Task A\n- [x] Task B\n";
/// let (new_md, removed) = remove_checklist_item(md, "Task A");
/// assert_eq!(removed, "Task A");
/// assert!(!new_md.contains("Task A"));
/// assert!(new_md.contains("Task B"));
/// ```
pub fn remove_checklist_item(md: &str, item_or_hash: &str) -> (String, String) {
    let mut removed_item = String::new();
    let mut new_lines: Vec<String> = Vec::new();

    for line in md.lines() {
        let trimmed = line.trim();
        // Preserve lines that are too short to be checklist items
        if trimmed.len() < 6 {
            new_lines.push(line.to_string());
            continue;
        }

        // Both "- [ ] X" and "- [x] X" have a 6-char prefix before the item text
        let rest = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("- [x] "));

        if let Some(rest) = rest {
            if hash_filename(rest) == item_or_hash || rest == item_or_hash {
                removed_item = rest.to_string();
                continue; // skip this line
            }
        }

        new_lines.push(line.to_string());
    }

    (new_lines.join("\n"), removed_item)
}

/// Remove all completed checklist items. Returns `(new_md, removed_md)`.
///
/// ```
/// use oxios_markdown::checklist::remove_completed_checklist_items;
/// let md = "- [ ] Task A\n- [x] Task B\n- [x] Task C\n";
/// let (new_md, removed) = remove_completed_checklist_items(md);
/// assert!(new_md.contains("Task A"));
/// assert!(!new_md.contains("Task B"));
/// assert!(removed.contains("Task B"));
/// ```
pub fn remove_completed_checklist_items(md: &str) -> (String, String) {
    let mut removed_lines: Vec<String> = Vec::new();
    let mut new_lines: Vec<String> = Vec::new();

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- [x] ") {
            removed_lines.push(trimmed.to_string());
        } else {
            new_lines.push(line.to_string());
        }
    }

    let removed_md = if removed_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", removed_lines.join("\n"))
    };

    (new_lines.join("\n"), removed_md)
}

/// Find a single checklist item by text or hash.
///
/// Returns the item text if found, empty string otherwise.
///
/// ```
/// use oxios_markdown::checklist::checklist_item;
/// let md = "- [ ] Task A\n- [x] Task B\n";
/// assert_eq!(checklist_item(md, "Task B"), "Task B");
/// assert_eq!(checklist_item(md, "nonexistent"), "");
/// ```
pub fn checklist_item(md: &str, item_or_hash: &str) -> String {
    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.len() < 6 {
            continue;
        }

        let rest = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("- [x] "));

        if let Some(rest) = rest {
            if hash_filename(rest) == item_or_hash || rest == item_or_hash {
                return rest.to_string();
            }
        }
    }

    String::new()
}

/// Insert text under a header in markdown content.
///
/// If the header doesn't exist yet it is created at the top.
/// If the header exists, `new_content` is inserted right after it
/// (before the next `###` sub-header).
///
/// ```
/// use oxios_markdown::checklist::add_header_and_text;
/// let content = "### Notes\nSome notes\n";
/// let result = add_header_and_text(content, "### Notes", "Extra line");
/// assert!(result.contains("Extra line"));
/// ```
pub fn add_header_and_text(content: &str, header: &str, new_content: &str) -> String {
    if !content.contains(header) {
        if content.is_empty() {
            return format!("{}\n{}", header, new_content);
        } else {
            return format!("{}\n{}\n\n{}", header, new_content, content);
        }
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut header_index: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        if *line == header {
            header_index = Some(i);
            break;
        }
    }

    let header_index = match header_index {
        Some(idx) => idx,
        None => return format!("{}\n{}\n\n{}", header, new_content, content),
    };

    // Find where to insert (after the last line belonging to this header,
    // stopping at the next ### sub-header).
    let mut insert_index = header_index + 1;

    for (i, line) in lines.iter().enumerate().skip(header_index + 1) {
        if line.starts_with("###") {
            insert_index = i;
            break;
        }
        insert_index = i + 1;
    }

    // Build the new content
    let mut new_lines: Vec<String> = Vec::with_capacity(lines.len() + 2);
    for line in &lines[..insert_index] {
        new_lines.push(line.to_string());
    }
    new_lines.push(new_content.to_string());

    // Add empty line after new content if there's content following and it's not empty
    if insert_index < lines.len() && !lines[insert_index].trim().is_empty() {
        new_lines.push(String::new());
    }

    for line in &lines[insert_index..] {
        new_lines.push(line.to_string());
    }

    new_lines.join("\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::hash_filename;

    #[test]
    fn test_checklist_items() {
        let md = "- [ ] Buy milk\n- [x] Write code\n- [ ] Read book\n";
        let (items, completed) = checklist_items(md);
        assert_eq!(items, vec!["Buy milk", "Write code", "Read book"]);
        assert!(!completed["Buy milk"]);
        assert!(completed["Write code"]);
        assert!(!completed["Read book"]);
    }

    #[test]
    fn test_checklist_items_empty() {
        let md = "No checklist here\nJust text\n";
        let (items, completed) = checklist_items(md);
        assert!(items.is_empty());
        assert!(completed.is_empty());
    }

    #[test]
    fn test_incomplete_checklist_items() {
        let md = "- [ ] Task A\n- [x] Task B\n- [ ] Task C\n";
        let incomplete = incomplete_checklist_items(md);
        assert_eq!(incomplete, vec!["Task A", "Task C"]);
    }

    #[test]
    fn test_add_checklist_item_unchecked() {
        let md = "- [ ] Existing task\n";
        let result = add_checklist_item(md, "New task", false);
        // New unchecked item should be inserted before existing unchecked
        let lines: Vec<&str> = result.lines().collect();
        let new_pos = lines.iter().position(|l| l.contains("New task")).unwrap();
        let existing_pos = lines
            .iter()
            .position(|l| l.contains("Existing task"))
            .unwrap();
        assert!(new_pos < existing_pos);
    }

    #[test]
    fn test_add_checklist_item_checked() {
        let md = "- [ ] Task A\n";
        let result = add_checklist_item(md, "Done task", true);
        assert!(result.contains("- [x] Done task"));
        assert!(result.contains("- [ ] Task A"));
    }

    #[test]
    fn test_add_checklist_item_removes_duplicate() {
        let md = "- [ ] Task A\n- [ ] Task B\n";
        let result = add_checklist_item(md, "Task A", false);
        // Should only appear once
        assert_eq!(result.matches("Task A").count(), 1);
    }

    #[test]
    fn test_add_checklist_item_newlines_to_spaces() {
        let md = "- [ ] Existing\n";
        let result = add_checklist_item(md, "Multi\nline\nitem", false);
        assert!(result.contains("- [ ] Multi line item"));
    }

    #[test]
    fn test_complete_checklist_item() {
        let md = "- [ ] Buy milk\n- [ ] Write code\n";
        let hash = hash_filename("Buy milk");
        let (new_md, completed) = complete_checklist_item(md, &hash);
        assert_eq!(completed, "Buy milk");
        assert!(new_md.contains("- [x] Buy milk"));
        assert!(new_md.contains("- [ ] Write code"));
    }

    #[test]
    fn test_complete_checklist_item_not_found() {
        let md = "- [ ] Buy milk\n";
        let (new_md, completed) = complete_checklist_item(md, "nonexistent_hash");
        assert_eq!(completed, "");
        assert_eq!(new_md, md.trim_end());
    }

    #[test]
    fn test_remove_checklist_item_by_text() {
        let md = "- [ ] Task A\n- [x] Task B\n";
        let (new_md, removed) = remove_checklist_item(md, "Task A");
        assert_eq!(removed, "Task A");
        assert!(!new_md.contains("Task A"));
        assert!(new_md.contains("Task B"));
    }

    #[test]
    fn test_remove_checklist_item_by_hash() {
        let md = "- [ ] Task A\n- [x] Task B\n";
        let hash = hash_filename("Task A");
        let (new_md, removed) = remove_checklist_item(md, &hash);
        assert_eq!(removed, "Task A");
        assert!(!new_md.contains("Task A"));
    }

    #[test]
    fn test_remove_checklist_item_not_found() {
        let md = "- [ ] Task A\n";
        let (new_md, removed) = remove_checklist_item(md, "nonexistent");
        assert_eq!(removed, "");
        assert!(new_md.contains("Task A"));
    }

    #[test]
    fn test_remove_completed_checklist_items() {
        let md = "- [ ] Task A\n- [x] Task B\n- [x] Task C\nSome text\n";
        let (new_md, removed) = remove_completed_checklist_items(md);
        assert!(new_md.contains("Task A"));
        assert!(!new_md.contains("Task B"));
        assert!(!new_md.contains("Task C"));
        assert!(new_md.contains("Some text"));
        assert!(removed.contains("- [x] Task B"));
        assert!(removed.contains("- [x] Task C"));
    }

    #[test]
    fn test_checklist_item_by_text() {
        let md = "- [ ] Task A\n- [x] Task B\n";
        assert_eq!(checklist_item(md, "Task B"), "Task B");
    }

    #[test]
    fn test_checklist_item_by_hash() {
        let md = "- [ ] Task A\n";
        let hash = hash_filename("Task A");
        assert_eq!(checklist_item(md, &hash), "Task A");
    }

    #[test]
    fn test_checklist_item_not_found() {
        let md = "- [ ] Task A\n";
        assert_eq!(checklist_item(md, "nonexistent"), "");
    }

    #[test]
    fn test_add_header_and_text_new_header() {
        let content = "Existing content\n";
        let result = add_header_and_text(content, "## New Header", "New text");
        assert!(result.starts_with("## New Header\nNew text\n\nExisting content"));
    }

    #[test]
    fn test_add_header_and_text_existing_header() {
        let content = "### Notes\nSome notes\n### Other\nOther content";
        let result = add_header_and_text(content, "### Notes", "Extra line");
        // Go behavior: inserts new content after existing content under the header, before next ### header
        assert!(result.contains("Some notes\nExtra line"));
        assert!(result.contains("### Other"));
    }

    #[test]
    fn test_add_header_and_text_empty_content() {
        let result = add_header_and_text("", "## Header", "Text");
        assert_eq!(result, "## Header\nText");
    }

    #[test]
    fn test_add_checklist_item_to_empty() {
        let result = add_checklist_item("", "First task", false);
        assert_eq!(result, "- [ ] First task");
    }
}

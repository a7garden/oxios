//! Chat/Inbox file management.
//!
//! Ported from files.md (`server/chat/mod.rs`) by Artem Zakirullin.
//! Handles the Chat.md file: parsing blocks, finding/renaming/deleting entries.

use chrono::Datelike;
use regex::Regex;
use crate::fs::hash_filename;
use crate::parser::norm_new_lines;

/// Strip the `- [ ]` / `- [x]` prefix and optional `` `HH:MM` `` timestamp.
pub fn strip_inbox_entry_prefix(block: &str) -> String {
    let re = Regex::new(r"^- \[[ xX]\] (?:`\d{2}:\d{2}` )?").unwrap();
    re.replace(block, "").to_string()
}

/// Compute a stable hash for a chat block (based on content after stripping marker).
pub fn chat_block_hash(block: &str) -> String {
    let stripped = Regex::new(r"^- \[[ xX]\] ").unwrap().replace_all(block, "");
    let first_line = stripped.splitn(2, '\n').next().unwrap_or("");
    hash_filename(first_line)
}

/// Parse chat content into logical blocks (headers and messages).
pub fn read_chat_msgs(content: &str) -> Vec<String> {
    let content = norm_new_lines(content);
    let header_re = Regex::new(r"^#### ").unwrap();
    let marker_re = Regex::new(r"^- \[[ xX]\] ").unwrap();

    let lines: Vec<&str> = content.split('\n').collect();
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();

    for line in lines {
        let is_header = header_re.is_match(line);
        let is_marker = marker_re.is_match(line);

        if is_header || is_marker {
            if !current.is_empty() {
                blocks.push(current.trim().to_string());
                current = String::new();
            }
            current.push_str(line);
        } else if !current.is_empty() {
            current.push('\n');
            current.push_str(line);
        } else {
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current.trim().to_string());
    }
    blocks
}

/// Find a chat message by its content hash.
pub fn find_chat_msg_by_hash(content: &str, msg_hash: &str) -> Option<(usize, String)> {
    let blocks = read_chat_msgs(content);
    let header_re = Regex::new(r"^#### ").unwrap();
    for (i, block) in blocks.iter().enumerate() {
        if header_re.is_match(block) { continue; }
        if chat_block_hash(block) == msg_hash {
            return Some((i, block.clone()));
        }
    }
    None
}

/// Rename a chat message identified by hash.
pub fn rename_chat_msg(content: &str, msg_hash: &str, new_body: &str) -> Result<String, String> {
    let blocks = read_chat_msgs(content);
    let header_re = Regex::new(r"^#### ").unwrap();
    let prefix_re = Regex::new(r"^- \[[ xX]\] (?:`\d{2}:\d{2}` )?").unwrap();

    let idx = blocks.iter().position(|b| {
        !header_re.is_match(b) && chat_block_hash(b) == msg_hash
    }).ok_or_else(|| format!("chat block not found for hash {:?}", msg_hash))?;

    let prefix = prefix_re.find(&blocks[idx]).map(|m| m.as_str().to_string()).unwrap_or_default();
    let new_body = new_body.trim().replace('\n', " ");
    let mut new_blocks = blocks;
    new_blocks[idx] = format!("{}{}", prefix, new_body);
    Ok(new_blocks.join("\n"))
}

/// Delete a chat message by hash.
pub fn delete_chat_msg(content: &str, msg_hash: &str) -> Result<String, String> {
    let blocks = read_chat_msgs(content);
    let header_re = Regex::new(r"^#### ").unwrap();

    let idx = blocks.iter().position(|b| {
        !header_re.is_match(b) && chat_block_hash(b) == msg_hash
    }).ok_or_else(|| format!("chat block not found for hash {:?}", msg_hash))?;

    let mut new_blocks = blocks;
    new_blocks.remove(idx);
    Ok(new_blocks.join("\n"))
}

/// Generate today's date header for the chat file.
pub fn today_header(timezone: &chrono::FixedOffset) -> String {
    let now_tz = chrono::Utc::now().with_timezone(timezone);
    format!("#### {} {}, {}", now_tz.date_naive().day(), now_tz.format("%B"), now_tz.format("%A"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_prefix() {
        assert_eq!(strip_inbox_entry_prefix("- [ ] `12:34` Task"), "Task");
        assert_eq!(strip_inbox_entry_prefix("- [x] Done"), "Done");
    }

    #[test]
    fn test_read_blocks() {
        let content = "#### 19 May\n- [ ] `09:00` First\n- [x] Done";
        let blocks = read_chat_msgs(content);
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn test_find_by_hash() {
        let content = "#### 19 May\n- [ ] `09:00` First\n- [x] Second";
        let hash = chat_block_hash("- [ ] `09:00` First");
        assert!(find_chat_msg_by_hash(content, &hash).is_some());
    }

    #[test]
    fn test_rename() {
        let content = "#### 19 May\n- [ ] `09:00` Old task\n- [ ] `10:00` Keep";
        let hash = chat_block_hash("- [ ] `09:00` Old task");
        let result = rename_chat_msg(content, &hash, "New task").unwrap();
        assert!(result.contains("- [ ] `09:00` New task"));
        assert!(result.contains("Keep"));
        assert!(!result.contains("Old task"));
    }

    #[test]
    fn test_delete() {
        let content = "#### 19 May\n- [ ] `09:00` Delete me\n- [ ] `10:00` Keep me";
        let hash = chat_block_hash("- [ ] `09:00` Delete me");
        let result = delete_chat_msg(content, &hash).unwrap();
        assert!(!result.contains("Delete me"));
        assert!(result.contains("Keep me"));
    }
}

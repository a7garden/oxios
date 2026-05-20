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

/// Append text to an existing chat message identified by hash.
///
/// Returns `(new_content, true)` on success, or `(original_content, false)` if not found.
/// The appended text becomes a new indented continuation line under the original entry.
pub fn append_to_chat_msg(
    content: &str,
    msg_hash: &str,
    new_text: &str,
) -> Result<String, String> {
    let blocks = read_chat_msgs(content);
    let header_re = Regex::new(r"^#### ").unwrap();

    let idx = blocks.iter().position(|b| {
        !header_re.is_match(b) && chat_block_hash(b) == msg_hash
    });

    let idx = match idx {
        Some(i) => i,
        None => return Err(format!("chat block not found for hash {:?}", msg_hash)),
    };

    let new_text = new_text.trim_end_matches('\n');
    if new_text.is_empty() {
        return Ok(content.to_string());
    }

    let mut new_blocks = blocks;
    let block = new_blocks[idx].trim_end_matches('\n').to_string();
    new_blocks[idx] = format!("{}\n{}", block, new_text);

    Ok(new_blocks.join("\n"))
}

/// Move a chat message to a target file as a checklist item.
///
/// Finds the message by hash in `chat_content`, removes it from chat, and
/// appends it as a `- [ ] ` checklist item to `target_content`.
/// Returns `(new_chat_content, new_target_content)`.
pub fn move_from_chat(
    chat_content: &str,
    msg_hash: &str,
    target_content: &str,
) -> (String, String) {
    // Find the message
    let found = find_chat_msg_by_hash(chat_content, msg_hash);

    match found {
        Some((_idx, block)) => {
            // Strip the inbox entry prefix to get the body
            let body = strip_inbox_entry_prefix(&block);
            let body = body.trim().replace('\n', " ");

            // Remove from chat
            let new_chat = match delete_chat_msg(chat_content, msg_hash) {
                Ok(c) => c,
                Err(_) => chat_content.to_string(),
            };

            // Add as checklist item to target
            let new_target = if target_content.is_empty() {
                format!("- [ ] {}", body)
            } else {
                format!("{}\n- [ ] {}", target_content.trim_end(), body)
            };

            (new_chat, new_target)
        }
        None => (chat_content.to_string(), target_content.to_string()),
    }
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

    #[test]
    fn test_append_to_chat_msg() {
        let content = "#### 19 May\n- [ ] `09:00` Task one\n- [ ] `10:00` Task two";
        let hash = chat_block_hash("- [ ] `09:00` Task one");
        let result = append_to_chat_msg(content, &hash, "added detail").unwrap();
        assert!(result.contains("Task one\nadded detail"));
        assert!(result.contains("Task two"));
    }

    #[test]
    fn test_append_to_chat_msg_not_found() {
        let content = "#### 19 May\n- [ ] `09:00` Task";
        let result = append_to_chat_msg(content, "nonexistent_hash", "text");
        assert!(result.is_err());
    }

    #[test]
    fn test_append_to_chat_msg_empty_text() {
        let content = "#### 19 May\n- [ ] `09:00` Task";
        let hash = chat_block_hash("- [ ] `09:00` Task");
        let result = append_to_chat_msg(content, &hash, "").unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_move_from_chat() {
        let chat = "#### 19 May\n- [ ] `09:00` Move me\n- [ ] `10:00` Stay";
        let hash = chat_block_hash("- [ ] `09:00` Move me");
        let (new_chat, new_target) = move_from_chat(chat, &hash, "- [ ] Existing item");

        // Should be removed from chat
        assert!(!new_chat.contains("Move me"));
        assert!(new_chat.contains("Stay"));

        // Should be added as checklist item in target (timestamp stripped)
        assert!(new_target.contains("- [ ] Existing item"));
        assert!(new_target.contains("- [ ] Move me"));
    }

    #[test]
    fn test_move_from_chat_not_found() {
        let chat = "#### 19 May\n- [ ] `09:00` Task";
        let (new_chat, new_target) = move_from_chat(chat, "nonexistent", "target");
        assert_eq!(new_chat, chat);
        assert_eq!(new_target, "target");
    }

    #[test]
    fn test_move_from_chat_empty_target() {
        let chat = "#### 19 May\n- [ ] `09:00` Move me";
        let hash = chat_block_hash("- [ ] `09:00` Move me");
        let (_new_chat, new_target) = move_from_chat(chat, &hash, "");
        assert!(new_target.contains("- [ ] Move me"));
    }
}

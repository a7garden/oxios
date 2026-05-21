//! i18n: emoji auto-mapping and UI string constants.
//!
//! Ported from files.md (`server/i18n/emoji.go` + `strings.go`) by Artem Zakirullin.

use std::collections::HashMap;

use once_cell::sync::Lazy;

// ── Embedded emoji data ─────────────────────────────────────

static EMOJI_MAP: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let raw: HashMap<String, Vec<String>> = serde_json::from_str(include_str!("data/emojis.json"))
        .expect("Failed to parse embedded emojis.json");

    let mut map = HashMap::new();
    for (emoji, keywords) in raw {
        for kw in keywords {
            map.insert(kw.to_lowercase(), emoji.clone());
        }
    }
    map
});

// ── Public API ──────────────────────────────────────────────

/// Find an emoji for the given keyword.
///
/// Lookup strategy:
/// 1. Exact lowercase match
/// 2. Append "s" (plural)
/// 3. Strip trailing "s" (singular)
/// 4. Match any individual word
pub fn emoji_for(keyword: &str) -> String {
    let lower = keyword.to_lowercase();

    // 1. Exact match
    if let Some(e) = EMOJI_MAP.get(&lower) {
        return e.clone();
    }

    // 2. Plural (add "s")
    let plural = format!("{}s", lower);
    if let Some(e) = EMOJI_MAP.get(&plural) {
        return e.clone();
    }

    // 3. Singular (strip "s")
    if lower.ends_with('s') && lower.len() > 1 {
        let singular = &lower[..lower.len() - 1];
        if let Some(e) = EMOJI_MAP.get(singular) {
            return e.clone();
        }
    }

    // 4. Word-level match
    for word in lower.split_whitespace() {
        if let Some(e) = EMOJI_MAP.get(word) {
            return e.clone();
        }
    }

    String::new()
}

/// Add an emoji prefix to a string if a matching keyword is found.
pub fn add_emoji(s: &str) -> String {
    let e = emoji_for(s);
    if e.is_empty() {
        s.to_string()
    } else {
        format!("{} {}", e, s)
    }
}

// ── UI String Constants ─────────────────────────────────────

/// ⏳ Later
pub const STR_LATER: &str = "⏳";
/// 🏠 Home
pub const STR_HOME: &str = "🏠 Home";
/// ⬅️ Back
pub const STR_BACK: &str = "⬅️ Back";
/// ✅ Complete
pub const STR_COMPLETE: &str = "✅ Complete";
/// ⏳ Move to later
pub const STR_MOVE_TO_LATER_LONG: &str = "⏳ Move to later";
/// ➡️ Move to today
pub const STR_TO_TODAY: &str = "➡️ Move to today";
/// 🌚 To tmrw
pub const STR_TO_TOMORROW: &str = "🌚 To tmrw";
/// ⏳ To later
pub const STR_TO_LATER: &str = "⏳ To later";
/// 📆 To a day
pub const STR_TO_A_DAY: &str = "📆 To a day";
/// ☑️ To Checklist
pub const STR_TO_CHECKLIST: &str = "☑️ To Checklist";
/// 📄 To Note
pub const STR_TO_FILE: &str = "📄 To Note";
/// 💚 To Journal
pub const STR_TO_JOURNAL: &str = "💚 To Journal";
/// 📚 To Read
pub const STR_TO_READ: &str = "📚 To Read";
/// 🛒 To Shop
pub const STR_TO_SHOP: &str = "🛒 To Shop";
/// 📺 To Watch
pub const STR_TO_WATCH: &str = "📺 To Watch";
/// ➡️ Today
pub const STR_GO_TO_TODAY: &str = "➡️ Today";
/// 🔄️ Repeat the task
pub const STR_REPEAT: &str = "🔄️ Repeat the task";
/// ⚡️ Quick buttons
pub const STR_QUICK_BTNS: &str = "⚡️ Quick buttons";
/// ➡️ Move to buttons
pub const STR_MOVE_TO_BTNS: &str = "➡️ Move to buttons";

/// Mon
pub const STR_MONDAY: &str = "Mon";
/// Tue
pub const STR_TUESDAY: &str = "Tue";
/// Wed
pub const STR_WEDNESDAY: &str = "Wed";
/// Thu
pub const STR_THURSDAY: &str = "Thu";
/// Fri
pub const STR_FRIDAY: &str = "Fri";
/// Sat
pub const STR_SATURDAY: &str = "Sat";
/// Sun
pub const STR_SUNDAY: &str = "Sun";
/// Weekdays
pub const STR_WEEKDAYS: &str = "Weekdays";
/// Every day
pub const STR_EVERYDAY: &str = "Every day";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_for_exact() {
        // "journal" is a keyword in emojis.json (green heart)
        let result = emoji_for("journal");
        assert_eq!(result, "💚");
    }

    #[test]
    fn test_emoji_for_case_insensitive() {
        let result = emoji_for("Journal");
        assert_eq!(result, "💚");
    }

    #[test]
    fn test_emoji_for_unknown() {
        let result = emoji_for("xyzzy_nonexistent");
        assert!(result.is_empty());
    }

    #[test]
    fn test_add_emoji_found() {
        let result = add_emoji("journal");
        assert!(result.starts_with('💚'));
        assert!(result.contains("journal"));
    }

    #[test]
    fn test_add_emoji_not_found() {
        let result = add_emoji("xyzzy_nonexistent");
        assert_eq!(result, "xyzzy_nonexistent");
    }

    #[test]
    fn test_string_constants() {
        assert_eq!(STR_HOME, "🏠 Home");
        assert_eq!(STR_BACK, "⬅️ Back");
        assert_eq!(STR_COMPLETE, "✅ Complete");
    }
}

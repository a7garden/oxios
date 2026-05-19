//! LCS-based text merge algorithm.
//!
//! Ported from files.md (`server/sync/merge.go`) by Artem Zakirullin.
//! Finds the longest common subsequence between two texts and merges them
//! preserving all unique content from both sides.

use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

/// Journal header pattern: `## 23 May, Friday` or `#### 23 May, Friday`.
const HEADER_RE: &str = r"^(####|##) \d+ \w+, \w+";

/// Merge two text versions using LCS (Longest Common Subsequence).
///
/// Returns a merged string that preserves unique content from both inputs.
/// When both sides modified the same section, the server version is preferred
/// first, then client additions are appended.
pub fn merge(s1: &str, s2: &str) -> String {
    if s1.is_empty() { return s2.to_string(); }
    if s2.is_empty() { return s1.to_string(); }

    let lines1: Vec<&str> = s1.lines().collect();
    let lines2: Vec<&str> = s2.lines().collect();

    // Build LCS DP table
    let mut lcs: Vec<Vec<usize>> = vec![vec![0; lines2.len() + 1]; lines1.len() + 1];
    for i in 1..=lines1.len() {
        for j in 1..=lines2.len() {
            if lines1[i - 1] == lines2[j - 1] {
                lcs[i][j] = lcs[i - 1][j - 1] + 1;
            } else {
                lcs[i][j] = lcs[i - 1][j].max(lcs[i][j - 1]);
            }
        }
    }

    let result = backtrack(&lines1, &lines2, &lcs, lines1.len(), lines2.len());
    let result = merge_journal_headers(&result);
    result.join("\n")
}

/// Backtrack through the LCS table to produce merged lines.
fn backtrack(lines1: &[&str], lines2: &[&str], lcs: &[Vec<usize>], i: usize, j: usize) -> Vec<String> {
    if i == 0 && j == 0 { return vec![]; }
    if i == 0 {
        let mut r = backtrack(lines1, lines2, lcs, 0, j - 1);
        r.push(lines2[j - 1].to_string());
        return r;
    }
    if j == 0 {
        let mut r = backtrack(lines1, lines2, lcs, i - 1, 0);
        r.push(lines1[i - 1].to_string());
        return r;
    }
    if lines1[i - 1] == lines2[j - 1] {
        let mut r = backtrack(lines1, lines2, lcs, i - 1, j - 1);
        r.push(lines1[i - 1].to_string());
        r
    } else if lcs[i - 1][j] > lcs[i][j - 1] {
        let mut r = backtrack(lines1, lines2, lcs, i - 1, j);
        r.push(lines1[i - 1].to_string());
        r
    } else {
        let mut r = backtrack(lines1, lines2, lcs, i, j - 1);
        r.push(lines2[j - 1].to_string());
        r
    }
}

/// Merge consecutive journal headers that differ only in emoji suffixes.
///
/// Example: `## 23 May, Friday 🤸` + `## 23 May, Friday 🤸🍽` → `## 23 May, Friday 🤸🍽`
fn merge_journal_headers(lines: &[String]) -> Vec<String> {
    let re = Regex::new(HEADER_RE).unwrap();
    let emoji_re = Regex::new(r" [^\w\s\p{P}]+$").unwrap();
    let mut merged = Vec::new();
    let groups = group_consecutive_headers(&re, lines);

    for group in groups {
        if group.len() == 1 {
            merged.push(group[0].clone());
            continue;
        }

        let date = emoji_re.replace_all(&group[0], "").to_string();
        let prefix_same = group.iter().all(|line| {
            let emojis = emoji_re.find(line).map(|m| m.as_str()).unwrap_or("");
            date.clone() + emojis == *line
        });

        if !prefix_same {
            merged.extend(group);
            continue;
        }

        let mut found = String::new();
        for line in &group {
            if let Some(m) = emoji_re.find(line) {
                found.push_str(m.as_str());
            }
        }
        if !found.is_empty() {
            found = format!(" {}", unique_graphemes(&found));
        }
        merged.push(date + &found);
    }
    merged
}

fn group_consecutive_headers(re: &Regex, lines: &[String]) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = vec![];
    let mut i = 0;
    while i < lines.len() {
        if re.is_match(&lines[i]) {
            let mut group = vec![];
            while i < lines.len() && re.is_match(&lines[i]) {
                group.push(lines[i].clone());
                i += 1;
            }
            groups.push(group);
        } else {
            groups.push(vec![lines[i].clone()]);
            i += 1;
        }
    }
    groups
}

/// Return unique unicode graphemes from a string, preserving order.
fn unique_graphemes(s: &str) -> String {
    let mut seen = String::new();
    for g in s.graphemes(true) {
        if !seen.contains(g) {
            seen.push_str(g);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_empty() {
        assert_eq!(merge("", "hello"), "hello");
        assert_eq!(merge("hello", ""), "hello");
    }

    #[test]
    fn test_merge_identical() {
        assert_eq!(merge("a\nb", "a\nb"), "a\nb");
    }

    #[test]
    fn test_merge_different() {
        let result = merge("a\nb\nc", "x\ny\nz");
        assert!(result.contains("a"));
        assert!(result.contains("x"));
    }

    #[test]
    fn test_merge_preserves_common() {
        let result = merge("header\na\nb\nfooter", "header\nc\nb\nfooter");
        assert!(result.contains("header"));
        assert!(result.contains("footer"));
        assert!(result.contains("a"));
        assert!(result.contains("c"));
        assert!(result.contains("b"));
    }

    #[test]
    fn test_unique_graphemes() {
        assert_eq!(unique_graphemes("aab"), "ab");
    }
}

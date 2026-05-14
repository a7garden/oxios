//! Text chunking utilities for memory content processing.
//!
//! Splits text into overlapping chunks for embedding generation.
//! Supports both fixed-size and semantic (paragraph-based) chunking.

/// Configuration for text chunking.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum characters per chunk.
    pub max_chunk_size: usize,
    /// Overlap characters between consecutive chunks.
    pub overlap: usize,
    /// Minimum characters for a chunk (shorter segments are merged).
    pub min_chunk_size: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 512,
            overlap: 64,
            min_chunk_size: 50,
        }
    }
}

/// A text chunk with metadata.
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// The chunk text content.
    pub text: String,
    /// Start byte offset in the original text.
    pub start: usize,
    /// End byte offset in the original text.
    pub end: usize,
    /// Chunk index (0-based).
    pub index: usize,
}

/// Split text into fixed-size overlapping chunks.
///
/// Chunks are created by sliding a window of `max_chunk_size` characters
/// with `overlap` characters of overlap between consecutive chunks.
pub fn chunk_fixed(text: &str, config: &ChunkConfig) -> Vec<TextChunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len <= config.max_chunk_size {
        return vec![TextChunk {
            text: text.to_string(),
            start: 0,
            end: len,
            index: 0,
        }];
    }

    let mut chunks = Vec::new();
    let step = config.max_chunk_size.saturating_sub(config.overlap);
    let step = step.max(1); // ensure progress
    let mut pos = 0;
    let mut idx = 0;

    while pos < len {
        let end = (pos + config.max_chunk_size).min(len);
        let chunk_text: String = chars[pos..end].iter().collect();

        chunks.push(TextChunk {
            text: chunk_text,
            start: pos,
            end,
            index: idx,
        });

        pos += step;
        idx += 1;

        // If remaining is too small, include in last chunk
        if pos < len && len - pos < config.min_chunk_size {
            if let Some(last) = chunks.last_mut() {
                let remaining: String = chars[pos..].iter().collect();
                last.text.push_str(&remaining);
                last.end = len;
            }
            break;
        }
    }

    chunks
}

/// Split text into paragraphs, then group paragraphs into chunks
/// that don't exceed `max_chunk_size`.
pub fn chunk_paragraphs(text: &str, config: &ChunkConfig) -> Vec<TextChunk> {
    if text.is_empty() {
        return Vec::new();
    }

    // Split on double newlines (paragraph boundaries)
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    if paragraphs.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut chunk_start = 0;
    let mut idx = 0;

    for para in &paragraphs {
        if !current_text.is_empty() {
            current_text.push_str("\n\n");
        }

        // If adding this paragraph exceeds max size, flush current chunk
        if !current_text.is_empty() && current_text.len() + para.len() > config.max_chunk_size {
            let end = chunk_start + current_text.len();
            chunks.push(TextChunk {
                text: current_text.clone(),
                start: chunk_start,
                end,
                index: idx,
            });
            idx += 1;
            chunk_start = end;
            current_text.clear();
        }

        current_text.push_str(para);
    }

    // Flush remaining
    if !current_text.is_empty() {
        let len = current_text.len();
        chunks.push(TextChunk {
            text: current_text,
            start: chunk_start,
            end: chunk_start + len,
            index: idx,
        });
    }

    chunks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_fixed_empty() {
        let config = ChunkConfig::default();
        let chunks = chunk_fixed("", &config);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_fixed_short_text() {
        let config = ChunkConfig::default();
        let chunks = chunk_fixed("hello world", &config);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello world");
    }

    #[test]
    fn test_chunk_fixed_long_text() {
        let text = "abcdefghij".repeat(100); // 1000 chars
        let config = ChunkConfig {
            max_chunk_size: 200,
            overlap: 20,
            min_chunk_size: 50,
        };
        let chunks = chunk_fixed(&text, &config);

        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.text.len() <= 250); // allow some slack for min_chunk merging
        }

        // Verify overlap: consecutive chunks share some prefix/suffix
        if chunks.len() >= 2 {
            let suffix: String = chunks[0].text.chars().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect();
            let prefix: String = chunks[1].text.chars().take(20).collect();
            assert_eq!(suffix, prefix, "Overlapping region should match");
        }
    }

    #[test]
    fn test_chunk_paragraphs_basic() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let config = ChunkConfig {
            max_chunk_size: 100,
            overlap: 0,
            min_chunk_size: 10,
        };
        let chunks = chunk_paragraphs(text, &config);
        assert_eq!(chunks.len(), 1); // all fit in one chunk
        assert!(chunks[0].text.contains("First"));
        assert!(chunks[0].text.contains("Third"));
    }

    #[test]
    fn test_chunk_paragraphs_split() {
        let para1 = "a".repeat(50);
        let para2 = "b".repeat(50);
        let para3 = "c".repeat(50);
        let text = format!("{}\n\n{}\n\n{}", para1, para2, para3);

        let config = ChunkConfig {
            max_chunk_size: 80,
            overlap: 0,
            min_chunk_size: 10,
        };
        let chunks = chunk_paragraphs(&text, &config);
        assert!(chunks.len() >= 2, "Should split into multiple chunks");
    }

    #[test]
    fn test_chunk_fixed_indices() {
        let text = "abcdefghij";
        let config = ChunkConfig {
            max_chunk_size: 5,
            overlap: 2,
            min_chunk_size: 1,
        };
        let chunks = chunk_fixed(text, &config);

        // Verify indices form a coherent sequence
        assert_eq!(chunks[0].start, 0);
        for i in 1..chunks.len() {
            assert!(chunks[i].start >= chunks[i - 1].start);
        }
    }

    #[test]
    fn test_chunk_default_config() {
        let config = ChunkConfig::default();
        assert_eq!(config.max_chunk_size, 512);
        assert_eq!(config.overlap, 64);
        assert_eq!(config.min_chunk_size, 50);
    }
}

/// Text diffing utilities for efficient keyboard updates
///
/// This module provides algorithms to compute the minimal set of keyboard operations
/// needed to transform one text string into another, minimizing flickering during
/// real-time transcription updates.

/// Result of computing the difference between two text strings
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiff {
    /// Number of bytes in the common prefix (UTF-8 byte offset)
    pub common_prefix_bytes: usize,

    /// Number of characters to delete from the old text (after common prefix)
    pub chars_to_delete: usize,

    /// New text to type (after deletion)
    pub suffix_to_type: String,
}

/// Compute the minimal keyboard operations to transform `old_text` into `new_text`
///
/// This function finds the longest common prefix between the two strings and returns
/// the operations needed to update only the differing suffix.
///
/// # Algorithm
///
/// 1. Find the longest common prefix by comparing character-by-character
/// 2. Track byte offset (for UTF-8 slicing) and character count separately
/// 3. Return the number of characters to delete and the new suffix to type
///
/// # Examples
///
/// ```
/// use live_transcribe::text_diff::compute_text_diff;
///
/// let diff = compute_text_diff("Hello world", "Hello there");
/// assert_eq!(diff.common_prefix_bytes, 6); // "Hello "
/// assert_eq!(diff.chars_to_delete, 5);      // "world"
/// assert_eq!(diff.suffix_to_type, "there"); // new text
/// ```
///
/// # UTF-8 Safety
///
/// This function correctly handles multi-byte UTF-8 characters by tracking byte offsets
/// separately from character counts.
pub fn compute_text_diff(old_text: &str, new_text: &str) -> TextDiff {
    // Find longest common prefix
    let mut common_prefix_bytes = 0;

    for (c1, c2) in old_text.chars().zip(new_text.chars()) {
        if c1 == c2 {
            common_prefix_bytes += c1.len_utf8();
        } else {
            break;
        }
    }

    // Calculate operations needed for the differing suffix
    let old_suffix = &old_text[common_prefix_bytes..];
    let new_suffix = &new_text[common_prefix_bytes..];

    let chars_to_delete = old_suffix.chars().count();
    let suffix_to_type = new_suffix.to_string();

    TextDiff {
        common_prefix_bytes,
        chars_to_delete,
        suffix_to_type,
    }
}

/// Determine if text should be appended (old text is prefix of new text)
///
/// Returns `Some(suffix)` if `new_text` starts with `old_text` and has additional content,
/// otherwise returns `None`.
pub fn compute_append(old_text: &str, new_text: &str) -> Option<String> {
    if new_text.starts_with(old_text) && new_text.len() > old_text.len() {
        Some(new_text[old_text.len()..].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_change() {
        let diff = compute_text_diff("hello", "hello");
        assert_eq!(diff.common_prefix_bytes, 5);
        assert_eq!(diff.chars_to_delete, 0);
        assert_eq!(diff.suffix_to_type, "");
    }

    #[test]
    fn test_complete_replacement() {
        let diff = compute_text_diff("hello", "world");
        assert_eq!(diff.common_prefix_bytes, 0);
        assert_eq!(diff.chars_to_delete, 5);
        assert_eq!(diff.suffix_to_type, "world");
    }

    #[test]
    fn test_partial_change() {
        let diff = compute_text_diff("Hello world", "Hello there");
        assert_eq!(diff.common_prefix_bytes, 6); // "Hello "
        assert_eq!(diff.chars_to_delete, 5);      // "world"
        assert_eq!(diff.suffix_to_type, "there");
    }

    #[test]
    fn test_append_only() {
        let diff = compute_text_diff("Hello", "Hello world");
        assert_eq!(diff.common_prefix_bytes, 5);
        assert_eq!(diff.chars_to_delete, 0);
        assert_eq!(diff.suffix_to_type, " world");
    }

    #[test]
    fn test_delete_only() {
        let diff = compute_text_diff("Hello world", "Hello");
        assert_eq!(diff.common_prefix_bytes, 5);
        assert_eq!(diff.chars_to_delete, 6); // " world"
        assert_eq!(diff.suffix_to_type, "");
    }

    #[test]
    fn test_unicode_characters() {
        let diff = compute_text_diff("Hello 😀", "Hello 👋");
        assert_eq!(diff.common_prefix_bytes, 6); // "Hello "
        assert_eq!(diff.chars_to_delete, 1);      // 😀 (one character, 4 bytes)
        assert_eq!(diff.suffix_to_type, "👋");
    }

    #[test]
    fn test_unicode_common_prefix() {
        let diff = compute_text_diff("café blue", "café green");
        assert_eq!(diff.common_prefix_bytes, 6); // "café " (5 chars, 6 bytes due to é)
        assert_eq!(diff.chars_to_delete, 4);      // "blue"
        assert_eq!(diff.suffix_to_type, "green");
    }

    #[test]
    fn test_empty_old_text() {
        let diff = compute_text_diff("", "hello");
        assert_eq!(diff.common_prefix_bytes, 0);
        assert_eq!(diff.chars_to_delete, 0);
        assert_eq!(diff.suffix_to_type, "hello");
    }

    #[test]
    fn test_empty_new_text() {
        let diff = compute_text_diff("hello", "");
        assert_eq!(diff.common_prefix_bytes, 0);
        assert_eq!(diff.chars_to_delete, 5);
        assert_eq!(diff.suffix_to_type, "");
    }

    #[test]
    fn test_both_empty() {
        let diff = compute_text_diff("", "");
        assert_eq!(diff.common_prefix_bytes, 0);
        assert_eq!(diff.chars_to_delete, 0);
        assert_eq!(diff.suffix_to_type, "");
    }

    #[test]
    fn test_compute_append_with_suffix() {
        let suffix = compute_append("Hello", "Hello world");
        assert_eq!(suffix, Some(" world".to_string()));
    }

    #[test]
    fn test_compute_append_no_change() {
        let suffix = compute_append("Hello", "Hello");
        assert_eq!(suffix, None);
    }

    #[test]
    fn test_compute_append_diverged() {
        let suffix = compute_append("Hello world", "Hello there");
        assert_eq!(suffix, None);
    }

    #[test]
    fn test_compute_append_empty_old() {
        let suffix = compute_append("", "Hello");
        assert_eq!(suffix, Some("Hello".to_string()));
    }
}

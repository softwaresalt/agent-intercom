//! Shared utilities for MCP tool handlers.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Truncate `text` to at most `max_len` bytes, breaking at the nearest
/// preceding char boundary so the result is always valid UTF-8.
/// Appends `"..."` when truncation occurs and `max_len >= 3`.
/// For `max_len < 3`, returns a prefix truncated to the nearest char
/// boundary without an ellipsis.
#[must_use]
pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_owned();
    }

    // When max_len is too small for an ellipsis, return a boundary-safe
    // prefix without "..." so the result never exceeds max_len.
    if max_len < 3 {
        let boundary = text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= max_len)
            .last()
            .unwrap_or(0);
        return text[..boundary].to_owned();
    }

    // Find the largest char-boundary ≤ max_len - 3 (room for "...").
    let limit = max_len.saturating_sub(3);
    let boundary = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= limit)
        .last()
        .unwrap_or(0);

    format!("{}...", &text[..boundary])
}

/// Compute the SHA-256 hex digest of a file's contents.
///
/// Returns `"new_file"` if the file does not exist.
///
/// # Errors
///
/// Returns an error if reading the file fails for a reason other than
/// the file not existing.
pub async fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    match tokio::fs::read(path).await {
        Ok(contents) => {
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            Ok(format!("{:x}", hasher.finalize()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok("new_file".to_owned()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_within_limit() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_boundary() {
        assert_eq!(truncate_text("hello", 5), "hello");
    }

    #[test]
    fn truncate_over_limit() {
        let result = truncate_text("hello world", 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn truncate_multibyte_safe() {
        // "café" is 5 bytes (é is 2 bytes). Truncating at 6 should not
        // split the é character.
        let result = truncate_text("café world", 7);
        assert!(result.is_char_boundary(result.len().saturating_sub(3)));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_text("", 10), "");
    }

    #[test]
    fn truncate_max_len_zero() {
        let result = truncate_text("hello", 0);
        assert!(result.is_empty());
        assert_eq!(result, "");
    }

    #[test]
    fn truncate_max_len_one() {
        let result = truncate_text("hello", 1);
        assert!(result.len() <= 1);
        assert_eq!(result, "h");
    }

    #[test]
    fn truncate_max_len_two() {
        let result = truncate_text("hello", 2);
        assert!(result.len() <= 2);
        assert_eq!(result, "he");
    }

    #[test]
    fn truncate_max_len_two_multibyte() {
        // 'é' is 2 bytes — a max_len of 1 must not split the char.
        let result = truncate_text("é", 1);
        assert!(result.len() <= 1);
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn hash_nonexistent_file() {
        match compute_file_hash(Path::new("/nonexistent/file.txt")).await {
            Ok(hash) => assert_eq!(hash, "new_file"),
            Err(err) => panic!("expected Ok, got Err({err})"),
        }
    }
}

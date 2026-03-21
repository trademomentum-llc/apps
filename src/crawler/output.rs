//! File output -- writes crawled pages as markdown files with YAML metadata.

use std::path::{Path, PathBuf};

use crate::types::{MorphResult, MorphlexError};
use super::CrawlPage;

/// Write a crawled page to disk as a markdown file with YAML front matter.
///
/// Returns the path of the written file.
pub fn write_page(page: &CrawlPage, output_dir: &Path) -> MorphResult<PathBuf> {
    std::fs::create_dir_all(output_dir)?;

    let filename = sanitize_url_to_filename(&page.url);
    let path = output_dir.join(&filename);

    let title_str = match &page.title {
        Some(t) => t.clone(),
        None => String::new(),
    };

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("url: {}\n", page.url));
    content.push_str(&format!("depth: {}\n", page.depth));
    content.push_str(&format!("timestamp: {}\n", page.timestamp));
    content.push_str(&format!("title: {}\n", title_str));
    content.push_str("---\n\n");

    if !title_str.is_empty() {
        content.push_str(&format!("# {}\n\n", title_str));
    }

    content.push_str(&page.markdown);
    content.push('\n');

    std::fs::write(&path, &content)
        .map_err(|e| MorphlexError::CrawlError(format!("Failed to write {}: {}", path.display(), e)))?;

    Ok(path)
}

/// Sanitize a URL into a safe filename.
///
/// Replaces path separators with underscores, strips special characters,
/// truncates to 200 characters, and appends a BLAKE3 hash suffix for
/// collision avoidance.
fn sanitize_url_to_filename(url: &url::Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    let path = url.path();

    // Build base from host + path
    let raw = format!("{}{}", host, path);

    // Replace slashes and special chars
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '?' | '&' | '=' | '#' | '%' | ' ' => '_',
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' => c,
            _ => '_',
        })
        .collect();

    // Remove leading/trailing underscores and collapse runs
    let mut result = String::new();
    let mut last_was_underscore = true; // skip leading
    for c in sanitized.chars() {
        if c == '_' {
            if !last_was_underscore {
                result.push('_');
            }
            last_was_underscore = true;
        } else {
            last_was_underscore = false;
            result.push(c);
        }
    }

    // Trim trailing underscore
    let result = result.trim_end_matches('_').to_string();

    // BLAKE3 hash suffix for collision avoidance
    let hash = blake3::hash(url.as_str().as_bytes());
    let hash_hex = &hash.to_hex()[..8];

    // Truncate base to 200 chars to leave room for hash + extension
    let base = if result.len() > 200 {
        &result[..200]
    } else {
        &result
    };

    format!("{}_{}.md", base, hash_hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url_basic() {
        let url = url::Url::parse("https://example.com/page/about").unwrap();
        let filename = sanitize_url_to_filename(&url);
        assert!(filename.starts_with("example.com_page_about_"));
        assert!(filename.ends_with(".md"));
    }

    #[test]
    fn test_sanitize_url_special_chars() {
        let url = url::Url::parse("https://example.com/search?q=hello&page=1").unwrap();
        let filename = sanitize_url_to_filename(&url);
        assert!(filename.ends_with(".md"));
        assert!(!filename.contains('?'));
        assert!(!filename.contains('&'));
    }

    #[test]
    fn test_sanitize_url_root() {
        let url = url::Url::parse("https://example.com/").unwrap();
        let filename = sanitize_url_to_filename(&url);
        assert!(filename.ends_with(".md"));
    }

    #[test]
    fn test_sanitize_url_deterministic() {
        let url = url::Url::parse("https://example.com/page").unwrap();
        let a = sanitize_url_to_filename(&url);
        let b = sanitize_url_to_filename(&url);
        assert_eq!(a, b);
    }
}

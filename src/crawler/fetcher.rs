//! HTTP fetching via ureq -- handles GET requests and robots.txt retrieval.

use std::io::Read as _;

use crate::types::{MorphResult, MorphlexError};
use url::Url;

/// Maximum response body size: 10 MB.
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Fetch a URL and return the response body as a string.
///
/// Only accepts text/html content. Enforces a 10s connect timeout,
/// 30s read timeout, and 10MB body limit.
pub fn fetch(url: &Url, user_agent: &str) -> MorphResult<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .user_agent(user_agent)
        .redirects(5)
        .build();

    let response = agent
        .get(url.as_str())
        .call()
        .map_err(|e| MorphlexError::CrawlError(format!("HTTP request failed for {}: {}", url, e)))?;

    // Check content type -- only accept HTML
    let content_type = response.content_type().to_string();
    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Err(MorphlexError::CrawlError(
            format!("Non-HTML content type '{}' for {}", content_type, url),
        ));
    }

    // Read body with size limit
    let mut body = String::new();
    response
        .into_reader()
        .take(MAX_BODY_SIZE as u64)
        .read_to_string(&mut body)
        .map_err(|e| MorphlexError::CrawlError(format!("Failed to read body from {}: {}", url, e)))?;

    Ok(body)
}

/// Fetch /robots.txt for a given domain and scheme.
///
/// Returns the raw text content. Returns an empty string on 404 or any error
/// (missing robots.txt means everything is allowed).
pub fn fetch_robots_txt(domain: &str, scheme: &str, user_agent: &str) -> MorphResult<String> {
    let robots_url = format!("{}://{}/robots.txt", scheme, domain);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(10))
        .user_agent(user_agent)
        .redirects(3)
        .build();

    let response = match agent.get(&robots_url).call() {
        Ok(resp) => resp,
        Err(_) => return Ok(String::new()),
    };

    let mut body = String::new();
    match response
        .into_reader()
        .take(MAX_BODY_SIZE as u64)
        .read_to_string(&mut body)
    {
        Ok(_) => Ok(body),
        Err(_) => Ok(String::new()),
    }
}

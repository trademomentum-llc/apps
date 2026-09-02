//! HTTP fetching via ureq -- handles GET requests and robots.txt retrieval.

use std::io::Read as _;
use std::net::{IpAddr, ToSocketAddrs};

use crate::types::{MorphResult, MorphlexError};
use url::Url;

/// Maximum response body size: 10 MB.
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

fn ip_is_forbidden(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
        }
        IpAddr::V6(v6) => {
            v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

/// Validate that a URL is safe for the default crawler egress policy.
pub fn validate_public_http_url(url: &Url) -> MorphResult<()> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(MorphlexError::CrawlError(format!(
                "Unsupported URL scheme '{}'; only http/https are allowed",
                scheme
            )));
        }
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(MorphlexError::CrawlError(
            "URLs with embedded credentials are not allowed".to_string(),
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| MorphlexError::CrawlError("URL has no host".to_string()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| MorphlexError::CrawlError("URL has no usable port".to_string()))?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip_is_forbidden(ip) {
            return Err(MorphlexError::CrawlError(format!(
                "Refusing private or local address: {}",
                ip
            )));
        }
        return Ok(());
    }

    let addrs = (host, port).to_socket_addrs().map_err(|e| {
        MorphlexError::CrawlError(format!("DNS resolution failed for {}: {}", host, e))
    })?;
    let mut saw_addr = false;
    for addr in addrs {
        saw_addr = true;
        if ip_is_forbidden(addr.ip()) {
            return Err(MorphlexError::CrawlError(format!(
                "Refusing host {} because it resolves to private/local address {}",
                host,
                addr.ip()
            )));
        }
    }
    if !saw_addr {
        return Err(MorphlexError::CrawlError(format!(
            "DNS resolution produced no addresses for {}",
            host
        )));
    }

    Ok(())
}

/// Fetch a URL and return the response body as a string.
///
/// Only accepts text/html content. Enforces a 10s connect timeout,
/// 30s read timeout, and 10MB body limit.
pub fn fetch(url: &Url, user_agent: &str) -> MorphResult<String> {
    validate_public_http_url(url)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .user_agent(user_agent)
        .redirects(0)
        .build();

    let response = agent.get(url.as_str()).call().map_err(|e| {
        MorphlexError::CrawlError(format!("HTTP request failed for {}: {}", url, e))
    })?;

    // Check content type -- only accept HTML
    let content_type = response.content_type().to_string();
    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Err(MorphlexError::CrawlError(format!(
            "Non-HTML content type '{}' for {}",
            content_type, url
        )));
    }

    // Read body with size limit
    let mut body = String::new();
    response
        .into_reader()
        .take(MAX_BODY_SIZE as u64)
        .read_to_string(&mut body)
        .map_err(|e| {
            MorphlexError::CrawlError(format!("Failed to read body from {}: {}", url, e))
        })?;

    Ok(body)
}

/// Fetch /robots.txt for a given domain and scheme.
///
/// Returns the raw text content. Returns an empty string on 404 or any error
/// (missing robots.txt means everything is allowed).
pub fn fetch_robots_txt(domain: &str, scheme: &str, user_agent: &str) -> MorphResult<String> {
    let robots_url = format!("{}://{}/robots.txt", scheme, domain);
    let parsed = Url::parse(&robots_url).map_err(|e| {
        MorphlexError::CrawlError(format!("Invalid robots.txt URL {}: {}", robots_url, e))
    })?;
    validate_public_http_url(&parsed)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(10))
        .user_agent(user_agent)
        .redirects(0)
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

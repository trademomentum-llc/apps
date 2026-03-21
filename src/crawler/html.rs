//! HTML to markdown converter -- extracts text, links, and structure from HTML.
//!
//! Uses the `scraper` crate for DOM parsing. Converts semantic HTML elements
//! to their markdown equivalents while skipping non-content elements.

use scraper::{Html, Selector, ElementRef};
use url::Url;

use super::CrawlPage;

/// Tags to skip entirely during markdown conversion.
const SKIP_TAGS: &[&str] = &[
    "script", "style", "nav", "footer", "form", "noscript", "iframe",
    "header", "aside", "svg",
];

/// Parse an HTML document into a CrawlPage.
///
/// Extracts title, converts body to markdown, and collects links.
pub fn parse(html_content: &str, base_url: &Url) -> CrawlPage {
    let document = Html::parse_document(html_content);
    let title = extract_title(&document);
    let markdown = html_to_markdown(&document);
    let links = extract_links(&document, base_url);

    CrawlPage {
        url: base_url.clone(),
        depth: 0,
        timestamp: String::new(),
        title,
        markdown,
        links,
    }
}

/// Extract the <title> text from an HTML document.
fn extract_title(document: &Html) -> Option<String> {
    let selector = Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Convert an HTML document to markdown.
///
/// Walks the DOM and converts semantic elements:
/// - h1-h6 -> # headers
/// - p -> double newline separated paragraphs
/// - li -> "- " list items
/// - a -> [text](url)
/// - strong/b -> **text**
/// - em/i -> *text*
/// - pre/code -> backtick blocks
/// - Skips script, style, nav, footer, form, noscript, iframe
fn html_to_markdown(document: &Html) -> String {
    let body_selector = Selector::parse("body").ok();
    let root = match &body_selector {
        Some(sel) => document.select(sel).next(),
        None => None,
    };

    let mut output = String::new();

    match root {
        Some(body) => {
            walk_element(&body, &mut output);
        }
        None => {
            // No body tag -- walk the entire document
            for node in document.tree.nodes() {
                if let Some(el) = ElementRef::wrap(node) {
                    if el.value().name() == "html" {
                        walk_element(&el, &mut output);
                        break;
                    }
                }
            }
        }
    }

    // Clean up excessive whitespace
    let mut cleaned = String::new();
    let mut blank_count = 0;
    for line in output.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                cleaned.push('\n');
            }
        } else {
            blank_count = 0;
            cleaned.push_str(trimmed);
            cleaned.push('\n');
        }
    }

    cleaned.trim().to_string()
}

/// Recursively walk an element, converting to markdown.
fn walk_element(element: &ElementRef, output: &mut String) {
    let tag = element.value().name();

    // Skip non-content tags
    if SKIP_TAGS.contains(&tag) {
        return;
    }

    match tag {
        "h1" => {
            output.push_str("\n\n# ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "h2" => {
            output.push_str("\n\n## ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "h3" => {
            output.push_str("\n\n### ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "h4" => {
            output.push_str("\n\n#### ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "h5" => {
            output.push_str("\n\n##### ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "h6" => {
            output.push_str("\n\n###### ");
            collect_text(element, output);
            output.push_str("\n\n");
        }
        "p" => {
            output.push_str("\n\n");
            walk_children(element, output);
            output.push_str("\n\n");
        }
        "br" => {
            output.push('\n');
        }
        "li" => {
            output.push_str("\n- ");
            walk_children(element, output);
        }
        "ul" | "ol" => {
            output.push('\n');
            walk_children(element, output);
            output.push('\n');
        }
        "pre" => {
            output.push_str("\n\n```\n");
            collect_text(element, output);
            output.push_str("\n```\n\n");
        }
        "code" => {
            // Inline code (not inside pre)
            output.push('`');
            collect_text(element, output);
            output.push('`');
        }
        "strong" | "b" => {
            output.push_str("**");
            walk_children(element, output);
            output.push_str("**");
        }
        "em" | "i" => {
            output.push('*');
            walk_children(element, output);
            output.push('*');
        }
        "a" => {
            let href = element.value().attr("href").unwrap_or("");
            let text: String = element.text().collect::<String>().trim().to_string();
            if text.is_empty() {
                // Skip empty links
            } else if href.is_empty() {
                output.push_str(&text);
            } else {
                output.push('[');
                output.push_str(&text);
                output.push_str("](");
                output.push_str(href);
                output.push(')');
            }
        }
        "blockquote" => {
            output.push_str("\n\n> ");
            let mut inner = String::new();
            walk_children(element, &mut inner);
            output.push_str(&inner.trim().replace('\n', "\n> "));
            output.push_str("\n\n");
        }
        "hr" => {
            output.push_str("\n\n---\n\n");
        }
        "img" => {
            let alt = element.value().attr("alt").unwrap_or("");
            let src = element.value().attr("src").unwrap_or("");
            if !src.is_empty() {
                output.push_str(&format!("![{}]({})", alt, src));
            }
        }
        "table" | "thead" | "tbody" | "tr" | "td" | "th" | "div" | "span"
        | "section" | "article" | "main" | "dl" | "dt" | "dd"
        | "figure" | "figcaption" | "details" | "summary" => {
            walk_children(element, output);
        }
        _ => {
            walk_children(element, output);
        }
    }
}

/// Walk all children of an element.
fn walk_children(element: &ElementRef, output: &mut String) {
    for child in element.children() {
        match child.value() {
            scraper::Node::Text(text) => {
                let t = text.text.trim();
                if !t.is_empty() {
                    output.push_str(t);
                }
            }
            scraper::Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    walk_element(&child_el, output);
                }
            }
            _ => {}
        }
    }
}

/// Collect all text content from an element (no markdown formatting).
fn collect_text(element: &ElementRef, output: &mut String) {
    let text: String = element.text().collect::<String>();
    output.push_str(text.trim());
}

/// Extract all links from an HTML document, resolving relative URLs.
///
/// Filters out mailto:, javascript:, fragment-only, and non-HTTP(S) URLs.
fn extract_links(document: &Html, base_url: &Url) -> Vec<Url> {
    let selector = match Selector::parse("a[href]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut links = Vec::new();

    for element in document.select(&selector) {
        let href = match element.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };

        // Skip empty, fragment-only, mailto, javascript
        if href.is_empty()
            || href.starts_with('#')
            || href.starts_with("mailto:")
            || href.starts_with("javascript:")
            || href.starts_with("tel:")
            || href.starts_with("data:")
        {
            continue;
        }

        // Resolve relative URLs against base
        match base_url.join(href) {
            Ok(resolved) => {
                let scheme = resolved.scheme();
                if scheme == "http" || scheme == "https" {
                    links.push(resolved);
                }
            }
            Err(_) => continue,
        }
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let doc = Html::parse_document("<html><head><title>Test Page</title></head><body></body></html>");
        assert_eq!(extract_title(&doc), Some("Test Page".to_string()));
    }

    #[test]
    fn test_extract_title_missing() {
        let doc = Html::parse_document("<html><head></head><body></body></html>");
        assert_eq!(extract_title(&doc), None);
    }

    #[test]
    fn test_html_to_markdown_headings() {
        let doc = Html::parse_document("<html><body><h1>Title</h1><h2>Subtitle</h2></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("# Title"));
        assert!(md.contains("## Subtitle"));
    }

    #[test]
    fn test_html_to_markdown_paragraph() {
        let doc = Html::parse_document("<html><body><p>Hello world.</p></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("Hello world."));
    }

    #[test]
    fn test_html_to_markdown_list() {
        let doc = Html::parse_document("<html><body><ul><li>One</li><li>Two</li></ul></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("- One"));
        assert!(md.contains("- Two"));
    }

    #[test]
    fn test_html_to_markdown_bold_italic() {
        let doc = Html::parse_document("<html><body><p><strong>bold</strong> and <em>italic</em></p></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
    }

    #[test]
    fn test_html_to_markdown_link() {
        let doc = Html::parse_document(r#"<html><body><a href="https://example.com">click</a></body></html>"#);
        let md = html_to_markdown(&doc);
        assert!(md.contains("[click](https://example.com)"));
    }

    #[test]
    fn test_html_to_markdown_code() {
        let doc = Html::parse_document("<html><body><p>Use <code>foo()</code> here.</p></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("`foo()`"));
    }

    #[test]
    fn test_html_to_markdown_pre_block() {
        let doc = Html::parse_document("<html><body><pre>line 1\nline 2</pre></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("```"));
        assert!(md.contains("line 1"));
    }

    #[test]
    fn test_html_to_markdown_skips_script() {
        let doc = Html::parse_document("<html><body><p>Visible</p><script>alert(1)</script></body></html>");
        let md = html_to_markdown(&doc);
        assert!(md.contains("Visible"));
        assert!(!md.contains("alert"));
    }

    #[test]
    fn test_extract_links_basic() {
        let doc = Html::parse_document(r#"<html><body><a href="/page">Link</a><a href="https://other.com/x">Ext</a></body></html>"#);
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links(&doc, &base);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].as_str(), "https://example.com/page");
        assert_eq!(links[1].as_str(), "https://other.com/x");
    }

    #[test]
    fn test_extract_links_filters_mailto() {
        let doc = Html::parse_document(r#"<html><body><a href="mailto:x@y.com">Email</a><a href="javascript:void(0)">JS</a></body></html>"#);
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links(&doc, &base);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_links_filters_fragments() {
        let html_str = "<html><body><a href=\"#section\">Anchor</a></body></html>";
        let doc = Html::parse_document(html_str);
        let base = Url::parse("https://example.com/page").unwrap();
        let links = extract_links(&doc, &base);
        assert!(links.is_empty());
    }

    #[test]
    fn test_parse_full_page() {
        let html = r#"<html>
            <head><title>My Page</title></head>
            <body>
                <h1>Welcome</h1>
                <p>Content here.</p>
                <a href="/about">About</a>
            </body>
        </html>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let page = parse(html, &base);
        assert_eq!(page.title, Some("My Page".to_string()));
        assert!(page.markdown.contains("# Welcome"));
        assert!(page.markdown.contains("Content here."));
        assert_eq!(page.links.len(), 1);
    }
}

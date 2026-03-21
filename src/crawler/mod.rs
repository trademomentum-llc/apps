//! Recursive web crawler module -- BFS crawl with robots.txt, rate limiting, and markdown output.
//!
//! Crawls from a seed URL using breadth-first search, respecting robots.txt
//! directives and rate limits. Outputs pages as markdown files with YAML metadata.

pub mod fetcher;
pub mod fragment;
pub mod html;
pub mod output;
pub mod robots;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use url::Url;

use crate::types::{MorphResult, MorphlexError};

/// Configuration for a crawl session.
pub struct CrawlConfig {
    /// The starting URL.
    pub seed_url: Url,
    /// Maximum link depth from the seed.
    pub max_depth: u32,
    /// Delay between requests in milliseconds.
    pub delay_ms: u64,
    /// Directory to write output markdown files.
    pub output_dir: PathBuf,
    /// User-agent string for HTTP requests.
    pub user_agent: String,
    /// Maximum number of pages to crawl (None = unlimited).
    pub max_pages: Option<usize>,
}

/// A single crawled page.
pub struct CrawlPage {
    /// The URL of this page.
    pub url: Url,
    /// How many links deep from the seed URL.
    pub depth: u32,
    /// ISO 8601 timestamp of when the page was fetched.
    pub timestamp: String,
    /// The page title extracted from <title>.
    pub title: Option<String>,
    /// Page content converted to markdown.
    pub markdown: String,
    /// All links found on the page.
    pub links: Vec<Url>,
}

/// Summary of a crawl session.
pub struct CrawlSummary {
    /// Total pages successfully crawled.
    pub pages_crawled: usize,
    /// Pages skipped (robots.txt, errors, duplicates).
    pub pages_skipped: usize,
    /// Output directory where markdown files were written.
    pub output_dir: PathBuf,
}

/// Entry for the BFS queue.
struct QueueEntry {
    url: Url,
    depth: u32,
}

/// Run a BFS crawl starting from the seed URL in the config.
///
/// Respects robots.txt, enforces rate limiting via thread::sleep,
/// and writes each page as a markdown file. Stays within the seed domain.
pub fn crawl(config: &CrawlConfig) -> MorphResult<CrawlSummary> {
    let seed_domain = config
        .seed_url
        .host_str()
        .ok_or_else(|| MorphlexError::CrawlError("Seed URL has no host".to_string()))?
        .to_string();

    let seed_scheme = config.seed_url.scheme().to_string();

    // BFS state
    let mut queue: VecDeque<QueueEntry> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut robots_cache: HashMap<String, robots::RobotsRules> = HashMap::new();

    let mut pages_crawled: usize = 0;
    let mut pages_skipped: usize = 0;

    // Seed the queue
    queue.push_back(QueueEntry {
        url: config.seed_url.clone(),
        depth: 0,
    });
    visited.insert(normalize_url(&config.seed_url));

    // Pre-fetch robots.txt for seed domain
    eprintln!("Fetching robots.txt for {}...", seed_domain);
    let robots_content = fetcher::fetch_robots_txt(&seed_domain, &seed_scheme, &config.user_agent)?;
    let rules = robots::parse_robots_txt(&robots_content, &config.user_agent);

    // Apply crawl-delay from robots.txt if it is larger than our configured delay
    let effective_delay = match rules.crawl_delay {
        Some(delay_secs) => {
            let robots_delay_ms = delay_secs * 1000;
            if robots_delay_ms > config.delay_ms {
                eprintln!("Using robots.txt crawl-delay: {}s", delay_secs);
                robots_delay_ms
            } else {
                config.delay_ms
            }
        }
        None => config.delay_ms,
    };

    robots_cache.insert(seed_domain.clone(), rules);

    eprintln!("Starting crawl from {} (max_depth={}, delay={}ms)", config.seed_url, config.max_depth, effective_delay);

    // BFS loop
    while let Some(entry) = queue.pop_front() {
        // Check page limit
        if let Some(max) = config.max_pages {
            if pages_crawled >= max {
                eprintln!("Reached max_pages limit ({})", max);
                break;
            }
        }

        // Check depth limit
        if entry.depth > config.max_depth {
            pages_skipped += 1;
            continue;
        }

        // Get domain for this URL
        let domain = match entry.url.host_str() {
            Some(d) => d.to_string(),
            None => {
                pages_skipped += 1;
                continue;
            }
        };

        // Only crawl same domain as seed
        if domain != seed_domain {
            pages_skipped += 1;
            continue;
        }

        // Check robots.txt
        let robots_rules = get_or_fetch_robots(
            &mut robots_cache,
            &domain,
            entry.url.scheme(),
            &config.user_agent,
        );
        if !robots::is_allowed(&robots_rules, entry.url.path()) {
            eprintln!("  [SKIP] robots.txt disallows: {}", entry.url);
            pages_skipped += 1;
            continue;
        }

        // Rate limiting
        if pages_crawled > 0 {
            thread::sleep(Duration::from_millis(effective_delay));
        }

        // Fetch the page
        eprintln!("  [GET]  depth={} {}", entry.depth, entry.url);
        let body = match fetcher::fetch(&entry.url, &config.user_agent) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  [ERR]  {}: {}", entry.url, e);
                pages_skipped += 1;
                continue;
            }
        };

        // Parse HTML -> CrawlPage
        let mut page = html::parse(&body, &entry.url);
        page.depth = entry.depth;
        page.timestamp = current_timestamp();

        // Enqueue discovered links
        if entry.depth < config.max_depth {
            for link in &page.links {
                let normalized = normalize_url(link);
                if !visited.contains(&normalized) {
                    visited.insert(normalized);
                    queue.push_back(QueueEntry {
                        url: link.clone(),
                        depth: entry.depth + 1,
                    });
                }
            }
        }

        // Write output
        match output::write_page(&page, &config.output_dir) {
            Ok(path) => {
                eprintln!("  [OK]   -> {}", path.display());
                pages_crawled += 1;
            }
            Err(e) => {
                eprintln!("  [ERR]  write failed: {}", e);
                pages_skipped += 1;
            }
        }
    }

    eprintln!(
        "Crawl complete: {} pages crawled, {} skipped",
        pages_crawled, pages_skipped
    );

    Ok(CrawlSummary {
        pages_crawled,
        pages_skipped,
        output_dir: config.output_dir.clone(),
    })
}

/// Get robots rules from cache or fetch them.
fn get_or_fetch_robots<'a>(
    cache: &'a mut HashMap<String, robots::RobotsRules>,
    domain: &str,
    scheme: &str,
    user_agent: &str,
) -> &'a robots::RobotsRules {
    if !cache.contains_key(domain) {
        let content = fetcher::fetch_robots_txt(domain, scheme, user_agent)
            .unwrap_or_default();
        let rules = robots::parse_robots_txt(&content, user_agent);
        cache.insert(domain.to_string(), rules);
    }
    cache.get(domain).unwrap()
}

/// Normalize a URL for deduplication.
///
/// Strips fragment, trailing slash, and lowercases scheme+host.
fn normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);

    let mut s = normalized.to_string();

    // Strip trailing slash for consistency (but not the root path)
    if s.ends_with('/') && s.matches('/').count() > 3 {
        s.pop();
    }

    s
}

/// Get the current time as an ISO 8601 string.
///
/// Uses a simple format without external chrono dependency.
fn current_timestamp() -> String {
    // Read from system -- no chrono needed for this basic format
    match std::process::Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

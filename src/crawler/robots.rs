//! robots.txt parser -- respects crawl directives.
//!
//! Parses Allow/Disallow/Crawl-delay directives for a given user-agent.
//! Longest prefix match determines allow/disallow.

/// Parsed robots.txt rules for a specific user-agent.
pub struct RobotsRules {
    pub disallow: Vec<String>,
    pub allow: Vec<String>,
    pub crawl_delay: Option<u64>,
}

/// Parse robots.txt content, extracting rules for our user-agent.
///
/// Matches our user-agent first; falls back to * if no specific match.
pub fn parse_robots_txt(content: &str, our_user_agent: &str) -> RobotsRules {
    let mut rules = RobotsRules {
        disallow: Vec::new(),
        allow: Vec::new(),
        crawl_delay: None,
    };

    // We need to find blocks that match our user-agent or the wildcard.
    // A "block" starts with one or more User-agent lines and contains
    // Disallow/Allow/Crawl-delay directives until the next User-agent line.

    let our_agent_lower = our_user_agent.to_lowercase();
    let mut in_matching_block = false;
    let mut in_wildcard_block = false;
    let mut found_specific = false;

    // Two-pass: first check if we have a specific match
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(agent) = trimmed.strip_prefix("User-agent:").or_else(|| trimmed.strip_prefix("user-agent:")) {
            let agent = agent.trim().to_lowercase();
            if agent == our_agent_lower || our_agent_lower.starts_with(&agent) {
                found_specific = true;
                break;
            }
        }
    }

    // Second pass: collect rules
    let mut wildcard_rules = RobotsRules {
        disallow: Vec::new(),
        allow: Vec::new(),
        crawl_delay: None,
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            // Empty line resets block context (standard behavior)
            if trimmed.is_empty() {
                in_matching_block = false;
                in_wildcard_block = false;
            }
            continue;
        }

        let lower = trimmed.to_lowercase();

        if lower.starts_with("user-agent:") {
            let agent = trimmed.split_once(':').map(|(_, v)| v.trim().to_lowercase()).unwrap_or_default();
            if agent == "*" {
                in_wildcard_block = true;
                in_matching_block = false;
            } else if agent == our_agent_lower || our_agent_lower.starts_with(&agent) {
                in_matching_block = true;
                in_wildcard_block = false;
            } else {
                in_matching_block = false;
                in_wildcard_block = false;
            }
            continue;
        }

        if in_matching_block {
            parse_directive(trimmed, &mut rules);
        } else if in_wildcard_block && !found_specific {
            parse_directive(trimmed, &mut wildcard_rules);
        }
    }

    // If no specific match found, use wildcard rules
    if !found_specific {
        return wildcard_rules;
    }

    rules
}

fn parse_directive(line: &str, rules: &mut RobotsRules) {
    let lower = line.to_lowercase();
    if lower.starts_with("disallow:") {
        if let Some((_, path)) = line.split_once(':') {
            let path = path.trim();
            if !path.is_empty() {
                rules.disallow.push(path.to_string());
            }
        }
    } else if lower.starts_with("allow:") {
        if let Some((_, path)) = line.split_once(':') {
            let path = path.trim();
            if !path.is_empty() {
                rules.allow.push(path.to_string());
            }
        }
    } else if lower.starts_with("crawl-delay:") {
        if let Some((_, delay)) = line.split_once(':') {
            if let Ok(d) = delay.trim().parse::<u64>() {
                rules.crawl_delay = Some(d);
            }
        }
    }
}

/// Check if a given path is allowed by the robots rules.
///
/// Uses longest prefix match: if both Allow and Disallow match,
/// the longer prefix wins. On tie, Allow wins.
pub fn is_allowed(rules: &RobotsRules, path: &str) -> bool {
    let mut best_disallow: Option<usize> = None;
    let mut best_allow: Option<usize> = None;

    for pattern in &rules.disallow {
        if path_matches(path, pattern) {
            let len = pattern.len();
            match best_disallow {
                Some(current) if len > current => best_disallow = Some(len),
                None => best_disallow = Some(len),
                _ => {}
            }
        }
    }

    for pattern in &rules.allow {
        if path_matches(path, pattern) {
            let len = pattern.len();
            match best_allow {
                Some(current) if len > current => best_allow = Some(len),
                None => best_allow = Some(len),
                _ => {}
            }
        }
    }

    match (best_allow, best_disallow) {
        (None, None) => true,          // no rules match -> allowed
        (Some(_), None) => true,       // only allow matches
        (None, Some(_)) => false,      // only disallow matches
        (Some(a), Some(d)) => a >= d,  // longest wins; tie -> allow
    }
}

/// Simple prefix-based path matching (standard robots.txt semantics).
/// Supports * as wildcard and $ as end-of-string anchor.
fn path_matches(path: &str, pattern: &str) -> bool {
    // Handle $ anchor at end
    if let Some(prefix) = pattern.strip_suffix('$') {
        if prefix.contains('*') {
            return glob_match(path, pattern);
        }
        return path == prefix;
    }

    // Handle * wildcard
    if pattern.contains('*') {
        return glob_match(path, pattern);
    }

    // Simple prefix match
    path.starts_with(pattern)
}

/// Simple glob matching for robots.txt patterns with * wildcard.
fn glob_match(path: &str, pattern: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = if anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match path[pos..].find(part) {
            Some(idx) => {
                if i == 0 && idx != 0 {
                    // First segment must match from start
                    return false;
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }

    if anchored {
        pos == path.len()
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wildcard_rules() {
        let content = "\
User-agent: *
Disallow: /admin/
Disallow: /private/
Allow: /admin/public/
Crawl-delay: 5
";
        let rules = parse_robots_txt(content, "morphlex-crawler/0.1");
        assert_eq!(rules.disallow.len(), 2);
        assert_eq!(rules.allow.len(), 1);
        assert_eq!(rules.crawl_delay, Some(5));
    }

    #[test]
    fn test_parse_specific_agent() {
        let content = "\
User-agent: *
Disallow: /

User-agent: morphlex-crawler
Disallow: /secret/
Allow: /public/
";
        let rules = parse_robots_txt(content, "morphlex-crawler");
        assert_eq!(rules.disallow, vec!["/secret/"]);
        assert_eq!(rules.allow, vec!["/public/"]);
    }

    #[test]
    fn test_is_allowed_basic() {
        let rules = RobotsRules {
            disallow: vec!["/admin/".to_string(), "/private/".to_string()],
            allow: vec!["/admin/public/".to_string()],
            crawl_delay: None,
        };

        assert!(is_allowed(&rules, "/"));
        assert!(is_allowed(&rules, "/about"));
        assert!(!is_allowed(&rules, "/admin/settings"));
        assert!(is_allowed(&rules, "/admin/public/page"));
        assert!(!is_allowed(&rules, "/private/data"));
    }

    #[test]
    fn test_is_allowed_empty_rules() {
        let rules = RobotsRules {
            disallow: Vec::new(),
            allow: Vec::new(),
            crawl_delay: None,
        };
        assert!(is_allowed(&rules, "/anything"));
    }

    #[test]
    fn test_disallow_all() {
        let content = "\
User-agent: *
Disallow: /
";
        let rules = parse_robots_txt(content, "testbot");
        assert!(!is_allowed(&rules, "/anything"));
        assert!(!is_allowed(&rules, "/"));
    }

    #[test]
    fn test_path_with_wildcard() {
        let rules = RobotsRules {
            disallow: vec!["/*.json".to_string()],
            allow: Vec::new(),
            crawl_delay: None,
        };
        assert!(!is_allowed(&rules, "/api/data.json"));
        assert!(is_allowed(&rules, "/api/data.html"));
    }

    #[test]
    fn test_path_with_dollar_anchor() {
        let rules = RobotsRules {
            disallow: vec!["/exact-path$".to_string()],
            allow: Vec::new(),
            crawl_delay: None,
        };
        assert!(!is_allowed(&rules, "/exact-path"));
        assert!(is_allowed(&rules, "/exact-path/more"));
    }
}

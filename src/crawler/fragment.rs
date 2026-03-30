//! Fragment analysis -- stub for future convergence pool integration.
//!
//! This module will eventually break crawled pages into fragments
//! for indexing and vectorization through the morphlex pipeline.

/// Result of fragment analysis on a crawled page.
pub struct FragmentAnalysis {
    pub fragment_count: usize,
}

/// Analyze a crawled page for fragments. Currently a stub.
pub fn analyze(_page: &super::CrawlPage) -> FragmentAnalysis {
    FragmentAnalysis { fragment_count: 0 }
}

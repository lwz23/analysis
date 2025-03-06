pub mod models;
pub mod visitors;
pub mod analysis;
pub mod utils;

// Re-export main types for convenience
pub use analysis::analyzer::StaticAnalyzer;

// Default configuration constants
pub const DEFAULT_MAX_SEARCH_DEPTH: usize = 20;
pub const DEFAULT_FILE_SIZE_LIMIT: u64 = 10; // MB
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
/// Cache implementations for the detection engine.
///
/// This module contains all caching layers used during project detection:
///
/// - **DetectionCache**: High-level cache for complete detection results
///   - Caches entire `DetectionResult` objects
///   - Persists across CLI invocations
///   - Invalidates based on file system changes
///
/// - **FileSystemCacheManager**: Low-level infrastructure cache
///   - Caches file existence checks and metadata
///   - Single detection run lifetime
///   - Avoids redundant file system operations
///
/// - **ParsedFileCache**: Content parsing cache
///   - Caches parsed JSON, TOML, and text file contents
///   - Single detection run lifetime
///   - Avoids redundant file parsing
pub mod detection;
pub mod file_system;
pub mod parsed_file;

pub use detection::{CachedDetection, DetectionCache};
pub use file_system::FileSystemCacheManager;
pub use parsed_file::ParsedFileCache;

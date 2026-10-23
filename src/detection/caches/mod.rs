/// Cache implementations for the detection engine.
///
/// **ParsedFileCache** caches parsed JSON, TOML, and text file contents for
/// a single detection run, so each manifest is read and parsed at most once.
/// (File existence/metadata caching lives in `crate::performance`.)
pub mod parsed_file;

pub use parsed_file::ParsedFileCache;

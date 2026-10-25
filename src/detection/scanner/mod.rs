//! Deterministic file scanning: a name-sorted, gitignore-aware walk bounded
//! by a match cap and a size-scaled timeout. Root-level manifests are always
//! kept ahead of the cap, and root-level files are topped up if a busy
//! subdirectory exhausts it.

pub mod scanning_engine;
pub mod traverser;

pub use scanning_engine::{ScanOutcome, ScanningEngine};

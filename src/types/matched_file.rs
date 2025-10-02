//! File matching and weighting system for relevance ranking.
//!
//! This module provides sophisticated file classification and weighting to prioritize
//! files based on their location and purpose within a project structure.
//!
//! # Overview
//!
//! When scanning a project, not all files are equally important. A `package.json` in the
//! root is more significant than one in `node_modules/some-package/`. This module
//! calculates relevance weights based on:
//!
//! - **File depth** - How many directories deep (root files are more important)
//! - **Directory type** - Source code vs tests vs dependencies vs build artifacts
//!
//! # Core Types
//!
//! - [`DirectoryType`] - Classification of directories (Source, Test, Build, etc.)
//! - [`MatchedFile`] - A matched file with computed depth and weight
//!
//! # Directory Classification
//!
//! Directories are classified into types with different weights:
//!
//! | Type | Weight | Examples | Rationale |
//! |------|--------|----------|-----------|
//! | **Source** | 1.2 | `src/`, `lib/`, `app/` | Core project code - highest value |
//! | **Config** | 1.1 | `.github/`, `config/` | Important configuration |
//! | **Root** | 1.0 | (root level) | Direct project files |
//! | **Unknown** | 0.8 | Other directories | Potentially relevant |
//! | **Documentation** | 0.6 | `docs/`, `manual/` | Supporting files |
//! | **Examples** | 0.3 | `examples/`, `demo/` | Sample code |
//! | **Test** | 0.2 | `test/`, `__tests__/` | Test fixtures |
//! | **Build** | 0.1 | `dist/`, `target/`, `build/` | Generated artifacts |
//! | **Dependencies** | 0.05 | `node_modules/`, `vendor/` | External code |
//!
//! ```rust
//! use project_indicator::types::DirectoryType;
//!
//! assert_eq!(DirectoryType::classify("src"), DirectoryType::Source);
//! assert_eq!(DirectoryType::classify("SRC"), DirectoryType::Source); // Case insensitive
//! assert_eq!(DirectoryType::classify("test"), DirectoryType::Test);
//! assert_eq!(DirectoryType::classify("node_modules"), DirectoryType::Dependencies);
//!
//! assert_eq!(DirectoryType::Source.weight(), 1.2);
//! assert_eq!(DirectoryType::Test.weight(), 0.2);
//! assert_eq!(DirectoryType::Dependencies.weight(), 0.05);
//! ```
//!
//! # Depth-Based Weighting
//!
//! File depth significantly affects relevance:
//!
//! | Depth | Weight | Example |
//! |-------|--------|---------|
//! | 0 (root) | 1.0 | `package.json` |
//! | 1 | 0.7 | `src/main.rs` |
//! | 2 | 0.4 | `src/utils/helpers.rs` |
//! | 3 | 0.1 | `src/components/ui/button.tsx` |
//! | 4+ | 0.05 | Very deeply nested files |
//!
//! # Combined Weighting
//!
//! Final weight = `depth_weight × directory_type_weight`
//!
//! This means:
//! - `src/main.rs` (depth 1, Source) = 0.7 × 1.2 = **0.84**
//! - `test/fixtures/package.json` (depth 2, Test) = 0.4 × 0.2 = **0.08**
//! - `node_modules/react/package.json` (depth 2, Dependencies) = 0.4 × 0.05 = **0.02**
//!
//! # Example: File Weight Calculation
//!
//! ```rust
//! use project_indicator::types::MatchedFile;
//!
//! // Root package.json - maximum relevance
//! let root_manifest = MatchedFile::new(
//!     "package.json".to_string(),
//!     "package.json".to_string(),
//! );
//! assert_eq!(root_manifest.depth, 0);
//! assert_eq!(root_manifest.weight(), 1.0);
//!
//! // Source file - high relevance
//! let source_file = MatchedFile::new(
//!     "main.rs".to_string(),
//!     "src/main.rs".to_string(),
//! );
//! assert_eq!(source_file.depth, 1);
//! assert!((source_file.weight() - 0.84).abs() < f32::EPSILON);
//!
//! // Test fixture - low relevance
//! let test_fixture = MatchedFile::new(
//!     "package.json".to_string(),
//!     "test/fixtures/package.json".to_string(),
//! );
//! assert_eq!(test_fixture.depth, 2);
//! assert!((test_fixture.weight() - 0.08).abs() < f32::EPSILON);
//!
//! // Dependency - very low relevance
//! let dependency_file = MatchedFile::new(
//!     "package.json".to_string(),
//!     "node_modules/react/package.json".to_string(),
//! );
//! assert_eq!(dependency_file.depth, 2);
//! assert!((dependency_file.weight() - 0.02).abs() < f32::EPSILON);
//! ```
//!
//! # Use Cases
//!
//! This weighting system helps with:
//!
//! - **Confidence calculation** - Root files boost confidence more than test files
//! - **Conflict resolution** - Prefer languages with higher-weighted matches
//! - **Result ranking** - Show most relevant detection results first
//! - **False positive reduction** - Ignore test fixtures and dependencies
//!
//! # Example: Comparing File Relevance
//!
//! ```rust
//! use project_indicator::types::MatchedFile;
//!
//! let files = vec![
//!     MatchedFile::new("main.rs".to_string(), "src/main.rs".to_string()),
//!     MatchedFile::new("test.rs".to_string(), "tests/test.rs".to_string()),
//!     MatchedFile::new("Cargo.toml".to_string(), "Cargo.toml".to_string()),
//! ];
//!
//! // Sort by weight (descending)
//! let mut sorted = files.clone();
//! sorted.sort_by(|a, b| b.weight().partial_cmp(&a.weight()).unwrap());
//!
//! // Root Cargo.toml is most relevant
//! assert_eq!(sorted[0].filename, "Cargo.toml");
//! assert_eq!(sorted[0].weight(), 1.0);
//!
//! // Source file is next
//! assert_eq!(sorted[1].filename, "main.rs");
//!
//! // Test file is least relevant
//! assert_eq!(sorted[2].filename, "test.rs");
//! ```

use crate::constants::DOT_GIT;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DirectoryType {
    Root,
    Source,
    Config,
    Test,
    Build,
    Dependencies,
    Documentation,
    Examples,
    Unknown,
}

impl DirectoryType {
    pub fn weight(&self) -> f32 {
        match self {
            DirectoryType::Root => 1.0,
            DirectoryType::Source => 1.2,
            DirectoryType::Config => 1.1,
            DirectoryType::Test => 0.2,
            DirectoryType::Build => 0.1,
            DirectoryType::Dependencies => 0.05,
            DirectoryType::Documentation => 0.6,
            DirectoryType::Examples => 0.3,
            DirectoryType::Unknown => 0.8,
        }
    }
    pub fn classify(path_component: &str) -> Self {
        match path_component.to_lowercase().as_str() {
            "src" | "lib" | "app" | "source" | "code" => DirectoryType::Source,

            "test" | "tests" | "spec" | "specs" | "__tests__" | "fixtures" | "test-fixtures"
            | "__test__" | "__spec__" | "testing" => DirectoryType::Test,

            "dist" | "build" | "target" | "out" | "output" | "bin" | "release" | "debug" => {
                DirectoryType::Build
            }

            "node_modules" | "vendor" | DOT_GIT | "packages" | "deps" => {
                DirectoryType::Dependencies
            }

            "docs" | "doc" | "documentation" | "manual" | "guide" => DirectoryType::Documentation,

            "examples" | "example" | "samples" | "sample" | "demo" | "demos" => {
                DirectoryType::Examples
            }

            ".github" | ".vscode" | ".idea" | "config" | "configuration" | "configs"
            | "settings" | ".config" => DirectoryType::Config,

            _ => DirectoryType::Unknown,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedFile {
    pub filename: String,
    pub relative_path: String,
    pub depth: usize,
    pub directory_type: DirectoryType,
}

impl MatchedFile {
    pub fn new(filename: String, relative_path: String) -> Self {
        let depth = Self::calculate_depth(&relative_path);
        let directory_type = Self::classify_directory(&relative_path);

        Self {
            filename,
            relative_path,
            depth,
            directory_type,
        }
    }
    pub fn weight(&self) -> f32 {
        Self::calculate_weight(self.depth, self.directory_type)
    }
    fn calculate_depth(relative_path: &str) -> usize {
        if relative_path.is_empty() {
            return 0;
        }

        relative_path.matches('/').count()
    }
    fn classify_directory(relative_path: &str) -> DirectoryType {
        if relative_path.is_empty() || !relative_path.contains('/') {
            return DirectoryType::Root;
        }

        let first_component = relative_path.split('/').next().unwrap_or("");
        DirectoryType::classify(first_component)
    }
    fn calculate_weight(depth: usize, directory_type: DirectoryType) -> f32 {
        let depth_weight = match depth {
            0 => 1.0,
            1 => 0.7,
            2 => 0.4,
            3 => 0.1,
            _ => 0.05,
        };

        depth_weight * directory_type.weight()
    }
}

//! A scan match with a relevance weight: `depth weight × directory-type
//! weight` (see `DirectoryType::weight` and `MatchedFile::weight`).

use crate::constants::DOT_GIT;

// ponytail: three weight tiers, not a nine-class taxonomy — the weights only
// shape the markerless ratio for probed source paths; bring finer classes
// back only if real folders rank wrongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryType {
    /// A depth-0 file (no directory component)
    Root,
    /// Source directories (src, lib, app, …)
    Source,
    /// Tests, build output, dependencies, docs, examples: present in every
    /// repo whatever the language, so nearly no identifying signal
    Noise,
    /// Any other directory
    Unknown,
}

impl DirectoryType {
    pub fn weight(&self) -> f32 {
        match self {
            DirectoryType::Root => 1.0,
            DirectoryType::Source => 1.2,
            DirectoryType::Noise => 0.1,
            DirectoryType::Unknown => 0.8,
        }
    }
    pub fn classify(path_component: &str) -> Self {
        match path_component.to_lowercase().as_str() {
            "src" | "lib" | "app" | "source" | "code" => DirectoryType::Source,

            // NOTE: "packages" is NOT noise — it is THE standard pnpm/yarn
            // monorepo layout, and the subproject scan skips noise dirs
            "test" | "tests" | "spec" | "specs" | "__tests__" | "fixtures" | "test-fixtures"
            | "__test__" | "__spec__" | "testing" | "dist" | "build" | "target" | "out"
            | "output" | "bin" | "release" | "debug" | "node_modules" | "vendor" | DOT_GIT
            | "deps" | "docs" | "doc" | "documentation" | "manual" | "guide" | "examples"
            | "example" | "samples" | "sample" | "demo" | "demos" => DirectoryType::Noise,

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

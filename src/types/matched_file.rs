//! A scan match with a relevance weight: `depth weight × directory-type
//! weight` (see `DirectoryType::weight` and `MatchedFile::weight`).

use crate::constants::DOT_GIT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

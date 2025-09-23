//! Shared test helper functions for matcher tests

#[cfg(test)]
pub mod helpers {
    use crate::types::{DetectionType, FrameworkDetector};
    use std::fs;
    use tempfile::TempDir;

    /// Create a temporary directory with a package.json file
    pub fn create_package_json(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let package_path = temp_dir.path().join("package.json");
        fs::write(package_path, content).unwrap();
        temp_dir
    }

    /// Create a temporary directory with a Cargo.toml file
    pub fn create_cargo_toml(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let cargo_path = temp_dir.path().join("Cargo.toml");
        fs::write(cargo_path, content).unwrap();
        temp_dir
    }

    /// Create a temporary directory with a pyproject.toml file
    pub fn create_pyproject_toml(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");
        fs::write(pyproject_path, content).unwrap();
        temp_dir
    }

    /// Create a temporary directory with a go.mod file
    pub fn create_go_mod(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let go_mod_path = temp_dir.path().join("go.mod");
        fs::write(go_mod_path, content).unwrap();
        temp_dir
    }

    /// Create a temporary directory with a Gemfile
    pub fn create_gemfile(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let gemfile_path = temp_dir.path().join("Gemfile");
        fs::write(gemfile_path, content).unwrap();
        temp_dir
    }

    /// Create a temporary directory with a composer.json file
    pub fn create_composer_json(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let composer_path = temp_dir.path().join("composer.json");
        fs::write(composer_path, content).unwrap();
        temp_dir
    }

    /// Create a test framework detector for PackageJson detection
    pub fn create_package_json_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::PackageJson {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    /// Create a test framework detector for CargoToml detection
    pub fn create_cargo_toml_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::CargoToml {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    /// Create a test framework detector for PyProjectToml detection
    pub fn create_pyproject_toml_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::PyProjectToml {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    /// Create a test framework detector for GoMod detection
    pub fn create_go_mod_framework(name: &str, modules: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::GoMod {
                modules: modules.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    /// Create a test framework detector for GemSpec detection
    pub fn create_gemspec_framework(name: &str, gems: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::GemSpec {
                gems: gems.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    /// Create a test framework detector for ComposerJson detection
    pub fn create_composer_json_framework(name: &str, packages: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::ComposerJson {
                packages: packages.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }
}

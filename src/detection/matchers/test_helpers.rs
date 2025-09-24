#[cfg(test)]
pub mod helpers {
    use crate::types::{DetectionType, FrameworkDetector};
    use std::fs;
    use tempfile::TempDir;

    pub fn create_file(
        filename: &str,
        content: &str,
    ) -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join(filename);
        fs::write(file_path, content)?;
        Ok(temp_dir)
    }

    pub fn create_package_json(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("package.json", content)
    }

    pub fn create_cargo_toml(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("Cargo.toml", content)
    }

    pub fn create_pyproject_toml(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("pyproject.toml", content)
    }

    pub fn create_go_mod(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("go.mod", content)
    }

    pub fn create_gemfile(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("Gemfile", content)
    }

    pub fn create_composer_json(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("composer.json", content)
    }

    pub fn create_package_lock_json(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("package-lock.json", content)
    }

    pub fn create_yarn_lock(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("yarn.lock", content)
    }

    pub fn create_pnpm_lock_yaml(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("pnpm-lock.yaml", content)
    }

    pub fn create_composer_lock(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("composer.lock", content)
    }

    pub fn create_gemfile_lock(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("Gemfile.lock", content)
    }

    pub fn create_poetry_lock(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("poetry.lock", content)
    }

    pub fn create_cargo_lock(content: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
        create_file("Cargo.lock", content)
    }

    fn create_framework_detector(
        name: &str,
        detection: DetectionType,
        priority: u8,
    ) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection,
            icon: None,
            color: None,
            priority,
            files: vec![],
            root_indicators: vec![],
        }
    }

    pub fn create_package_json_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::NodeEcosystem {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_cargo_toml_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::RustEcosystem {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_pyproject_toml_framework(name: &str, deps: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::PythonEcosystem {
                dependencies: deps.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_go_mod_framework(name: &str, modules: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::GoEcosystem {
                modules: modules.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_gemspec_framework(name: &str, gems: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::RubyEcosystem {
                gems: gems.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_composer_json_framework(name: &str, packages: Vec<&str>) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::PHPEcosystem {
                packages: packages.into_iter().map(String::from).collect(),
            },
            1,
        )
    }

    pub fn create_test_language(name: &str, patterns: Vec<&str>) -> crate::types::ProjectIndicator {
        crate::types::ProjectIndicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            1,
            vec![],
        )
    }

    pub fn create_test_language_with_priority(
        name: &str,
        patterns: Vec<&str>,
        priority: u8,
    ) -> crate::types::ProjectIndicator {
        crate::types::ProjectIndicator::new(
            name.to_string(),
            patterns.iter().map(|s| s.to_string()).collect(),
            "#FF0000".to_string(),
            "🔥".to_string(),
            priority,
            vec![],
        )
    }

    pub fn create_test_file(filename: &str, path: &str) -> crate::types::MatchedFile {
        crate::types::MatchedFile::new(filename.to_string(), path.to_string())
    }

    pub fn create_test_framework_generic(name: &str, priority: u8) -> FrameworkDetector {
        create_framework_detector(
            name,
            DetectionType::FileExists {
                files: vec![format!("{}.toml", name.to_lowercase())],
            },
            priority,
        )
    }

    pub fn create_test_language_with_indicators(
        name: &str,
        indicators: Vec<(&str, f32)>,
    ) -> crate::types::ProjectIndicator {
        use crate::types::IndicatorContext;

        crate::types::ProjectIndicator::with_root_indicators(
            name.to_string(),
            vec![],
            "#000000".to_string(),
            "🔧".to_string(),
            1,
            vec![],
            indicators
                .into_iter()
                .map(|(pattern, weight)| crate::types::RootIndicator {
                    pattern: pattern.to_string(),
                    weight,
                    context: IndicatorContext::default(),
                })
                .collect(),
        )
    }
}

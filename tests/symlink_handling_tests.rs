#[cfg(unix)]
mod unix_symlink_tests {
    use project_indicator::config::built_in_config;
    use project_indicator::detection::engine::DetectionEngine;
    use project_indicator::types::DetectionResult;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    /// Detection with the full built-in configuration
    fn detect(path: &std::path::Path) -> Result<DetectionResult, Box<dyn std::error::Error>> {
        let config = built_in_config();
        let engine =
            DetectionEngine::with_config(config.indicators, config.frameworks, config.detection);
        Ok(engine.detect(path)?)
    }

    fn write_cargo_toml(dir: &std::path::Path) -> std::io::Result<()> {
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
    }

    fn detected_name(result: &DetectionResult) -> Result<&str, Box<dyn std::error::Error>> {
        Ok(&result
            .indicator
            .as_ref()
            .ok_or("expected a detected language")?
            .name)
    }

    #[test]
    fn test_detection_with_circular_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        write_cargo_toml(&project_dir)?;

        let link1 = project_dir.join("link1");
        let link2 = project_dir.join("link2");

        symlink(&link2, &link1)?;
        symlink(&link1, &link2)?;

        let result = detect(&project_dir)?;

        assert_eq!(detected_name(&result)?, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_self_referencing_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        let self_link = project_dir.join("self_link");
        symlink(&project_dir, &self_link)?;

        write_cargo_toml(&project_dir)?;

        let result = detect(&project_dir)?;

        assert_eq!(detected_name(&result)?, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_broken_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        let broken_link = project_dir.join("broken_link");
        symlink("/nonexistent/path", &broken_link)?;

        write_cargo_toml(&project_dir)?;

        let result = detect(&project_dir)?;

        assert_eq!(detected_name(&result)?, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_symlink_chain() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let project_dir = temp_dir.path().join("real_project");
        fs::create_dir(&project_dir)?;
        write_cargo_toml(&project_dir)?;

        let link1 = temp_dir.path().join("link1");
        let link2 = temp_dir.path().join("link2");
        let link3 = temp_dir.path().join("link3");

        symlink(&project_dir, &link1)?;
        symlink(&link1, &link2)?;
        symlink(&link2, &link3)?;

        let result = detect(&link3)?;

        assert_eq!(detected_name(&result)?, "Rust");

        Ok(())
    }

    #[test]
    fn test_symlink_to_individual_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        let package_json = project_dir.join("package.json");
        fs::write(
            &package_json,
            r#"{
  "name": "test-project",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0"
  }
}"#,
        )?;

        let link_to_file = project_dir.join("link_package.json");
        symlink(&package_json, &link_to_file)?;

        let result = detect(&project_dir)?;

        assert!(["JavaScript", "TypeScript"].contains(&detected_name(&result)?));

        Ok(())
    }

    #[test]
    fn test_nested_project_with_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let main_project = temp_dir.path().join("main");
        fs::create_dir(&main_project)?;
        write_cargo_toml(&main_project)?;

        let subdir = main_project.join("vendor");
        fs::create_dir(&subdir)?;

        let external_project = temp_dir.path().join("external");
        fs::create_dir(&external_project)?;
        write_cargo_toml(&external_project)?;

        let link_in_vendor = subdir.join("external_link");
        symlink(&external_project, &link_in_vendor)?;

        let result = detect(&main_project)?;

        assert_eq!(detected_name(&result)?, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_performance_with_many_symlinks() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        write_cargo_toml(&project_dir)?;

        for i in 0..50 {
            let link_name = project_dir.join(format!("link_{}", i));
            if i % 2 == 0 {
                symlink(&project_dir, &link_name)?;
            } else {
                symlink(format!("/nonexistent/{}", i), &link_name)?;
            }
        }

        let start = std::time::Instant::now();
        let result = detect(&project_dir)?;
        let elapsed = start.elapsed();

        assert_eq!(detected_name(&result)?, "Rust");
        assert!(
            elapsed.as_secs() < 5,
            "Detection took too long: {:?}",
            elapsed
        );

        Ok(())
    }
}

/// A symlinked source file must be visible to the full scan, not just root
/// discovery (which stats through symlinks and always saw them).
#[cfg(unix)]
#[test]
fn test_scan_detects_symlinked_file() -> Result<(), Box<dyn std::error::Error>> {
    use project_indicator::detection::DetectionEngine;
    use project_indicator::types::Indicator;

    let outside = tempfile::TempDir::new()?;
    let real_file = outside.path().join("real.rs");
    std::fs::write(&real_file, "fn main() {}")?;

    let project = tempfile::TempDir::new()?;
    std::os::unix::fs::symlink(&real_file, project.path().join("linked.rs"))?;

    // Glob-only language with no root indicators, so detection must go
    // through the full scan rather than upward root discovery
    let language = Indicator::new(
        "Rust".to_string(),
        vec!["*.rs".to_string()],
        "#dea584".to_string(),
        "R".to_string(),
        vec![],
    );
    let engine = DetectionEngine::new(vec![language], vec![]);

    let result = engine.detect(project.path())?;
    let language = result.indicator.as_ref().ok_or("expected a language")?;
    assert_eq!(language.name, "Rust");
    Ok(())
}

//! Symlink handling tests
//!
//! Tests for correct handling of symbolic links in project detection:
//! - Detection through symlinks to project directories
//! - Circular symlink cycle detection and prevention
//! - Multiple symlink chains
//! - Broken symlink handling

#[cfg(unix)]
mod unix_symlink_tests {
    use project_indicator::detection::caches::FileSystemCacheManager;
    use project_indicator::detection::engine::DetectionEngineBuilder;
    use project_indicator::Config;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn test_detection_through_simple_symlink() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temp directory with a real Rust project
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("real_project");
        fs::create_dir(&project_dir)?;

        // Create a Cargo.toml file
        fs::write(
            project_dir.join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )?;

        // Create a src directory with main.rs
        fs::create_dir(project_dir.join("src"))?;
        fs::write(
            project_dir.join("src/main.rs"),
            "fn main() { println!(\"Hello\"); }",
        )?;

        // Create a symlink to the project
        let link_path = temp_dir.path().join("link_to_project");
        symlink(&project_dir, &link_path)?;

        // Test detection through symlink
        // Using builder pattern to inject custom cache with longer TTL
        // This demonstrates how builder enables controlled testing
        let config = Config::load_default()?;

        let custom_cache = FileSystemCacheManager::with_ttl(600); // 10 minutes
        let detector = DetectionEngineBuilder::new(config.languages.clone())
            .with_cache_manager(custom_cache)
            .build();

        let result = detector.detect(&link_path)?;

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_circular_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create a project directory
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        // Create a Cargo.toml file
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;

        // Create a circular symlink: link1 -> link2 -> link1
        let link1 = project_dir.join("link1");
        let link2 = project_dir.join("link2");

        symlink(&link2, &link1)?;
        symlink(&link1, &link2)?;

        // Detection should not crash with circular symlinks
        // Using builder pattern with minimal cache (demonstrates testing control)
        let config = Config::load_default()?;

        let minimal_cache = FileSystemCacheManager::with_ttl(1);
        let detector = DetectionEngineBuilder::new(config.languages.clone())
            .with_cache_manager(minimal_cache)
            .build();

        let result = detector.detect(&project_dir)?;

        // Should still detect Rust project at the root level
        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_self_referencing_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        // Create a self-referencing symlink
        let self_link = project_dir.join("self_link");
        symlink(&project_dir, &self_link)?;

        // Create a valid project file
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;

        // Detection should handle the self-reference gracefully
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();
        let result = detector.detect(&project_dir)?;

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_broken_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        // Create a broken symlink (points to non-existent file)
        let broken_link = project_dir.join("broken_link");
        symlink("/nonexistent/path", &broken_link)?;

        // Create a valid project file
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;

        // Detection should handle broken symlinks gracefully
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();
        let result = detector.detect(&project_dir)?;

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_with_symlink_chain() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create the actual project
        let project_dir = temp_dir.path().join("real_project");
        fs::create_dir(&project_dir)?;
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;

        // Create a chain of symlinks: link3 -> link2 -> link1 -> real_project
        let link1 = temp_dir.path().join("link1");
        let link2 = temp_dir.path().join("link2");
        let link3 = temp_dir.path().join("link3");

        symlink(&project_dir, &link1)?;
        symlink(&link1, &link2)?;
        symlink(&link2, &link3)?;

        // Detection through the chain should work
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();
        let result = detector.detect(&link3)?;

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_symlink_to_individual_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        // Create a package.json
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

        // Create a symlink to the package.json file
        let link_to_file = project_dir.join("link_package.json");
        symlink(&package_json, &link_to_file)?;

        // Detection should work with file symlinks present
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();
        let result = detector.detect(&project_dir)?;

        assert!(result.language.is_some());
        // Should detect JavaScript/TypeScript project
        let language = result
            .language
            .as_ref()
            .ok_or("Expected Some language but got None")?;
        let lang_name = &language.name;
        assert!(lang_name == "JavaScript" || lang_name == "TypeScript");

        Ok(())
    }

    #[test]
    fn test_nested_project_with_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create main project
        let main_project = temp_dir.path().join("main");
        fs::create_dir(&main_project)?;
        fs::write(
            main_project.join("Cargo.toml"),
            "[package]\nname = \"main\"\nversion = \"0.1.0\"",
        )?;

        // Create a subdirectory
        let subdir = main_project.join("vendor");
        fs::create_dir(&subdir)?;

        // Create another project outside
        let external_project = temp_dir.path().join("external");
        fs::create_dir(&external_project)?;
        fs::write(
            external_project.join("Cargo.toml"),
            "[package]\nname = \"external\"\nversion = \"0.1.0\"",
        )?;

        // Symlink external project into vendor directory
        let link_in_vendor = subdir.join("external_link");
        symlink(&external_project, &link_in_vendor)?;

        // Detection at main project should work
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();
        let result = detector.detect(&main_project)?;

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");

        Ok(())
    }

    #[test]
    fn test_detection_performance_with_many_symlinks() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let project_dir = temp_dir.path().join("project");
        fs::create_dir(&project_dir)?;

        // Create a valid project
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )?;

        // Create many symlinks in the project directory
        for i in 0..50 {
            let link_name = project_dir.join(format!("link_{}", i));
            // Create circular symlinks
            if i % 2 == 0 {
                symlink(&project_dir, &link_name)?;
            } else {
                // Create broken symlinks
                symlink(format!("/nonexistent/{}", i), &link_name)?;
            }
        }

        // Detection should complete in reasonable time
        let config = Config::load_default()?;
        let detector = DetectionEngineBuilder::new(config.languages.clone()).build();

        let start = std::time::Instant::now();
        let result = detector.detect(&project_dir)?;
        let elapsed = start.elapsed();

        assert!(result.language.is_some());
        let language = result
            .language
            .ok_or("Expected Some language but got None")?;
        assert_eq!(language.name, "Rust");
        // Should complete in under 5 seconds even with many symlinks
        assert!(
            elapsed.as_secs() < 5,
            "Detection took too long: {:?}",
            elapsed
        );

        Ok(())
    }
}

#[cfg(not(unix))]
mod windows_placeholder_tests {
    #[test]
    fn symlink_tests_are_unix_only() {
        // On Windows, we'd need different symlink creation APIs
        // and administrator privileges. For now, these tests are Unix-only.
        println!("Symlink tests are Unix-only");
    }
}

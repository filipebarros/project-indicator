//! Integration tests for project-indicator

use project_indicator::{
    output::{formatters::format_result, OutputFormat},
    types::*,
    Config, DetectionEngine,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

/// Test helper to create a temporary project directory
fn create_test_project(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    for (file_path, content) in files {
        let file_path = temp_dir.path().join(file_path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(file_path, content).unwrap();
    }

    temp_dir
}

#[test]
fn test_config_serialization() {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::PackageJson {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
    };

    let language = ProjectIndicator {
        name: "TypeScript".to_string(),
        files: vec!["package.json".to_string(), "tsconfig.json".to_string()],
        color: "#3178C6".to_string(),
        icon: "󰛦".to_string(),
        priority: 1,
        frameworks: vec![framework],
    };

    let config = Config {
        meta: ConfigMeta::default(),
        display: DisplayConfig::default(),
        cache: CacheConfig::default(),
        detection: DetectionConfig::default(),
        languages: vec![language],
    };

    // Test TOML serialization
    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("TypeScript"));
    assert!(toml_str.contains("React"));

    // Test deserialization
    let deserialized: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(deserialized.languages.len(), 1);
    assert_eq!(deserialized.languages[0].name, "TypeScript");
    assert_eq!(deserialized.languages[0].frameworks.len(), 1);
    assert_eq!(deserialized.languages[0].frameworks[0].name, "React");
}

#[test]
fn test_detection_types_serialization() {
    let detection_types = vec![
        DetectionType::PackageJson {
            dependencies: vec!["react".to_string(), "vue".to_string()],
        },
        DetectionType::CargoToml {
            dependencies: vec!["serde".to_string()],
        },
        DetectionType::GoMod {
            modules: vec!["github.com/gin-gonic/gin".to_string()],
        },
        DetectionType::PyProjectToml {
            dependencies: vec!["Django".to_string()],
        },
        DetectionType::GemSpec {
            gems: vec!["rails".to_string()],
        },
        DetectionType::ComposerJson {
            packages: vec!["laravel/framework".to_string()],
        },
        DetectionType::FileExists {
            files: vec!["next.config.js".to_string()],
        },
        DetectionType::ConfigFile {
            file: "pyproject.toml".to_string(),
            keys: vec!["tool.poetry".to_string()],
        },
    ];

    for detection_type in detection_types {
        let json = serde_json::to_string(&detection_type).unwrap();
        let deserialized: DetectionType = serde_json::from_str(&json).unwrap();
        assert_eq!(detection_type, deserialized);
    }
}

#[test]
fn test_complex_project_indicator() {
    let react_framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::PackageJson {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
    };

    let nextjs_framework = FrameworkDetector {
        name: "Next.js".to_string(),
        detection: DetectionType::PackageJson {
            dependencies: vec!["next".to_string()],
        },
        icon: Some("▲".to_string()),
        color: Some("#000000".to_string()),
        priority: 1,
        files: vec!["next.config.js".to_string()],
    };

    let typescript_lang = ProjectIndicator {
        name: "TypeScript".to_string(),
        files: vec!["package.json".to_string(), "tsconfig.json".to_string()],
        color: "#3178C6".to_string(),
        icon: "󰛦".to_string(),
        priority: 1,
        frameworks: vec![react_framework, nextjs_framework],
    };

    // Test framework sorting by priority
    let sorted_frameworks = typescript_lang.frameworks_by_priority();
    assert_eq!(sorted_frameworks.len(), 2);
    // Both have priority 1, so order should be maintained
    assert_eq!(sorted_frameworks[0].name, "React");
    assert_eq!(sorted_frameworks[1].name, "Next.js");

    // Test file matching
    let project_files = vec![
        "package.json".to_string(),
        "tsconfig.json".to_string(),
        "src/components/App.tsx".to_string(),
    ];
    assert!(typescript_lang.matches_files(&project_files));
}

#[test]
fn test_detection_result_display() {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::PackageJson {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
    };

    let language = ProjectIndicator {
        name: "TypeScript".to_string(),
        files: vec!["package.json".to_string()],
        color: "#3178C6".to_string(),
        icon: "󰛦".to_string(),
        priority: 1,
        frameworks: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    // Test with framework (should prefer framework icon/color)
    let result_with_framework =
        DetectionResult::new(Some(Arc::new(language.clone())), vec![framework_match], 0.9);

    assert_eq!(result_with_framework.display_icon(), Some("⚛️"));
    assert_eq!(result_with_framework.display_color(), Some("#61DAFB"));

    // Test without framework (should use language icon/color)
    let result_language_only = DetectionResult::new(Some(Arc::new(language)), vec![], 0.8);

    assert_eq!(result_language_only.display_icon(), Some("󰛦"));
    assert_eq!(result_language_only.display_color(), Some("#3178C6"));
}

#[test]
fn test_output_formatting() {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::PackageJson {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result = DetectionResult::new(None, vec![framework_match], 0.9);

    // Test simple format (icon only)
    let simple = format_result(&result, OutputFormat::Simple);
    assert_eq!(simple, "⚛️");

    // Test full format (icon + frameworks + color)
    let full = format_result(&result, OutputFormat::Full);
    assert_eq!(full, "⚛️ (React)|#61DAFB");

    // Test JSON format
    let json = format_result(&result, OutputFormat::Json);
    assert!(json.contains("\"frameworks\":[\"React\"]"));
    assert!(json.contains("\"icon\":\"⚛️\""));
    assert!(json.contains("\"color\":\"#61DAFB\""));
}

#[test]
fn test_detection_engine_creation() {
    let language = ProjectIndicator {
        name: "Rust".to_string(),
        files: vec!["Cargo.toml".to_string()],
        color: "#DEA584".to_string(),
        icon: "".to_string(),
        priority: 1,
        frameworks: vec![],
    };

    let engine = DetectionEngine::new(vec![language]);

    // Test detection of Rust project
    let temp_dir = create_test_project(&[("Cargo.toml", "[package]\nname = \"test\"")]);
    let result = engine.detect(temp_dir.path()).unwrap();

    assert!(!result.is_empty()); // Should detect Rust project
    assert_eq!(result.language.as_ref().unwrap().name, "Rust");
    assert!(result.confidence > 0.0);
}

#[test]
fn test_multiple_framework_priorities() {
    let low_priority_framework = FrameworkDetector {
        name: "LowPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 5,
        files: vec![],
    };

    let high_priority_framework = FrameworkDetector {
        name: "HighPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 1,
        files: vec![],
    };

    let medium_priority_framework = FrameworkDetector {
        name: "MediumPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 3,
        files: vec![],
    };

    let language = ProjectIndicator {
        name: "Test".to_string(),
        files: vec![],
        color: "#000000".to_string(),
        icon: "".to_string(),
        priority: 1,
        frameworks: vec![
            low_priority_framework,
            high_priority_framework,
            medium_priority_framework,
        ],
    };

    let sorted = language.frameworks_by_priority();
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].name, "HighPriority"); // priority 1
    assert_eq!(sorted[1].name, "MediumPriority"); // priority 3
    assert_eq!(sorted[2].name, "LowPriority"); // priority 5
}

#[test]
fn test_config_defaults() {
    let config = Config::default();

    assert_eq!(config.meta.version, "2.0");
    assert!(config.display.show_frameworks);
    assert_eq!(config.display.max_frameworks, 2);
    assert_eq!(config.display.framework_separator, "+");
    assert_eq!(config.cache.ttl_seconds, 300);
    assert!(config.languages.is_empty());
}

#[test]
fn test_wildcard_patterns() {
    let cpp_lang = ProjectIndicator {
        name: "C++".to_string(),
        files: vec![
            "*.cpp".to_string(),
            "*.hpp".to_string(),
            "CMakeLists.txt".to_string(),
        ],
        color: "#00599C".to_string(),
        icon: "".to_string(),
        priority: 1,
        frameworks: vec![],
    };

    // Test various file combinations
    let cpp_files = vec!["main.cpp".to_string(), "header.hpp".to_string()];
    assert!(cpp_lang.matches_files(&cpp_files));

    let cmake_files = vec!["CMakeLists.txt".to_string()];
    assert!(cpp_lang.matches_files(&cmake_files));

    let no_match_files = vec!["main.py".to_string(), "package.json".to_string()];
    assert!(!cpp_lang.matches_files(&no_match_files));

    // Test partial wildcard matches
    let partial_files = vec!["src/main.cpp".to_string()]; // Should match *.cpp
    assert!(cpp_lang.matches_files(&partial_files));
}

mod common;

use common::create_test_project;
use project_indicator::{
    detection::DetectionEngineBuilder,
    output::{format_result, OutputFormat},
    types::*,
    Config,
};
use std::sync::Arc;

#[test]
fn test_config_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let language = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string(), "tsconfig.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![framework],
    );

    let config = Config {
        meta: ConfigMeta::default(),
        display: DisplayConfig::default(),
        cache: CacheConfig::default(),
        detection: DetectionConfig::default(),
        languages: vec![language],
    };

    let toml_str = toml::to_string(&config)?;
    assert!(toml_str.contains("TypeScript"));
    assert!(toml_str.contains("React"));

    let deserialized: Config = toml::from_str(&toml_str)?;
    assert_eq!(deserialized.languages.len(), 1);
    assert_eq!(deserialized.languages[0].name, "TypeScript");
    assert_eq!(deserialized.languages[0].frameworks.len(), 1);
    assert_eq!(deserialized.languages[0].frameworks[0].name, "React");
    Ok(())
}

#[test]
fn test_detection_types_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let detection_types = vec![
        DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string(), "vue".to_string()],
        },
        DetectionType::RustEcosystem {
            dependencies: vec!["serde".to_string()],
        },
        DetectionType::GoEcosystem {
            modules: vec!["github.com/gin-gonic/gin".to_string()],
        },
        DetectionType::PythonEcosystem {
            dependencies: vec!["Django".to_string()],
        },
        DetectionType::RubyEcosystem {
            gems: vec!["rails".to_string()],
        },
        DetectionType::PHPEcosystem {
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
        let json = serde_json::to_string(&detection_type)?;
        let deserialized: DetectionType = serde_json::from_str(&json)?;
        assert_eq!(detection_type, deserialized);
    }
    Ok(())
}

#[test]
fn test_complex_project_indicator() -> Result<(), Box<dyn std::error::Error>> {
    let react_framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let nextjs_framework = FrameworkDetector {
        name: "Next.js".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["next".to_string()],
        },
        icon: Some("▲".to_string()),
        color: Some("#000000".to_string()),
        priority: 1,
        files: vec!["next.config.js".to_string()],
        root_indicators: vec![],
    };

    let typescript_lang = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string(), "tsconfig.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![react_framework, nextjs_framework],
    );

    let sorted_frameworks = typescript_lang.frameworks_by_priority();
    assert_eq!(sorted_frameworks.len(), 2);
    assert_eq!(sorted_frameworks[0].name, "React");
    assert_eq!(sorted_frameworks[1].name, "Next.js");

    let project_files = vec![
        "package.json".to_string(),
        "tsconfig.json".to_string(),
        "src/components/App.tsx".to_string(),
    ];
    assert!(typescript_lang.matches_files(&project_files));
    Ok(())
}

#[test]
fn test_detection_result_display() -> Result<(), Box<dyn std::error::Error>> {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let language = ProjectIndicator::new(
        "TypeScript".to_string(),
        vec!["package.json".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        1,
        vec![],
    );

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result_with_framework =
        DetectionResult::new(Some(Arc::new(language.clone())), vec![framework_match], 0.9);

    assert_eq!(result_with_framework.display_icon(), Some("⚛️"));
    assert_eq!(result_with_framework.display_color(), Some("#61DAFB"));

    let result_language_only = DetectionResult::new(Some(Arc::new(language)), vec![], 0.8);

    assert_eq!(result_language_only.display_icon(), Some("󰛦"));
    assert_eq!(result_language_only.display_color(), Some("#3178C6"));
    Ok(())
}

#[test]
fn test_output_formatting() -> Result<(), Box<dyn std::error::Error>> {
    let framework = FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        },
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    let result = DetectionResult::new(None, vec![framework_match], 0.9);

    let simple = format_result(&result, OutputFormat::Simple);
    assert_eq!(simple, "⚛️");

    let full = format_result(&result, OutputFormat::Full);
    assert_eq!(full, "⚛️|#61DAFB");

    let json = format_result(&result, OutputFormat::Json);
    assert!(json.contains("\"frameworks\":[\"React\"]"));
    assert!(json.contains("\"icon\":\"⚛️\""));
    assert!(json.contains("\"color\":\"#61DAFB\""));
    Ok(())
}

#[test]
fn test_detection_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
    let language = ProjectIndicator::new(
        "Rust".to_string(),
        vec!["Cargo.toml".to_string()],
        "#DEA584".to_string(),
        "".to_string(),
        1,
        vec![],
    );

    let engine = DetectionEngineBuilder::new(vec![language]).build();

    let temp_dir = create_test_project(&[("Cargo.toml", "[package]\nname = \"test\"")])?;
    let result = engine.detect(temp_dir.path())?;

    assert!(!result.is_empty());
    assert_eq!(
        result
            .language
            .as_ref()
            .ok_or("Failed to get language reference")?
            .name,
        "Rust"
    );
    assert!(result.confidence > 0.0);
    Ok(())
}

#[test]
fn test_multiple_framework_priorities() -> Result<(), Box<dyn std::error::Error>> {
    let low_priority_framework = FrameworkDetector {
        name: "LowPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 5,
        files: vec![],
        root_indicators: vec![],
    };

    let high_priority_framework = FrameworkDetector {
        name: "HighPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    };

    let medium_priority_framework = FrameworkDetector {
        name: "MediumPriority".to_string(),
        detection: DetectionType::FileExists { files: vec![] },
        icon: None,
        color: None,
        priority: 3,
        files: vec![],
        root_indicators: vec![],
    };

    let language = ProjectIndicator::new(
        "Test".to_string(),
        vec![],
        "#000000".to_string(),
        "".to_string(),
        1,
        vec![
            low_priority_framework,
            high_priority_framework,
            medium_priority_framework,
        ],
    );

    let sorted = language.frameworks_by_priority();
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].name, "HighPriority");
    assert_eq!(sorted[1].name, "MediumPriority");
    assert_eq!(sorted[2].name, "LowPriority");
    Ok(())
}

#[test]
fn test_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::default();

    assert_eq!(config.meta.version, "2.0");
    assert!(config.display.show_frameworks);
    assert_eq!(config.display.max_frameworks, 2);
    assert_eq!(config.display.framework_separator, "+");
    assert_eq!(config.cache.ttl_seconds, 300);
    assert!(config.languages.is_empty());
    Ok(())
}

#[test]
fn test_wildcard_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let cpp_lang = ProjectIndicator::new(
        "C++".to_string(),
        vec![
            "*.cpp".to_string(),
            "*.hpp".to_string(),
            "CMakeLists.txt".to_string(),
        ],
        "#00599C".to_string(),
        "".to_string(),
        1,
        vec![],
    );

    let cpp_files = vec!["main.cpp".to_string(), "header.hpp".to_string()];
    assert!(cpp_lang.matches_files(&cpp_files));

    let cmake_files = vec!["CMakeLists.txt".to_string()];
    assert!(cpp_lang.matches_files(&cmake_files));

    let no_match_files = vec!["main.py".to_string(), "package.json".to_string()];
    assert!(!cpp_lang.matches_files(&no_match_files));

    let partial_files = vec!["src/main.cpp".to_string()];
    assert!(cpp_lang.matches_files(&partial_files));
    Ok(())
}

use crate::constants::*;
use crate::types::{
    CacheConfig, ConfigMeta, DetectionConfig, DetectionType, DisplayConfig, FrameworkDetector,
    IndicatorContext, ProjectIndicator, RootIndicator,
};

pub fn root_indicator(pattern: &str, weight: f32, context: IndicatorContext) -> RootIndicator {
    RootIndicator {
        pattern: pattern.to_string(),
        weight,
        context,
    }
}

pub fn framework(
    name: &str,
    detection: DetectionType,
    icon: Option<String>,
    color: Option<&str>,
    priority: u8,
    root_indicators: Vec<RootIndicator>,
) -> FrameworkDetector {
    FrameworkDetector {
        name: name.to_string(),
        detection,
        icon,
        color: color.map(String::from),
        priority,
        files: vec![],
        root_indicators,
    }
}

pub fn simple_framework(
    name: &str,
    detection: DetectionType,
    icon: Option<String>,
    color: Option<&str>,
    priority: u8,
) -> FrameworkDetector {
    framework(name, detection, icon, color, priority, vec![])
}

pub fn create_react_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string()],
        },
        icon: Some(nerd_icon("e7ba")),
        color: Some("#61dafb".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    }
}

pub fn create_react_typescript_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "React".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["react".to_string(), "@types/react".to_string()],
        },
        icon: Some(nerd_icon("e7ba")),
        color: Some("#61dafb".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    }
}

pub fn create_angular_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "Angular".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["@angular/core".to_string()],
        },
        icon: Some(nerd_icon("e753")),
        color: Some("#dd0031".to_string()),
        priority: 2,
        files: vec![],
        root_indicators: vec![RootIndicator {
            pattern: "angular.json".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn create_nextjs_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "Next.js".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["next".to_string()],
        },
        icon: Some(nerd_icon("e83e")),
        color: Some("#000000".to_string()),
        priority: 3,
        files: vec!["next.config.js".to_string(), "next.config.mjs".to_string()],
        root_indicators: vec![RootIndicator {
            pattern: "next.config.js".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn create_nextjs_typescript_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "Next.js".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["next".to_string()],
        },
        icon: Some(nerd_icon("e83e")),
        color: Some("#000000".to_string()),
        priority: 3,
        files: vec!["next.config.js".to_string(), "next.config.ts".to_string()],
        root_indicators: vec![RootIndicator {
            pattern: "next.config.ts".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn create_vue_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "Vue".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["vue".to_string()],
        },
        icon: Some(nerd_icon("e8dc")),
        color: Some("#4fc08d".to_string()),
        priority: 2,
        files: vec![],
        root_indicators: vec![],
    }
}

pub fn create_nestjs_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "NestJS".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["@nestjs/core".to_string()],
        },
        icon: Some(nerd_icon("e83b")),
        color: Some("#e0234e".to_string()),
        priority: 4,
        files: vec!["nest-cli.json".to_string()],
        root_indicators: vec![RootIndicator {
            pattern: "nest-cli.json".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn create_astro_framework() -> FrameworkDetector {
    FrameworkDetector {
        name: "Astro".to_string(),
        detection: DetectionType::NodeEcosystem {
            dependencies: vec!["astro".to_string()],
        },
        icon: Some(nerd_icon("e735")),
        color: Some("#ff5d01".to_string()),
        priority: 3,
        files: vec![
            "astro.config.mjs".to_string(),
            "astro.config.js".to_string(),
            "astro.config.ts".to_string(),
        ],
        root_indicators: vec![RootIndicator {
            pattern: "astro.config.mjs".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn nerd_icon(hex_code: &str) -> String {
    if let Ok(code_point) = u32::from_str_radix(hex_code, 16) {
        if let Some(character) = char::from_u32(code_point) {
            return character.to_string();
        }
    }
    "".to_string()
}

pub fn node_lockfiles() -> Vec<String> {
    vec![
        PACKAGE_JSON.to_string(),
        PACKAGE_LOCK_JSON.to_string(),
        YARN_LOCK.to_string(),
        PNPM_LOCK_YAML.to_string(),
    ]
}

pub fn node_lockfile_root_indicators() -> Vec<RootIndicator> {
    vec![
        RootIndicator {
            pattern: PACKAGE_JSON.to_string(),
            weight: 0.95,
            context: IndicatorContext::LanguageRoot,
        },
        RootIndicator {
            pattern: PACKAGE_LOCK_JSON.to_string(),
            weight: 0.8,
            context: IndicatorContext::LanguageRoot,
        },
        RootIndicator {
            pattern: YARN_LOCK.to_string(),
            weight: 0.8,
            context: IndicatorContext::LanguageRoot,
        },
        RootIndicator {
            pattern: PNPM_LOCK_YAML.to_string(),
            weight: 0.8,
            context: IndicatorContext::LanguageRoot,
        },
    ]
}

pub fn vcs_root_indicators() -> Vec<RootIndicator> {
    vec![
        RootIndicator {
            pattern: DOT_GIT.to_string(),
            weight: 1.0,
            context: IndicatorContext::VersionControl,
        },
        RootIndicator {
            pattern: ".hg".to_string(),
            weight: 1.0,
            context: IndicatorContext::VersionControl,
        },
        RootIndicator {
            pattern: ".svn".to_string(),
            weight: 1.0,
            context: IndicatorContext::VersionControl,
        },
    ]
}

pub fn generate_root_indicators_simple_max_weight(
    languages: &[ProjectIndicator],
) -> Vec<RootIndicator> {
    use std::collections::HashMap;

    let mut indicator_weights: HashMap<String, f32> = HashMap::new();

    for vcs_indicator in vcs_root_indicators() {
        indicator_weights.insert(vcs_indicator.pattern, vcs_indicator.weight);
    }

    for language in languages {
        for root_indicator in &language.root_indicators {
            let existing_weight = indicator_weights
                .get(&root_indicator.pattern)
                .unwrap_or(&0.0);
            indicator_weights.insert(
                root_indicator.pattern.clone(),
                existing_weight.max(root_indicator.weight),
            );
        }

        for framework in &language.frameworks {
            for root_indicator in &framework.root_indicators {
                let existing_weight = indicator_weights
                    .get(&root_indicator.pattern)
                    .unwrap_or(&0.0);
                indicator_weights.insert(
                    root_indicator.pattern.clone(),
                    existing_weight.max(root_indicator.weight),
                );
            }
        }
    }

    indicator_weights
        .into_iter()
        .map(|(pattern, weight)| RootIndicator {
            pattern,
            weight,
            context: IndicatorContext::LanguageRoot,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    pub show_frameworks: bool,
    pub max_frameworks: usize,
    pub framework_separator: String,
    pub cache_enabled: bool,
    pub ttl_seconds: u64,
    pub max_entries: usize,
    pub max_upward_traversal: usize,
    pub require_vcs_root: bool,
    pub confidence_threshold: f32,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            show_frameworks: true,
            max_frameworks: 3,
            framework_separator: " + ".to_string(),
            cache_enabled: true,
            ttl_seconds: 300,
            max_entries: 1000,
            max_upward_traversal: 3,
            require_vcs_root: false,
            confidence_threshold: 0.3,
        }
    }

    pub fn display(
        mut self,
        show_frameworks: bool,
        max_frameworks: usize,
        framework_separator: &str,
    ) -> Self {
        self.show_frameworks = show_frameworks;
        self.max_frameworks = max_frameworks;
        self.framework_separator = framework_separator.to_string();
        self
    }

    pub fn cache(mut self, enabled: bool, ttl_seconds: u64, max_entries: usize) -> Self {
        self.cache_enabled = enabled;
        self.ttl_seconds = ttl_seconds;
        self.max_entries = max_entries;
        self
    }

    pub fn detection(
        mut self,
        max_upward_traversal: usize,
        require_vcs_root: bool,
        confidence_threshold: f32,
    ) -> Self {
        self.max_upward_traversal = max_upward_traversal;
        self.require_vcs_root = require_vcs_root;
        self.confidence_threshold = confidence_threshold;
        self
    }

    pub fn build(self) -> (ConfigMeta, DisplayConfig, CacheConfig, DetectionConfig) {
        let meta = ConfigMeta {
            version: "2.0".to_string(),
        };

        let display = DisplayConfig {
            show_frameworks: self.show_frameworks,
            max_frameworks: self.max_frameworks,
            framework_separator: self.framework_separator,
        };

        let cache = CacheConfig {
            enabled: self.cache_enabled,
            ttl_seconds: self.ttl_seconds,
            max_entries: self.max_entries,
        };

        let detection = DetectionConfig {
            max_upward_traversal: self.max_upward_traversal,
            require_vcs_root: self.require_vcs_root,
            confidence_threshold: self.confidence_threshold,
            root_indicators: vcs_root_indicators(),
            max_depth: 1,
            detection_mode: crate::types::DetectionMode::default(),
            max_matches_per_pattern: 15,
            small_project_threshold: 50,
            extreme_size_threshold: 500,
        };

        (meta, display, cache, detection)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

use crate::constants::*;
use crate::types::{
    ConfigMeta, DetectionConfig, DetectionType, DisplayConfig, Ecosystem, Framework, Indicator,
    IndicatorContext, RootIndicator,
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
    ecosystems: Vec<Ecosystem>,
    detection: DetectionType,
    icon: Option<String>,
    color: Option<&str>,
    priority: u8,
    root_indicators: Vec<RootIndicator>,
) -> Framework {
    Framework {
        name: name.to_string(),
        ecosystems,
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
    ecosystems: Vec<Ecosystem>,
    detection: DetectionType,
    icon: Option<String>,
    color: Option<&str>,
    priority: u8,
) -> Framework {
    framework(name, ecosystems, detection, icon, color, priority, vec![])
}

pub fn create_react_framework() -> Framework {
    Framework {
        name: "React".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["react".to_string()],
        },
        icon: Some(nerd_icon("e7ba")),
        color: Some("#61dafb".to_string()),
        priority: 1,
        files: vec![],
        root_indicators: vec![],
    }
}

pub fn create_angular_framework() -> Framework {
    Framework {
        name: "Angular".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
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

pub fn create_nextjs_framework() -> Framework {
    Framework {
        name: "Next.js".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["next".to_string()],
        },
        icon: Some(nerd_icon("e83e")),
        color: Some("#000000".to_string()),
        priority: 3,
        files: vec![
            "next.config.js".to_string(),
            "next.config.mjs".to_string(),
            "next.config.ts".to_string(),
        ],
        root_indicators: vec![
            RootIndicator {
                pattern: "next.config.js".to_string(),
                weight: 0.9,
                context: IndicatorContext::FrameworkRoot,
            },
            RootIndicator {
                pattern: "next.config.ts".to_string(),
                weight: 0.9,
                context: IndicatorContext::FrameworkRoot,
            },
        ],
    }
}

pub fn create_vue_framework() -> Framework {
    Framework {
        name: "Vue".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["vue".to_string()],
        },
        icon: Some(nerd_icon("e8dc")),
        color: Some("#4fc08d".to_string()),
        priority: 2,
        files: vec![],
        root_indicators: vec![],
    }
}

pub fn create_nestjs_framework() -> Framework {
    Framework {
        name: "NestJS".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
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

pub fn create_astro_framework() -> Framework {
    Framework {
        name: "Astro".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
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

pub fn create_vite_framework() -> Framework {
    Framework {
        name: "Vite".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["vite".to_string()],
        },
        icon: None,
        color: Some("#646cff".to_string()),
        // Build tooling: app frameworks (React, Svelte, …) win the display
        priority: 5,
        files: vec![
            "vite.config.js".to_string(),
            "vite.config.ts".to_string(),
            "vite.config.mjs".to_string(),
        ],
        root_indicators: vec![],
    }
}

pub fn create_svelte_framework() -> Framework {
    Framework {
        name: "Svelte".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["svelte".to_string(), "@sveltejs/kit".to_string()],
        },
        icon: None,
        color: Some("#ff3e00".to_string()),
        priority: 2,
        files: vec!["svelte.config.js".to_string()],
        root_indicators: vec![RootIndicator {
            pattern: "svelte.config.js".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
        }],
    }
}

pub fn create_solid_framework() -> Framework {
    Framework {
        name: "SolidJS".to_string(),
        ecosystems: vec![Ecosystem::Npm],
        detection: DetectionType::Dependencies {
            dependencies: vec!["solid-js".to_string()],
        },
        icon: None,
        color: Some("#2c4f7c".to_string()),
        priority: 2,
        files: vec![],
        root_indicators: vec![],
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
        "bun.lockb".to_string(),
        "bun.lock".to_string(),
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
        RootIndicator {
            pattern: "bun.lockb".to_string(),
            weight: 0.8,
            context: IndicatorContext::LanguageRoot,
        },
        RootIndicator {
            pattern: "bun.lock".to_string(),
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
    indicators: &[Indicator],
    frameworks: &[Framework],
) -> Vec<RootIndicator> {
    use std::collections::HashMap;

    let mut indicator_weights: HashMap<String, f32> = HashMap::new();

    for vcs_indicator in vcs_root_indicators() {
        indicator_weights.insert(vcs_indicator.pattern, vcs_indicator.weight);
    }

    for indicator in indicators {
        for root_indicator in &indicator.root_indicators {
            let existing_weight = indicator_weights
                .get(&root_indicator.pattern)
                .unwrap_or(&0.0);
            indicator_weights.insert(
                root_indicator.pattern.clone(),
                existing_weight.max(root_indicator.weight),
            );
        }
    }

    for framework in frameworks {
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

    pub fn build(self) -> (ConfigMeta, DisplayConfig, DetectionConfig) {
        let meta = ConfigMeta {
            version: "3.0".to_string(),
        };

        let display = DisplayConfig {
            show_frameworks: self.show_frameworks,
            max_frameworks: self.max_frameworks,
            framework_separator: self.framework_separator,
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

        (meta, display, detection)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

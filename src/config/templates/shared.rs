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
        alternative_group: None,
    }
}

/// Like `root_indicator`, but tags this pattern as one of several
/// mutually-exclusive alternatives sharing `group` (e.g. Java's pom.xml vs
/// build.gradle). Confidence scoring takes the max weight within a group
/// instead of summing every alternative, since a real project only ever
/// uses one of them.
pub fn root_indicator_grouped(
    pattern: &str,
    weight: f32,
    context: IndicatorContext,
    group: &str,
) -> RootIndicator {
    RootIndicator {
        pattern: pattern.to_string(),
        weight,
        context,
        alternative_group: Some(group.to_string()),
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
        root_indicators: vec![RootIndicator {
            pattern: "angular.json".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
            alternative_group: None,
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
        root_indicators: vec![
            RootIndicator {
                pattern: "next.config.js".to_string(),
                weight: 0.9,
                context: IndicatorContext::FrameworkRoot,
                alternative_group: None,
            },
            RootIndicator {
                pattern: "next.config.ts".to_string(),
                weight: 0.9,
                context: IndicatorContext::FrameworkRoot,
                alternative_group: None,
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
        root_indicators: vec![RootIndicator {
            pattern: "nest-cli.json".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
            alternative_group: None,
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
        root_indicators: vec![RootIndicator {
            pattern: "astro.config.mjs".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
            alternative_group: None,
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
        root_indicators: vec![RootIndicator {
            pattern: "svelte.config.js".to_string(),
            weight: 0.9,
            context: IndicatorContext::FrameworkRoot,
            alternative_group: None,
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
    ]
}

pub fn node_lockfile_root_indicators() -> Vec<RootIndicator> {
    vec![
        // Not an alternative to the lockfiles below — package.json co-occurs
        // with whichever lockfile (or none) is present.
        root_indicator(PACKAGE_JSON, 0.95, IndicatorContext::LanguageRoot),
        // npm/yarn/pnpm are mutually-exclusive package-manager choices: a
        // real project only ever has one of these lockfiles, so they
        // contribute their max weight to scoring, not their sum.
        root_indicator_grouped(
            PACKAGE_LOCK_JSON,
            0.8,
            IndicatorContext::LanguageRoot,
            "node-lockfile",
        ),
        root_indicator_grouped(
            YARN_LOCK,
            0.8,
            IndicatorContext::LanguageRoot,
            "node-lockfile",
        ),
        root_indicator_grouped(
            PNPM_LOCK_YAML,
            0.8,
            IndicatorContext::LanguageRoot,
            "node-lockfile",
        ),
    ]
}

pub fn node_runtime_config_files() -> Vec<String> {
    vec![
        ".npmrc".to_string(),
        ".yarnrc".to_string(),
        ".yarnrc.yml".to_string(),
        "pnpm-workspace.yaml".to_string(),
        ".nvmrc".to_string(),
        ".node-version".to_string(),
    ]
}

/// Supporting evidence beyond the manifest/lockfile: package-manager config
/// files and Node version pins. Weaker signals than a lockfile, but real and
/// common. Grouped separately from `node_lockfile_root_indicators`'s
/// lockfile group so they still add corroborating value when they co-occur
/// with a lockfile, which they usually do.
pub fn node_runtime_config_root_indicators() -> Vec<RootIndicator> {
    vec![
        // npm/yarn/pnpm config files are alternatives to each other (a
        // project uses one package manager's config, not several).
        root_indicator_grouped(
            ".npmrc",
            0.4,
            IndicatorContext::LanguageRoot,
            "node-pm-config",
        ),
        root_indicator_grouped(
            ".yarnrc",
            0.6,
            IndicatorContext::LanguageRoot,
            "node-pm-config",
        ),
        root_indicator_grouped(
            ".yarnrc.yml",
            0.7,
            IndicatorContext::LanguageRoot,
            "node-pm-config",
        ),
        root_indicator_grouped(
            "pnpm-workspace.yaml",
            0.7,
            IndicatorContext::LanguageRoot,
            "node-pm-config",
        ),
        // .nvmrc vs .node-version are alternative conventions for the same
        // thing (a Node version pin), not package-manager-specific.
        root_indicator_grouped(
            ".nvmrc",
            0.6,
            IndicatorContext::LanguageRoot,
            "node-version-pin",
        ),
        root_indicator_grouped(
            ".node-version",
            0.55,
            IndicatorContext::LanguageRoot,
            "node-version-pin",
        ),
    ]
}

pub fn vcs_root_indicators() -> Vec<RootIndicator> {
    vec![
        root_indicator(DOT_GIT, 1.0, IndicatorContext::VersionControl),
        root_indicator(".hg", 1.0, IndicatorContext::VersionControl),
        root_indicator(".svn", 1.0, IndicatorContext::VersionControl),
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
            alternative_group: None,
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
    pub max_depth: usize,
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
            max_depth: 1,
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

    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
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
            max_depth: self.max_depth,
            detection_mode: crate::types::DetectionMode::default(),
        };

        (meta, display, detection)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

use crate::constants::*;
use crate::types::{DetectionType, Ecosystem, Framework, IndicatorContext, RootIndicator};

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

/// A framework detected by any of `dependencies` in the manifests of its
/// ecosystems. `icon` is a nerd-font code point in hex.
pub fn dependency_framework(
    name: &str,
    ecosystems: &[Ecosystem],
    dependencies: &[&str],
    icon: Option<&str>,
    color: Option<&str>,
    priority: u8,
) -> Framework {
    framework(
        name,
        ecosystems,
        DetectionType::Dependencies(dependencies.iter().map(|d| d.to_string()).collect()),
        icon,
        color,
        priority,
    )
}

/// A framework detected by the presence of any of `files`
pub fn file_framework(
    name: &str,
    ecosystems: &[Ecosystem],
    files: &[&str],
    icon: Option<&str>,
    color: Option<&str>,
    priority: u8,
) -> Framework {
    framework(
        name,
        ecosystems,
        DetectionType::FileExists(files.iter().map(|f| f.to_string()).collect()),
        icon,
        color,
        priority,
    )
}

fn framework(
    name: &str,
    ecosystems: &[Ecosystem],
    detection: DetectionType,
    icon: Option<&str>,
    color: Option<&str>,
    priority: u8,
) -> Framework {
    Framework {
        name: name.to_string(),
        ecosystems: ecosystems.to_vec(),
        detection,
        icon: icon.map(nerd_icon),
        color: color.map(String::from),
        priority,
    }
}

pub fn nerd_icon(hex_code: &str) -> String {
    u32::from_str_radix(hex_code, 16)
        .ok()
        .and_then(char::from_u32)
        .map_or_else(String::new, String::from)
}

fn language_alternative(pattern: &str, weight: f32, group: &str) -> RootIndicator {
    root_indicator_grouped(pattern, weight, IndicatorContext::LanguageRoot, group)
}

/// Package-manager configs and version pins of the Node-based indicators:
/// evidence for the scan, but not project markers on their own.
pub const NODE_CONFIG_FILES: [&str; 7] = [
    ".npmrc",
    ".yarnrc",
    ".yarnrc.yml",
    "pnpm-workspace.yaml",
    "bunfig.toml",
    ".nvmrc",
    ".node-version",
];

/// Manifest and lockfile markers shared by the Node-based
/// indicators (JavaScript, TypeScript); the file patterns follow from them.
pub fn node_root_indicators() -> Vec<RootIndicator> {
    vec![
        // Not an alternative to the lockfiles — package.json co-occurs with
        // whichever lockfile (or none) is present. Weighted below the other
        // languages' manifests: it is also the tooling file of Python, Ruby
        // and PHP projects, so beside one of them it must not win.
        root_indicator(PACKAGE_JSON, 0.9, IndicatorContext::LanguageRoot),
        // npm/yarn/pnpm are mutually-exclusive package-manager choices: a
        // real project only ever has one of these lockfiles, so they
        // contribute their max weight to scoring, not their sum.
        language_alternative(PACKAGE_LOCK_JSON, 0.8, "node-lockfile"),
        language_alternative(YARN_LOCK, 0.8, "node-lockfile"),
        language_alternative(PNPM_LOCK_YAML, 0.8, "node-lockfile"),
        // Bun's lockfile comes in a text and a binary format
        language_alternative("bun.lock", 0.8, "node-lockfile"),
        language_alternative("bun.lockb", 0.8, "node-lockfile"),
    ]
}

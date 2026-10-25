//! Detection, display and file-metadata configuration. Serde derives back the
//! TOML config file; unknown keys are rejected (`deny_unknown_fields`).

use serde::{Deserialize, Serialize};

// Unknown keys fail loudly (deny_unknown_fields); OMITTED keys take the
// Default value, so a partial [detection]/[display] section is valid
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct DetectionConfig {
    pub require_vcs_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct DisplayConfig {
    /// How many frameworks to list; 0 hides them.
    pub max_frameworks: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { max_frameworks: 5 }
    }
}

//! Shared fixtures for unit tests.

use crate::types::{DetectionResult, DetectionType, Framework, FrameworkMatch, Indicator};
use std::{fs, sync::Arc};
use tempfile::TempDir;

/// A TypeScript result (confidence 0.9) with a React framework match
pub fn create_test_result() -> DetectionResult {
    let language = Indicator::new(
        "TypeScript".to_string(),
        vec!["tsconfig.json".to_string(), "*.ts".to_string()],
        "#3178C6".to_string(),
        "󰛦".to_string(),
        vec![],
    );

    let framework = Framework {
        name: "React".to_string(),
        ecosystems: vec![],
        detection: DetectionType::Dependencies(vec!["react".to_string()]),
        icon: Some("⚛️".to_string()),
        color: Some("#61DAFB".to_string()),
        priority: 1,
    };

    let framework_match = FrameworkMatch::new(framework, 0.9, vec!["package.json".to_string()]);

    DetectionResult::new(Some(Arc::new(language)), vec![framework_match], 0.9)
}

/// Encode text as UTF-16LE with a byte-order mark.
pub fn utf16le_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

/// A temp directory with `Cargo.toml`, `main.rs` and `src/lib.rs`
pub fn create_test_rust_project() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"")?;
    fs::write(root.join("main.rs"), "fn main() {}")?;

    let src_dir = root.join("src");
    fs::create_dir(&src_dir)?;
    fs::write(src_dir.join("lib.rs"), "// lib")?;

    Ok(temp_dir)
}

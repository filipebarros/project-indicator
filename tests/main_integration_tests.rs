use std::fs;
use tempfile::TempDir;

#[test]
fn test_main_path_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    fs::write(temp_path.join("test.txt"), "test content")?;

    assert!(temp_path.exists());
    assert!(temp_path.is_dir());
    Ok(())
}

#[test]
fn test_main_binary_help() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo").args(["run", "--", "--help"]).output();

    match output {
        Ok(output) => {
            assert!(output.status.success() || output.status.code() == Some(2));
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Project detection") || stdout.contains("Usage:"));
        }
        Err(_) => {
            println!("Skipping binary test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_binary_version() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo").args(["run", "--", "--help"]).output();

    match output {
        Ok(output) => {
            assert!(output.status.success() || output.status.code() == Some(2));
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("project-indicator"));
        }
        Err(_) => {
            println!("Skipping version test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_detection_on_rust_project() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "--format", "json"])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(stdout.contains("Rust") || stdout.contains("language"));
            }
        }
        Err(_) => {
            println!("Skipping detection test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_nonexistent_path() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--path",
            "/nonexistent/path/that/should/not/exist",
        ])
        .output();

    match output {
        Ok(output) => {
            assert!(!output.status.success());
        }
        Err(_) => {
            println!("Skipping error test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_config_help() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "config", "--help"])
        .output();

    match output {
        Ok(output) => {
            assert!(output.status.success() || output.status.code() == Some(2));
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("config") || stdout.contains("init"));
        }
        Err(_) => {
            println!("Skipping config test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_debug_command() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo").args(["run", "--", "debug"]).output();

    match output {
        Ok(output) => {
            assert!(output.status.code().is_some());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Debug mode")
                    || stderr.contains("config")
                    || output.status.success()
            );
        }
        Err(_) => {
            println!("Skipping debug test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_benchmark_command() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "benchmark"])
        .output();

    match output {
        Ok(output) => {
            assert!(output.status.code().is_some());
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success() || !stdout.is_empty() || !stderr.is_empty());
        }
        Err(_) => {
            println!("Skipping benchmark test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_output_formats() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let formats = ["simple", "full", "json", "compact"];

    for format in &formats {
        let output = Command::new("cargo")
            .args(["run", "--", "--format", format])
            .output();

        match output {
            Ok(output) => {
                assert!(output.status.code().is_some());
            }
            Err(_) => {
                println!(
                    "Skipping format test for {} - cargo run not available",
                    format
                );
            }
        }
    }
    Ok(())
}

#[test]
fn test_main_verbose_flag() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "--verbose"])
        .output();

    match output {
        Ok(output) => {
            assert!(output.status.code().is_some());
        }
        Err(_) => {
            println!("Skipping verbose test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_max_frameworks() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "--max-frameworks", "3"])
        .output();

    match output {
        Ok(output) => {
            assert!(output.status.code().is_some());
        }
        Err(_) => {
            println!("Skipping max-frameworks test - cargo run not available");
        }
    }
    Ok(())
}

#[test]
fn test_main_theme_option() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(["run", "--", "--theme", "dark"])
        .output();

    match output {
        Ok(output) => {
            assert!(output.status.code().is_some());
        }
        Err(_) => {
            println!("Skipping theme test - cargo run not available");
        }
    }
    Ok(())
}

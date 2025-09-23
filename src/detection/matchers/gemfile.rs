use super::common::{calculate_dependency_confidence, sort_framework_matches};
use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Gemfile parser for Ruby projects
#[derive(Debug, Clone)]
pub struct Gemfile {
    /// Raw content of the Gemfile
    content: String,
}

impl Gemfile {
    /// Load Gemfile from a directory
    pub fn load_from_dir(dir: &Path) -> Result<Option<Self>> {
        let gemfile_path = dir.join("Gemfile");
        if !gemfile_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&gemfile_path)?;
        Ok(Some(Gemfile { content }))
    }

    /// Check if a gem exists in the Gemfile
    pub fn has_gem(&self, gem_name: &str) -> bool {
        // Parse gem declarations like:
        // gem 'rails', '~> 7.0'
        // gem "sinatra"
        // gem 'devise', '~> 4.8'

        for line in self.content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Look for gem declarations
            if line.starts_with("gem ") {
                // Extract gem name from quotes
                if let Some(name) = extract_gem_name(line) {
                    if name.eq_ignore_ascii_case(gem_name) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get all gems from the Gemfile
    pub fn get_all_gems(&self) -> Vec<String> {
        let mut gems = Vec::new();

        for line in self.content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Look for gem declarations
            if line.starts_with("gem ") {
                if let Some(name) = extract_gem_name(line) {
                    gems.push(name);
                }
            }
        }
        gems
    }
}

/// Extract gem name from a gem declaration line
fn extract_gem_name(line: &str) -> Option<String> {
    // Handle patterns like:
    // gem 'rails', '~> 7.0'
    // gem "sinatra"
    // gem 'devise', '~> 4.8', require: false

    let line = line.strip_prefix("gem ").unwrap_or(line).trim();

    // Find the first quoted string
    if let Some(start) = line.find(['\'', '"']) {
        let quote_char = line.chars().nth(start).unwrap();
        let after_quote = &line[start + 1..];

        if let Some(end) = after_quote.find(quote_char) {
            return Some(after_quote[..end].to_string());
        }
    }

    None
}

/// Gemfile-based framework matcher
pub struct GemfileMatcher;

impl GemfileMatcher {
    /// Detect frameworks in a directory using Gemfile
    pub fn detect_frameworks(
        path: &Path,
        frameworks: &[FrameworkDetector],
    ) -> Result<Vec<FrameworkMatch>> {
        let gemfile = match Gemfile::load_from_dir(path)? {
            Some(gemfile) => gemfile,
            None => return Ok(Vec::new()),
        };

        let mut matches = Vec::new();

        for framework in frameworks {
            if let DetectionType::GemSpec { gems } = &framework.detection {
                let found_gems: Vec<String> = gems
                    .iter()
                    .filter(|gem| gemfile.has_gem(gem))
                    .cloned()
                    .collect();

                if !found_gems.is_empty() {
                    let confidence = calculate_dependency_confidence(gems, &found_gems);
                    let evidence = vec!["Gemfile".to_string()];

                    matches.push(FrameworkMatch::new(framework.clone(), confidence, evidence));
                }
            }
        }

        // Sort by confidence (highest first), then by priority
        sort_framework_matches(&mut matches);

        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_gemfile(content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let gemfile_path = temp_dir.path().join("Gemfile");
        fs::write(gemfile_path, content).unwrap();
        temp_dir
    }

    fn create_test_framework(name: &str, gems: Vec<&str>) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection: DetectionType::GemSpec {
                gems: gems.into_iter().map(String::from).collect(),
            },
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
        }
    }

    #[test]
    fn test_gemfile_loading() {
        let content = r#"source 'https://rubygems.org'

gem 'rails', '~> 7.0'
gem 'sqlite3', '~> 1.4'
gem 'puma', '~> 5.0'
"#;

        let temp_dir = create_gemfile(content);
        let gemfile = Gemfile::load_from_dir(temp_dir.path()).unwrap().unwrap();

        assert!(gemfile.has_gem("rails"));
        assert!(gemfile.has_gem("sqlite3"));
        assert!(gemfile.has_gem("puma"));
        assert!(!gemfile.has_gem("sinatra"));

        let all_gems = gemfile.get_all_gems();
        assert!(all_gems.contains(&"rails".to_string()));
        assert!(all_gems.contains(&"sqlite3".to_string()));
        assert!(all_gems.contains(&"puma".to_string()));
    }

    #[test]
    fn test_gem_name_extraction() {
        assert_eq!(
            extract_gem_name("gem 'rails', '~> 7.0'"),
            Some("rails".to_string())
        );
        assert_eq!(
            extract_gem_name("gem \"sinatra\""),
            Some("sinatra".to_string())
        );
        assert_eq!(
            extract_gem_name("gem 'devise', '~> 4.8', require: false"),
            Some("devise".to_string())
        );
        assert_eq!(
            extract_gem_name("gem 'bootsnap', '>= 1.4.4', require: false"),
            Some("bootsnap".to_string())
        );
    }

    #[test]
    fn test_framework_detection() {
        let content = r#"source 'https://rubygems.org'

gem 'rails', '~> 7.0'
gem 'devise', '~> 4.8'
"#;

        let temp_dir = create_gemfile(content);
        let frameworks = vec![
            create_test_framework("Rails", vec!["rails"]),
            create_test_framework("Sinatra", vec!["sinatra"]),
        ];

        let matches = GemfileMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Rails");
        assert!(matches[0].confidence > 0.0);
        assert!(matches[0].evidence.contains(&"Gemfile".to_string()));
    }

    #[test]
    fn test_multiple_frameworks() {
        let content = r#"source 'https://rubygems.org'

gem 'rails', '~> 7.0'
gem 'sidekiq', '~> 6.0'
gem 'redis', '~> 4.0'
"#;

        let temp_dir = create_gemfile(content);
        let frameworks = vec![
            create_test_framework("Rails", vec!["rails"]),
            create_test_framework("Sidekiq", vec!["sidekiq", "redis"]),
            create_test_framework("Sinatra", vec!["sinatra"]),
        ];

        let matches = GemfileMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();

        assert_eq!(matches.len(), 2);
        let framework_names: Vec<&String> = matches.iter().map(|m| &m.framework.name).collect();
        assert!(framework_names.contains(&&"Rails".to_string()));
        assert!(framework_names.contains(&&"Sidekiq".to_string()));
        assert!(!framework_names.contains(&&"Sinatra".to_string()));
    }

    #[test]
    fn test_no_gemfile() {
        let temp_dir = TempDir::new().unwrap();
        let frameworks = vec![create_test_framework("Rails", vec!["rails"])];

        let matches = GemfileMatcher::detect_frameworks(temp_dir.path(), &frameworks).unwrap();
        assert!(matches.is_empty());
    }
}

use super::common::{calculate_dependency_confidence, sort_framework_matches};
use super::ecosystems::*;
use crate::detection::caches::ParsedFileCache;
use crate::types::{DetectionType, FrameworkDetector, FrameworkMatch};
use crate::Result;
use rayon::prelude::*;
use std::path::Path;

pub struct DependencyMatcher;

impl DependencyMatcher {
    pub fn detect_frameworks<P: AsRef<Path>>(
        path: P,
        frameworks: &[FrameworkDetector],
        parsed_cache: &ParsedFileCache,
    ) -> Result<Vec<FrameworkMatch>> {
        // Convert path to PathBuf for thread safety (Sync)
        let path_buf = path.as_ref().to_path_buf();

        // Parallel detection: Each framework can be checked concurrently
        // The ParsedFileCache is thread-safe (uses Mutex internally)
        let matches: Vec<FrameworkMatch> = frameworks
            .par_iter()
            .filter_map(|framework| {
                Self::try_detect_ecosystem_framework(&path_buf, framework, parsed_cache)
                    .ok()
                    .flatten()
            })
            .collect();

        let mut sorted_matches = matches;
        sort_framework_matches(&mut sorted_matches);
        Ok(sorted_matches)
    }

    fn try_detect_ecosystem_framework<P: AsRef<Path>>(
        path: P,
        framework: &FrameworkDetector,
        parsed_cache: &ParsedFileCache,
    ) -> Result<Option<FrameworkMatch>> {
        let (found_deps, evidence) = match &framework.detection {
            DetectionType::NodeEcosystem { dependencies } => {
                check_node_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::PythonEcosystem { dependencies } => {
                check_python_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::RustEcosystem { dependencies } => {
                check_rust_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::GoEcosystem { modules } => {
                check_go_ecosystem(&path, modules, parsed_cache)?
            }
            DetectionType::PHPEcosystem { packages } => {
                check_php_ecosystem(&path, packages, parsed_cache)?
            }
            DetectionType::RubyEcosystem { gems } => {
                check_ruby_ecosystem(&path, gems, parsed_cache)?
            }
            DetectionType::JavaEcosystem { dependencies } => {
                check_java_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::DotNetEcosystem { packages } => {
                check_dotnet_ecosystem(&path, packages, parsed_cache)?
            }
            DetectionType::ScalaEcosystem { dependencies } => {
                check_scala_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::DartEcosystem { dependencies } => {
                check_dart_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::LuaEcosystem { packages } => {
                check_lua_ecosystem(&path, packages, parsed_cache)?
            }
            DetectionType::KotlinEcosystem { dependencies } => {
                check_kotlin_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::SwiftEcosystem { dependencies } => {
                check_swift_ecosystem(&path, dependencies, parsed_cache)?
            }
            DetectionType::ElixirEcosystem { dependencies } => {
                check_elixir_ecosystem(&path, dependencies, parsed_cache)?
            }
            _ => return Ok(None),
        };

        if found_deps.is_empty() {
            return Ok(None);
        }

        let target_deps = Self::get_target_dependencies(&framework.detection);
        let confidence = calculate_dependency_confidence(&target_deps, &found_deps);
        Ok(Some(FrameworkMatch::new(
            framework.clone(),
            confidence,
            evidence,
        )))
    }

    fn get_target_dependencies(detection: &DetectionType) -> Vec<String> {
        match detection {
            DetectionType::NodeEcosystem { dependencies }
            | DetectionType::PythonEcosystem { dependencies }
            | DetectionType::RustEcosystem { dependencies }
            | DetectionType::JavaEcosystem { dependencies }
            | DetectionType::ScalaEcosystem { dependencies }
            | DetectionType::DartEcosystem { dependencies }
            | DetectionType::KotlinEcosystem { dependencies }
            | DetectionType::SwiftEcosystem { dependencies }
            | DetectionType::ElixirEcosystem { dependencies } => dependencies.clone(),
            DetectionType::GoEcosystem { modules } => modules.clone(),
            DetectionType::PHPEcosystem { packages }
            | DetectionType::DotNetEcosystem { packages }
            | DetectionType::LuaEcosystem { packages } => packages.clone(),
            DetectionType::RubyEcosystem { gems } => gems.clone(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionType;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_framework(name: &str, detection: DetectionType) -> FrameworkDetector {
        FrameworkDetector {
            name: name.to_string(),
            detection,
            icon: None,
            color: None,
            priority: 1,
            files: vec![],
            root_indicators: vec![],
        }
    }

    #[test]
    fn test_unified_cargo_toml_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let cargo_content = r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
tokio = "1.0"
serde = "1.0"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_content)?;

        let framework = create_test_framework(
            "Tokio",
            DetectionType::RustEcosystem {
                dependencies: vec!["tokio".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Tokio");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Cargo.toml"]);
        Ok(())
    }

    #[test]
    fn test_unified_package_json_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_content = r#"{
  "name": "test",
  "dependencies": {
    "react": "^18.0.0",
    "express": "^4.18.0"
  }
}"#;
        fs::write(temp_dir.path().join("package.json"), package_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["package.json"]);
        Ok(())
    }

    #[test]
    fn test_multiple_frameworks_single_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_content = r#"{
  "dependencies": {
    "react": "^18.0.0",
    "vue": "^3.0.0"
  }
}"#;
        fs::write(temp_dir.path().join("package.json"), package_content)?;

        let frameworks = vec![
            create_test_framework(
                "React",
                DetectionType::NodeEcosystem {
                    dependencies: vec!["react".to_string()],
                },
            ),
            create_test_framework(
                "Vue",
                DetectionType::NodeEcosystem {
                    dependencies: vec!["vue".to_string()],
                },
            ),
        ];

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &frameworks, &cache)?;

        assert_eq!(matches.len(), 2);
        let names: Vec<&str> = matches.iter().map(|m| m.framework.name.as_str()).collect();
        assert!(names.contains(&"React"));
        assert!(names.contains(&"Vue"));
        Ok(())
    }

    #[test]
    fn test_package_lock_json_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let package_lock_content = r#"{
  "name": "test-app",
  "version": "1.0.0",
  "packages": {
    "": {
      "name": "test-app",
      "version": "1.0.0"
    },
    "node_modules/react": {
      "version": "18.2.0",
      "resolved": "https://registry.npmjs.org/react/-/react-18.2.0.tgz"
    },
    "node_modules/react-dom": {
      "version": "18.2.0",
      "resolved": "https://registry.npmjs.org/react-dom/-/react-18.2.0.tgz"
    }
  }
}"#;
        fs::write(
            temp_dir.path().join("package-lock.json"),
            package_lock_content,
        )?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["package-lock.json"]);
        Ok(())
    }

    #[test]
    fn test_yarn_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let yarn_lock_content = r#"# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1

react@^18.2.0:
  version "18.2.0"
  resolved "https://registry.yarnpkg.com/react/-/react-18.2.0.tgz#555bd98592883255fa00de14f1151a917b5d77d5"
  integrity sha512-/3IjMdb2L9QbBdWiW5e3P2/npwMBaU9mHCSCUzNln0ZCYbcfTsGbTJrU/kGemdH2IWmB2ioZ+zkxtmq6g09fGQ==

"react-dom@^18.2.0":
  version "18.2.0"
  resolved "https://registry.yarnpkg.com/react-dom/-/react-dom-18.2.0.tgz#22aaf38708db2674ed9ada224ca4aa708d821e37"
  integrity sha512-6IMTriUmvsjHUjNtEDudZfuDQUoWXVxKHhlEGSk81n4YFS+r/Kl99wXiwlVXtPBtJenozv2P+hxDsw9eA7Xo6g==
"#;
        fs::write(temp_dir.path().join("yarn.lock"), yarn_lock_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["yarn.lock"]);
        Ok(())
    }

    #[test]
    fn test_pnpm_lock_yaml_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let pnpm_lock_content = r#"lockfileVersion: 5.4

specifiers:
  react: ^18.2.0
  react-dom: ^18.2.0

dependencies:
  react: 18.2.0
  react-dom: 18.2.0_react@18.2.0

packages:
  /react/18.2.0:
    resolution: {integrity: sha512-/3IjMdb2L9QbBdWiW5e3P2/npwMBaU9mHCSCUzNln0ZCYbcfTsGbTJrU/kGemdH2IWmB2ioZ+zkxtmq6g09fGQ==}
    engines: {node: '>=0.10.0'}
    dev: false
"#;
        fs::write(temp_dir.path().join("pnpm-lock.yaml"), pnpm_lock_content)?;

        let framework = create_test_framework(
            "React",
            DetectionType::NodeEcosystem {
                dependencies: vec!["react".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "React");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["pnpm-lock.yaml"]);
        Ok(())
    }

    #[test]
    fn test_composer_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let composer_lock_content = r#"{
    "_readme": [
        "This file locks the dependencies of your project to a known state",
        "Read more about it at https://getcomposer.org/doc/01-basic-usage.md#installing-dependencies"
    ],
    "content-hash": "5e6a10e1ec8e7e70e1d8f6a4e5a7e8f6f6a10e1e",
    "packages": [
        {
            "name": "laravel/framework",
            "version": "v10.15.0",
            "source": {
                "type": "git",
                "url": "https://github.com/laravel/framework.git",
                "reference": "4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b"
            },
            "dist": {
                "type": "zip",
                "url": "https://api.github.com/repos/laravel/framework/zipball/4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b",
                "reference": "4c91d5db1de7e8b56e23f6c85b2b1b3b3b3b3b3b",
                "shasum": ""
            },
            "require": {
                "php": "^8.1"
            },
            "type": "library"
        }
    ],
    "packages-dev": [],
    "aliases": [],
    "minimum-stability": "dev",
    "stability-flags": [],
    "prefer-stable": false,
    "prefer-lowest": false,
    "platform": {
        "php": "^8.1"
    },
    "platform-dev": [],
    "plugin-api-version": "2.3.0"
}"#;
        fs::write(temp_dir.path().join("composer.lock"), composer_lock_content)?;

        let framework = create_test_framework(
            "Laravel",
            DetectionType::PHPEcosystem {
                packages: vec!["laravel/framework".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Laravel");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["composer.lock"]);
        Ok(())
    }

    #[test]
    fn test_gemfile_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let gemfile_lock_content = r#"GEM
  remote: https://rubygems.org/
  specs:
    actioncable (7.0.5)
      actionpack (= 7.0.5)
      activesupport (= 7.0.5)
      nio4r (~> 2.0)
      websocket-driver (>= 0.6.1)
    actionmailbox (7.0.5)
      actionpack (= 7.0.5)
      activejob (= 7.0.5)
      activerecord (= 7.0.5)
      activestorage (= 7.0.5)
      activesupport (= 7.0.5)
      mail (>= 2.7.1)
    rails (7.0.5)
      actioncable (= 7.0.5)
      actionmailbox (= 7.0.5)
      actionmailer (= 7.0.5)
      actionpack (= 7.0.5)
      actiontext (= 7.0.5)
      actionview (= 7.0.5)
      activejob (= 7.0.5)
      activemodel (= 7.0.5)
      activerecord (= 7.0.5)
      activestorage (= 7.0.5)
      activesupport (= 7.0.5)
      bootsnap (>= 1.4.4)
      bundler (>= 1.15.0)
      railties (= 7.0.5)

PLATFORMS
  ruby

DEPENDENCIES
  rails (~> 7.0.0)

BUNDLED WITH
   2.4.13
"#;
        fs::write(temp_dir.path().join("Gemfile.lock"), gemfile_lock_content)?;

        let framework = create_test_framework(
            "Rails",
            DetectionType::RubyEcosystem {
                gems: vec!["rails".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Rails");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Gemfile.lock"]);
        Ok(())
    }

    #[test]
    fn test_poetry_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let poetry_lock_content = r#"# This file is automatically @generated by Poetry and should not be changed by hand.

[[package]]
name = "asgiref"
version = "3.7.2"
description = "ASGI specs, helper code, and adapters"
optional = false
python-versions = ">=3.7"
files = [
    {file = "asgiref-3.7.2-py3-none-any.whl", hash = "sha256:89b2ef2247e3b562a16eef663bc0e2e703ec6468e2fa8a5cd61cd449786d4f6e"},
    {file = "asgiref-3.7.2.tar.gz", hash = "sha256:9e0ce3aa93a819ba5b45120216b23878cf6e8525eb3848653452b4192b92afed"},
]

[[package]]
name = "django"
version = "4.2.3"
description = "A high-level Python Web framework that encourages rapid development and clean, pragmatic design."
optional = false
python-versions = ">=3.8"
files = [
    {file = "Django-4.2.3-py3-none-any.whl", hash = "sha256:f7c7852a5ac5a3da5a8d5b35cc6168f31b605971441798dac845f17ca8028039"},
    {file = "Django-4.2.3.tar.gz", hash = "sha256:45a747e1c5b3d6df1b141b1481e193b033fd1fdbda3ff52677dc81afdaacbaed"},
]

[package.dependencies]
asgiref = ">=3.6.0,<4"
sqlparse = ">=0.3.1"
tzdata = {version = "*", markers = "sys_platform == \"win32\""}

[metadata]
lock-version = "2.0"
python-versions = "^3.8"
content-hash = "f7c7852a5ac5a3da5a8d5b35cc6168f31b605971441798dac845f17ca8028039"
"#;
        fs::write(temp_dir.path().join("poetry.lock"), poetry_lock_content)?;

        let framework = create_test_framework(
            "Django",
            DetectionType::PythonEcosystem {
                dependencies: vec!["django".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Django");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["poetry.lock"]);
        Ok(())
    }

    #[test]
    fn test_cargo_lock_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let cargo_lock_content = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "axum"
version = "0.6.18"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "f8175979259124331c1d7bf6586ee7e0da434155e4b2d48ec2c8386281d8df39"
dependencies = [
 "async-trait",
 "axum-core",
 "bitflags 1.3.2",
 "bytes",
 "futures-util",
 "http",
 "http-body",
 "hyper",
 "itoa",
 "matchit",
 "memchr",
 "mime",
 "percent-encoding",
 "pin-project-lite",
 "rustversion",
 "serde",
 "serde_json",
 "serde_path_to_error",
 "serde_urlencoded",
 "sync_wrapper",
 "tokio",
 "tower",
 "tower-layer",
 "tower-service",
]

[[package]]
name = "test-app"
version = "0.1.0"
dependencies = [
 "axum",
 "tokio",
]
"#;
        fs::write(temp_dir.path().join("Cargo.lock"), cargo_lock_content)?;

        let framework = create_test_framework(
            "Axum",
            DetectionType::RustEcosystem {
                dependencies: vec!["axum".to_string()],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Axum");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["Cargo.lock"]);
        Ok(())
    }

    #[test]
    fn test_java_maven_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let pom_xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <groupId>com.example</groupId>
    <artifactId>spring-app</artifactId>
    <version>1.0.0</version>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter</artifactId>
            <version>2.7.0</version>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>2.7.0</version>
        </dependency>
    </dependencies>
</project>"#;

        fs::write(temp_dir.path().join("pom.xml"), pom_xml_content)?;

        let framework = create_test_framework(
            "Spring Boot",
            DetectionType::JavaEcosystem {
                dependencies: vec![
                    "spring-boot-starter".to_string(),
                    "spring-boot-starter-web".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Spring Boot");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["pom.xml"]);
        Ok(())
    }

    #[test]
    fn test_java_gradle_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let build_gradle_content = r#"
plugins {
    id 'org.springframework.boot' version '2.7.0'
    id 'io.spring.dependency-management' version '1.0.11.RELEASE'
    id 'java'
}

dependencies {
    implementation 'org.springframework.boot:spring-boot-starter'
    implementation 'org.springframework.boot:spring-boot-starter-web'
    testImplementation 'org.springframework.boot:spring-boot-starter-test'
}
"#;

        fs::write(temp_dir.path().join("build.gradle"), build_gradle_content)?;

        let framework = create_test_framework(
            "Spring Boot",
            DetectionType::JavaEcosystem {
                dependencies: vec![
                    "spring-boot-starter".to_string(),
                    "spring-boot-starter-web".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "Spring Boot");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["build.gradle"]);
        Ok(())
    }

    #[test]
    fn test_dotnet_csproj_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache = ParsedFileCache::new();

        let csproj_content = r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.AspNetCore.App" Version="2.1.1" />
    <PackageReference Include="Microsoft.EntityFrameworkCore" Version="6.0.0" />
    <PackageReference Include="Microsoft.EntityFrameworkCore.SqlServer" Version="6.0.0" />
  </ItemGroup>
</Project>"#;

        fs::write(temp_dir.path().join("web.csproj"), csproj_content)?;

        let framework = create_test_framework(
            "ASP.NET Core",
            DetectionType::DotNetEcosystem {
                packages: vec![
                    "Microsoft.AspNetCore".to_string(),
                    "Microsoft.EntityFrameworkCore".to_string(),
                ],
            },
        );

        let matches = DependencyMatcher::detect_frameworks(temp_dir.path(), &[framework], &cache)?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].framework.name, "ASP.NET Core");
        assert!(matches[0].confidence > 0.0);
        assert_eq!(matches[0].evidence, vec!["web.csproj"]);
        Ok(())
    }
}

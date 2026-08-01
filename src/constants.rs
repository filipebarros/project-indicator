// Package/manifest files
pub const PACKAGE_JSON: &str = "package.json";
pub const CARGO_TOML: &str = "Cargo.toml";
pub const PYPROJECT_TOML: &str = "pyproject.toml";
pub const COMPOSER_JSON: &str = "composer.json";
pub const GEMFILE: &str = "Gemfile";
pub const GO_MOD: &str = "go.mod";
pub const TSCONFIG_JSON: &str = "tsconfig.json";
pub const REQUIREMENTS_TXT: &str = "requirements.txt";
pub const BUILD_SBT: &str = "build.sbt";
pub const BUILD_SC: &str = "build.sc";
pub const PUBSPEC_YAML: &str = "pubspec.yaml";
pub const SETUP_PY: &str = "setup.py";
pub const POM_XML: &str = "pom.xml";
pub const BUILD_GRADLE: &str = "build.gradle";
pub const BUILD_GRADLE_KTS: &str = "build.gradle.kts";
pub const SETTINGS_GRADLE_KTS: &str = "settings.gradle.kts";
pub const PIPFILE: &str = "Pipfile";
pub const PACKAGES_CONFIG: &str = "packages.config";
pub const PACKAGE_SWIFT: &str = "Package.swift";
pub const MIX_EXS: &str = "mix.exs";

// VCS
pub const DOT_GIT: &str = ".git";

// Lock files
pub const PACKAGE_LOCK_JSON: &str = "package-lock.json";
pub const YARN_LOCK: &str = "yarn.lock";
pub const PNPM_LOCK_YAML: &str = "pnpm-lock.yaml";
pub const COMPOSER_LOCK: &str = "composer.lock";
pub const GEMFILE_LOCK: &str = "Gemfile.lock";
pub const POETRY_LOCK: &str = "poetry.lock";
pub const CARGO_LOCK: &str = "Cargo.lock";
pub const PUBSPEC_LOCK: &str = "pubspec.lock";
pub const LUAROCKS_LOCK: &str = "luarocks.lock";

// File extensions

// JavaScript/TypeScript
pub const JS_EXTENSION: &str = "*.js";
pub const MJS_EXTENSION: &str = "*.mjs";
pub const CJS_EXTENSION: &str = "*.cjs";
pub const TS_EXTENSION: &str = "*.ts";
pub const MTS_EXTENSION: &str = "*.mts";
pub const CTS_EXTENSION: &str = "*.cts";
pub const TSX_EXTENSION: &str = "*.tsx";

// Systems languages
pub const RS_EXTENSION: &str = "*.rs";
pub const C_EXTENSION: &str = "*.c";
pub const CPP_EXTENSION: &str = "*.cpp";
pub const CXX_EXTENSION: &str = "*.cxx";
pub const CC_EXTENSION: &str = "*.cc";
pub const C_HEADER_EXTENSION: &str = "*.h";
pub const CPP_HEADER_EXTENSION: &str = "*.hpp";
pub const HXX_HEADER_EXTENSION: &str = "*.hxx";
pub const GO_EXTENSION: &str = "*.go";
pub const ZIG_EXTENSION: &str = "*.zig";
pub const SWIFT_EXTENSION: &str = "*.swift";

// JVM languages
pub const JAVA_EXTENSION: &str = "*.java";
pub const KOTLIN_EXTENSION: &str = "*.kt";
pub const KOTLIN_SCRIPT_EXTENSION: &str = "*.kts";
pub const SCALA_EXTENSION: &str = "*.scala";

// Dynamic languages
pub const PY_EXTENSION: &str = "*.py";
pub const PYTHON_WINDOWS_EXTENSION: &str = "*.pyw";
pub const PYTHON_INTERFACE_EXTENSION: &str = "*.pyi";
pub const RUBY_EXTENSION: &str = "*.rb";
pub const PHP_EXTENSION: &str = "*.php";
pub const JULIA_EXTENSION: &str = "*.jl";

// Other
pub const DART_EXTENSION: &str = "*.dart";

// Framework config files
pub const NEXT_CONFIG_JS: &str = "next.config.js";
pub const NEXT_CONFIG_TS: &str = "next.config.ts";
pub const ANGULAR_JSON: &str = "angular.json";
pub const ROCKET_TOML: &str = "Rocket.toml";
pub const MANAGE_PY: &str = "manage.py";
pub const VITE_CONFIG_JS: &str = "vite.config.js";
pub const VITE_CONFIG_TS: &str = "vite.config.ts";
pub const NUXT_CONFIG_JS: &str = "nuxt.config.js";
pub const NUXT_CONFIG_TS: &str = "nuxt.config.ts";
pub const SVELTE_CONFIG_JS: &str = "svelte.config.js";

// Project/Build files
pub const GEMSPEC_EXTENSION: &str = "*.gemspec";
pub const XCWORKSPACE_EXTENSION: &str = "*.xcworkspace";
pub const VCXPROJ_EXTENSION: &str = "*.vcxproj";
pub const QT_PRO_EXTENSION: &str = "*.pro";
pub const QT_PRI_EXTENSION: &str = "*.pri";

pub const EARLY_TERMINATION: &str = "early_termination";
pub const EARLY_TERMINATION_MSG: &str = "Root indicator found - early termination";
pub const FRAMEWORK_DETECTION_SKIPPED: &str = "framework_detection_skipped";
pub const FRAMEWORK_DETECTION_SKIPPED_MSG: &str =
    "Framework detection skipped due to low confidence";
pub const FRAMEWORK_DETECTION: &str = "framework_detection";
pub const FRAMEWORK_DETECTION_PATTERN: &str = "FRAMEWORK_DETECTION";

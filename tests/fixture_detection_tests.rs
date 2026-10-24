//! Fixtures are copied to a temp directory first: detecting them in place
//! would let upward root-indicator traversal find this repo's own
//! `Cargo.toml`/`.git` and report everything as Rust.

use project_indicator::{config::TemplateGenerator, detection::DetectionEngine};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// (fixture dir, expected language, expected frameworks)
const EXPECTATIONS: &[(&str, &str, &[&str])] = &[
    ("bun", "Bun", &[]),
    ("bun-lock-bare", "Bun", &[]),
    ("bun-lockb-bare", "Bun", &[]),
    ("bun-react", "Bun", &["React"]),
    ("bun-react-notypescript", "Bun", &["React"]),
    ("bun-ts-noreact", "Bun", &[]),
    ("cpp", "C++", &[]),
    ("csharp-aspnet", "C#", &["ASP.NET Core"]),
    ("csharp-xamarin", "C#", &["Xamarin"]),
    // A bare Web SDK project (the modern template) has no PackageReference
    ("csharp-web-sdk-only", "C#", &["ASP.NET Core"]),
    ("csharp-global-json-only", "C#", &[]),
    ("dart-analysis-options-only", "Dart", &[]),
    ("dart-flutter", "Dart", &["Flutter"]),
    // pubspec.yaml declares deps, so the lock (transitive) is not consulted
    ("dart-lock-not-authoritative", "Dart", &[]),
    ("deno", "Deno", &[]),
    ("deno-fresh", "Deno", &["Fresh"]),
    // Fresh 2 declares jsr:@fresh/core instead of the $fresh/ alias
    ("deno-fresh2", "Deno", &["Fresh"]),
    ("deno-hono", "Deno", &["Hono"]),
    ("deno-json-and-jsonc", "Deno", &[]),
    ("deno-jsonc-bare", "Deno", &[]),
    ("deno-jsonc-comments", "Deno", &["Oak"]),
    ("deno-jsr-scope-only", "Deno", &[]),
    ("deno-oak", "Deno", &["Oak"]),
    ("deno-react", "Deno", &["React"]),
    ("deno-ts-react", "Deno", &["React"]),
    ("elixir", "Elixir", &["Phoenix"]),
    ("elixir-formatter-only", "Elixir", &[]),
    ("go-echo", "Go", &["Echo"]),
    ("go-fiber", "Go", &["Fiber"]),
    ("js-astro", "JavaScript", &["Astro"]),
    ("js-solid", "JavaScript", &["SolidJS"]),
    ("php-symfony", "PHP", &["Symfony"]),
    ("php-codeigniter", "PHP", &["CodeIgniter"]),
    ("php-yii", "PHP", &["Yii"]),
    ("python-flask", "Python", &["Flask"]),
    ("python-tornado", "Python", &["Tornado"]),
    ("python-pyramid", "Python", &["Pyramid"]),
    ("python-sanic", "Python", &["Sanic"]),
    ("rust-actix", "Rust", &["Actix Web"]),
    ("rust-rocket", "Rust", &["Rocket"]),
    ("ts-nestjs", "TypeScript", &["NestJS"]),
    ("gleam", "Gleam", &[]),
    ("gleam-with-package-json", "Gleam", &[]),
    ("go", "Go", &[]),
    ("go-work-only", "Go", &[]),
    ("go-gin", "Go", &["Gin"]),
    ("java", "Java", &[]),
    ("java-spring", "Java", &["Spring Boot"]),
    ("js-multi-lockfile", "JavaScript", &[]),
    ("js-plain-nodep", "JavaScript", &[]),
    ("js-runtime-config-only", "JavaScript", &[]),
    ("js-react", "JavaScript", &["React"]),
    // vite appears only transitively in the lockfile (vitest depends on it)
    ("js-transitive-lock", "JavaScript", &[]),
    // One stray .gleam file must not override a real JavaScript project
    ("js-stray-gleam", "JavaScript", &["React"]),
    ("js-svelte", "JavaScript", &["Svelte", "Vite"]),
    ("js-vue-plain", "JavaScript", &["Vue"]),
    ("jvm-gradle-groovy-kotlin-src", "Kotlin", &[]),
    ("jvm-gradle-kts-android", "Kotlin", &["Android"]),
    // Real Gradle layouts keep the manifest under src/main/, not at the root
    ("jvm-gradle-kts-android-app", "Kotlin", &["Android"]),
    ("jvm-gradle-kts-android-module", "Kotlin", &["Android"]),
    ("jvm-gradle-kts-bare", "Kotlin", &[]),
    ("jvm-gradle-kts-java-src", "Java", &[]),
    // Gradle is a build tool: the standard source dirs decide the language
    ("jvm-gradle-kts-java-dir", "Java", &[]),
    ("jvm-gradle-kts-kotlin-dir", "Kotlin", &[]),
    ("jvm-gradle-kts-scala-dir", "Scala", &[]),
    ("jvm-maven-scala-dir", "Scala", &[]),
    ("jvm-gradle-kts-kotlin-src", "Kotlin", &[]),
    ("jvm-gradle-mixed-src", "Java", &[]),
    ("jvm-pom-spring-bare", "Java", &["Spring Boot"]),
    ("jvm-scala-plain", "Scala", &[]),
    // settings.gradle alone is genuinely ambiguous (Gradle-generic, not
    // Java-specific — shared by Java/Kotlin/Scala Gradle projects), so no
    // confident guess is the honest behavior, not a bug.
    ("jvm-settings-gradle-bare", "<none>", &[]),
    ("kotlin", "Kotlin", &["Ktor"]),
    ("kotlin-spring", "Kotlin", &["Spring Boot"]),
    ("lua", "Lua", &["LÖVE"]),
    // Real rockspecs are named <name>-<version>.rockspec
    ("lua-rockspec-only", "Lua", &[]),
    ("nix", "Nix", &[]),
    ("odin", "Odin", &[]),
    // ols.json (the Odin language-server config) alone is deliberately enough
    ("odin-ols-only", "Odin", &[]),
    ("odin-sources-only", "Odin", &[]),
    ("php-laravel", "PHP", &["Laravel"]),
    ("php-phpunit-only", "PHP", &[]),
    // Makefile is generic build glue, not evidence of C++
    ("makefile-only", "<none>", &[]),
    ("makefile-with-lone-source", "<none>", &[]),
    // A Python backend with JS tooling (Tailwind etc.) stays Python, with or
    // without a JS lockfile beside it
    ("python-pyproject-with-js-tooling", "Python", &["Django"]),
    ("python-pyproject-with-pnpm-lock", "Python", &["Django"]),
    ("python-with-makefile", "Python", &["Flask"]),
    // PyPI names are case-insensitive: "Django==4.2" is Django
    ("python-django-capitalized", "Python", &["Django"]),
    ("python-django", "Python", &["Django"]),
    // A pyenv version pin alone, with zero other Python evidence, is a
    // genuinely weak signal — no confident guess is the honest behavior,
    // not a bug.
    ("python-version-only", "<none>", &[]),
    ("python-fastapi", "Python", &["FastAPI"]),
    ("ruby-rails", "Ruby", &["Rails"]),
    ("ruby-version-only", "Ruby", &[]),
    ("rust", "Rust", &[]),
    // A manifest that declares NO dependencies of its own (virtual workspace,
    // bare `gemspec` Gemfile, empty composer.json, tool-only pyproject) does
    // not shadow its lockfile
    ("rust-virtual-workspace", "Rust", &["Axum"]),
    // A gem's lone .gemspec is a Ruby marker
    ("ruby-gemspec-only", "Ruby", &[]),
    ("ruby-gemspec-gemfile", "Ruby", &["Rails"]),
    ("php-empty-composer", "PHP", &["Laravel"]),
    ("python-poetry-lock-only-deps", "Python", &["Django"]),
    // axum appears only transitively in Cargo.lock (tonic depends on it)
    ("rust-transitive-lock", "Rust", &[]),
    ("rust-workspace-deps", "Rust", &["Axum"]),
    // *.v is also a Verilog extension; a stray .v must not outrank Cargo.toml
    ("rust-stray-v", "Rust", &[]),
    ("rust-toolchain-only", "Rust", &[]),
    ("rust-axum", "Rust", &["Axum"]),
    ("scala-akka", "Scala", &["Akka HTTP"]),
    // akka-actor alone is Akka, not Akka HTTP
    ("scala-akka-actor-only", "Scala", &[]),
    ("scala-mill", "Scala", &["Akka HTTP"]),
    ("swift", "Swift", &["Vapor"]),
    ("swift-package-resolved-only", "Swift", &[]),
    // Tauri keeps its config in src-tauri/ under a JavaScript-rooted project
    ("tauri-js", "JavaScript", &["Tauri"]),
    ("terraform", "Terraform", &[]),
    ("ts-angular", "TypeScript", &["Angular"]),
    ("ts-nextjs", "TypeScript", &["React", "Next.js"]),
    ("ts-no-tsconfig", "TypeScript", &[]),
    ("ts-with-tsconfig", "TypeScript", &[]),
    ("v", "V", &[]),
];

fn copy_dir(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[test]
fn test_all_fixtures_detect_as_expected() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    let config = TemplateGenerator::generate_template(Some("full"))?;
    let engine = DetectionEngine::with_config(
        config.indicators.clone(),
        config.frameworks.clone(),
        config.detection.clone(),
    );

    let mut failures = Vec::new();

    for (name, expected_language, expected_frameworks) in EXPECTATIONS {
        let staged = TempDir::new()?;
        copy_dir(&fixtures_root.join(name), staged.path())?;

        let result = engine.detect(staged.path())?;

        let language = result
            .indicator
            .as_ref()
            .map(|l| l.name.as_str())
            .unwrap_or("<none>");
        if language != *expected_language {
            failures.push(format!(
                "{name}: expected language {expected_language}, got {language}"
            ));
        }

        let mut got: Vec<&str> = result
            .frameworks
            .iter()
            .map(|f| f.framework.name.as_str())
            .collect();
        got.sort_unstable();
        let mut want: Vec<&str> = expected_frameworks.to_vec();
        want.sort_unstable();
        if got != want {
            failures.push(format!("{name}: expected frameworks {want:?}, got {got:?}"));
        }
    }

    assert!(
        failures.is_empty(),
        "fixture expectations failed:\n  {}",
        failures.join("\n  ")
    );
    Ok(())
}

/// Every fixture directory must have a row in the expectations table, so new
/// fixtures can't be added without locking in their behavior.
#[test]
fn test_every_fixture_has_an_expectation() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    let mut missing = Vec::new();
    for entry in fs::read_dir(&fixtures_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !EXPECTATIONS.iter().any(|(n, _, _)| *n == name) {
                missing.push(name);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "fixtures without expectations: {missing:?}"
    );
    Ok(())
}

/// A framework no fixture expects is a framework nothing proves can be
/// detected: add a minimal fixture for it.
#[test]
fn test_every_catalog_framework_has_a_fixture() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashSet;

    let covered: HashSet<&str> = EXPECTATIONS
        .iter()
        .flat_map(|(_, _, frameworks)| frameworks.iter().copied())
        .collect();
    let config = project_indicator::config::TemplateGenerator::generate_template(Some("full"))?;
    let uncovered: Vec<&str> = config
        .frameworks
        .iter()
        .map(|f| f.name.as_str())
        .filter(|name| !covered.contains(name))
        .collect();

    assert!(
        uncovered.is_empty(),
        "frameworks without a fixture: {uncovered:?}"
    );
    Ok(())
}

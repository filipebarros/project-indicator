//! Fixtures are copied to a temp directory first: detecting them in place
//! would let upward root-indicator traversal find this repo's own
//! `Cargo.toml`/`.git` and report everything as Rust.

mod common;

use project_indicator::{detection::DetectionEngine, types::DetectionResult};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// (fixture dir, expected language, expected frameworks)
const EXPECTATIONS: &[(&str, &str, &[&str])] = &[
    ("bun", "JavaScript", &[]),
    ("bun-lock-bare", "JavaScript", &[]),
    ("bun-lockb-bare", "JavaScript", &[]),
    // plain-JS react deps + a bare tsconfig: the editor-tsconfig rule keeps
    // it JavaScript (a real TS project declares typescript, as ts-nextjs does)
    ("bun-react", "JavaScript", &["React"]),
    ("bun-react-notypescript", "JavaScript", &["React"]),
    ("bun-ts-noreact", "TypeScript", &[]),
    ("cpp", "C++", &[]),
    // C sources decide against the shared CMake build file, like the JVM rule
    ("c-sources", "C", &[]),
    ("cmake-c", "C", &[]),
    // header-only is genuinely ambiguous; C claims bare *.h
    ("c-header-only", "C", &[]),
    ("csharp-aspnet", "C#", &["ASP.NET Core"]),
    ("csharp-xamarin", "C#", &["Xamarin"]),
    // A bare Web SDK project (the modern template) has no PackageReference
    ("csharp-web-sdk-only", "C#", &["ASP.NET Core"]),
    ("dart-flutter", "Dart", &["Flutter"]),
    // the lockfile alone identifies Dart (marker, even with an empty scan)
    ("dart-pubspec-lock-only", "Dart", &[]),
    // pubspec.yaml declares deps, so the lock (transitive) is not consulted
    ("dart-lock-not-authoritative", "Dart", &[]),
    ("elixir", "Elixir", &["Phoenix"]),
    // .formatter.exs is itself Elixir source (*.exs), so source-file
    // evidence identifies the folder even though it is formatter config
    ("elixir-formatter-only", "Elixir", &[]),
    ("go-echo", "Go", &["Echo"]),
    ("go-fiber", "Go", &["Fiber"]),
    // a prose DESCRIPTION has no Package: field, so R never contests Go
    ("go-with-prose-description", "Go", &[]),
    ("js-astro", "JavaScript", &["Astro"]),
    ("js-electron", "JavaScript", &["Electron"]),
    ("js-express", "JavaScript", &["Express"]),
    ("js-hono", "JavaScript", &["Hono"]),
    // a React components folder is JavaScript even with .jsx only
    ("js-jsx-only", "JavaScript", &[]),
    // package.json declares no deps, so the lockfile's DIRECT deps count
    ("js-empty-manifest-lockfile", "JavaScript", &["React"]),
    ("js-nuxt", "JavaScript", &["Nuxt"]),
    // React Native outranks React for the prompt icon; both are listed
    ("js-react-native", "JavaScript", &["React", "React Native"]),
    ("js-solid", "JavaScript", &["SolidJS"]),
    // @types/* alone is editor hints in a JS project, not TypeScript
    ("js-types-editor-hints", "JavaScript", &[]),
    // ...and a tsconfig.json beside a plainly-JS package.json is editor
    // configuration (checkJs), not a TypeScript project
    ("js-editor-tsconfig", "JavaScript", &["React"]),
    // JS/TS tie on a lone shared lockfile: JavaScript is the deterministic
    // default flavour (config order decides)
    ("node-lockfile-only", "JavaScript", &[]),
    // Two manifests: certainty decides (Cargo.toml 0.86 over go.mod 0.81)
    ("polyglot-rust-go", "Rust", &[]),
    // Known tradeoff: tsconfig.json (0.86) outranks go.mod (0.81) on marker
    // certainty, and certainty ranks above sources, so root *.go files do
    // NOT flip it — only a Go framework dependency (e.g. gin) does
    ("go-with-tsconfig", "TypeScript", &[]),
    // A React Native app: the manifest that declares a detected framework
    // beats the Gemfile that only pins CocoaPods
    ("rn-with-gemfile", "JavaScript", &["React", "React Native"]),
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
    ("js-react", "JavaScript", &["React"]),
    // vite appears only transitively in the lockfile (vitest depends on it)
    ("js-transitive-lock", "JavaScript", &[]),
    // One stray .gleam file must not override a real JavaScript project
    ("js-stray-gleam", "JavaScript", &["React"]),
    ("js-svelte", "JavaScript", &["Svelte", "Vite"]),
    ("js-vue-plain", "JavaScript", &["Vue"]),
    // The shared JVM build file only breaks late ties: root Kotlin sources
    // beside a Groovy build.gradle make it a Kotlin project
    ("jvm-gradle-groovy-kotlin-src", "Kotlin", &[]),
    ("jvm-gradle-kts-android", "Kotlin", &["Android"]),
    // Real Gradle layouts keep the manifest under src/main/, not at the root
    ("jvm-gradle-kts-android-app", "Kotlin", &["Android"]),
    // Java shares the Gradle ecosystem, so a Groovy-built Android app
    // surfaces the Android framework too
    ("jvm-gradle-groovy-android", "Java", &["Android"]),
    ("jvm-gradle-kts-android-module", "Kotlin", &["Android"]),
    ("jvm-gradle-kts-bare", "Kotlin", &[]),
    // ...and root Java sources beside a Kotlin-DSL build.gradle.kts make it
    // a Java project: build files are build configuration, not sources
    ("jvm-gradle-kts-java-src", "Java", &[]),
    // Gradle is a build tool: the standard source dirs decide the language
    ("jvm-gradle-kts-java-dir", "Java", &[]),
    ("jvm-gradle-kts-kotlin-dir", "Kotlin", &[]),
    ("jvm-gradle-kts-scala-dir", "Scala", &[]),
    ("jvm-maven-scala-dir", "Scala", &[]),
    ("jvm-gradle-kts-kotlin-src", "Kotlin", &[]),
    ("jvm-gradle-mixed-src", "Java", &[]),
    // Decided: a settings file marks its DSL's language (low confidence);
    // settings.gradle.kts marks Kotlin the same way
    ("jvm-settings-gradle-bare", "Java", &[]),
    ("jvm-pom-spring-bare", "Java", &["Spring Boot"]),
    ("jvm-scala-plain", "Scala", &[]),
    ("kotlin", "Kotlin", &["Ktor"]),
    ("kotlin-spring", "Kotlin", &["Spring Boot"]),
    ("lua", "Lua", &["LÖVE"]),
    // Real rockspecs are named <name>-<version>.rockspec
    ("lua-rockspec-only", "Lua", &[]),
    ("odin", "Odin", &[]),
    // ols.json (the Odin language-server config) alone is deliberately enough
    ("odin-ols-only", "Odin", &[]),
    ("odin-sources-only", "Odin", &[]),
    ("php-laravel", "PHP", &["Laravel"]),
    // ...and it does not stop a lone source file from being its own language
    ("makefile-with-lone-source", "Python", &[]),
    // A Python backend with JS tooling (Tailwind etc.) stays Python, with or
    // without a JS lockfile beside it
    ("python-pyproject-with-js-tooling", "Python", &["Django"]),
    ("python-pyproject-with-pnpm-lock", "Python", &["Django"]),
    ("python-with-makefile", "Python", &["Flask"]),
    // PyPI names are case-insensitive: "Django==4.2" is Django
    ("python-django-capitalized", "Python", &["Django"]),
    ("python-django", "Python", &["Django"]),
    ("python-fastapi", "Python", &["FastAPI"]),
    // build tooling (vite, priority 5) is not framework evidence: the
    // Python backend keeps the folder
    ("python-with-vite", "Python", &[]),
    // a real R package: DESCRIPTION with a Package: field
    ("r-package", "R", &[]),
    ("ruby-hanami", "Ruby", &["Hanami"]),
    ("ruby-rails", "Ruby", &["Rails"]),
    ("ruby-sinatra", "Ruby", &["Sinatra"]),
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
    ("rust-axum", "Rust", &["Axum"]),
    ("scala-akka", "Scala", &["Akka HTTP"]),
    // akka-actor alone is Akka, not Akka HTTP
    ("scala-akka-actor-only", "Scala", &[]),
    ("scala-mill", "Scala", &["Akka HTTP"]),
    ("swift", "Swift", &["Vapor"]),
    ("swift-package-resolved-only", "Swift", &[]),
    // Tauri keeps its config in src-tauri/ under a JavaScript-rooted project
    ("tauri-js", "JavaScript", &["Tauri"]),
    ("ts-angular", "TypeScript", &["Angular"]),
    ("ts-nextjs", "TypeScript", &["React", "Next.js"]),
    ("ts-no-tsconfig", "TypeScript", &[]),
    ("ts-with-tsconfig", "TypeScript", &[]),
    ("v", "V", &[]),
];

/// Folders that are NOT a project: (fixture dir, why). A file that is not the
/// ecosystem's own manifest, lockfile or build file (a version pin, a tool
/// config, generic build glue) is no evidence of a language, and neither is a
/// build file shared by several languages with nothing to tell them apart.
const NOT_A_PROJECT: &[(&str, &str)] = &[
    ("csharp-global-json-only", "global.json is a .NET SDK pin"),
    (
        "dart-analysis-options-only",
        "analysis_options.yaml is linter config",
    ),
    (
        "js-runtime-config-only",
        ".npmrc and .nvmrc are tool config and a version pin",
    ),
    ("makefile-only", "Makefile is generic build glue"),
    ("php-phpunit-only", "phpunit.xml.dist is test-runner config"),
    ("python-version-only", "a pyenv pin is not a Python project"),
    (
        "pyproject-comment-only",
        "a pyproject with only tool config is not a Python project, even \
         when a comment mentions requires-python",
    ),
    ("rust-toolchain-only", "a rustup pin is not a Rust project"),
    ("ruby-version-only", "an rbenv pin is not a Ruby project"),
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

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn detect_fixture(
    engine: &DetectionEngine,
    fixtures_root: &Path,
    name: &str,
) -> Result<DetectionResult, Box<dyn std::error::Error>> {
    let staged = TempDir::new()?;
    copy_dir(&fixtures_root.join(name), staged.path())?;
    Ok(engine.detect(staged.path())?)
}

#[test]
fn test_all_fixtures_detect_as_expected() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = fixtures_root();
    let engine = common::built_in_engine();

    let mut failures = Vec::new();

    for (name, expected_language, expected_frameworks) in EXPECTATIONS {
        let result = detect_fixture(&engine, &fixtures_root, name)?;

        let language = result
            .indicator
            .as_ref()
            .map_or("none", |l| l.name.as_str());
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

#[test]
fn test_not_a_project_fixtures_detect_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = fixtures_root();
    let engine = common::built_in_engine();

    let mut failures = Vec::new();
    for (name, why) in NOT_A_PROJECT {
        let result = detect_fixture(&engine, &fixtures_root, name)?;
        if let Some(indicator) = &result.indicator {
            failures.push(format!("{name} ({why}): detected {}", indicator.name));
        }
        if !result.frameworks.is_empty() {
            failures.push(format!("{name} ({why}): detected frameworks"));
        }
    }

    assert!(
        failures.is_empty(),
        "folders that are not projects were detected:\n  {}",
        failures.join("\n  ")
    );
    Ok(())
}

/// Every fixture directory must have a row in the expectations table (or in
/// `NOT_A_PROJECT`), so new
/// fixtures can't be added without locking in their behavior.
#[test]
fn test_every_fixture_has_an_expectation() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = fixtures_root();

    let mut missing = Vec::new();
    for entry in fs::read_dir(&fixtures_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !EXPECTATIONS.iter().any(|(n, _, _)| *n == name)
                && !NOT_A_PROJECT.iter().any(|(n, _)| *n == name)
            {
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
fn test_every_catalog_framework_has_a_fixture() {
    use std::collections::HashSet;

    let covered: HashSet<&str> = EXPECTATIONS
        .iter()
        .flat_map(|(_, _, frameworks)| frameworks.iter().copied())
        .collect();
    let config = project_indicator::config::built_in_config();
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
}

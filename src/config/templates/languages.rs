//! Every built-in language indicator, one data row per language.

use super::shared::{
    nerd_icon, node_root_indicators, root_indicator as root,
    root_indicator_grouped as root_grouped, NODE_CONFIG_FILES,
};
use crate::constants::{PYPROJECT_TOML, TSCONFIG_JSON};
use crate::types::Ecosystem::{
    Cargo, Go, Gradle, Hex, Luarocks, Maven, Npm, Nuget, Packagist, Pub, Pypi, Rubygems, Sbt,
    Swiftpm,
};
use crate::types::IndicatorContext::{BuildSystem, LanguageRoot};
use crate::types::{Ecosystem, Indicator, RootIndicator};

fn indicator(
    name: &str,
    files: &[&str],
    color: &str,
    icon: &str,
    ecosystems: &[Ecosystem],
    roots: &[RootIndicator],
) -> Indicator {
    Indicator::with_root_indicators(
        name.to_string(),
        files.iter().map(|f| f.to_string()).collect(),
        color.to_string(),
        nerd_icon(icon),
        ecosystems.to_vec(),
        roots.to_vec(),
    )
}

/// JavaScript and TypeScript share the Node manifest/lockfile markers
/// (`node_root_indicators`) and package-manager config files
/// (`NODE_CONFIG_FILES`); only their source patterns and extra roots differ.
/// The marker patterns themselves reach the scan set through the engine's
/// root-pattern union.
fn node_indicator(
    name: &str,
    sources: &[&str],
    color: &str,
    icon: &str,
    extra_roots: &[RootIndicator],
) -> Indicator {
    let mut files: Vec<String> = sources.iter().map(|s| s.to_string()).collect();
    files.extend(NODE_CONFIG_FILES.iter().map(|f| f.to_string()));

    let mut roots = extra_roots.to_vec();
    roots.extend(node_root_indicators());

    Indicator::with_root_indicators(
        name.to_string(),
        files,
        color.to_string(),
        nerd_icon(icon),
        vec![Npm],
        roots,
    )
}

/// Every built-in language indicator. Row order is the arbitration
/// tie-breaker (config order — earlier wins), so insert deliberately.
/// `files` lists sources and tool configs only: filename-shaped root
/// patterns join the scan set via the engine's union, so they are not
/// repeated here.
#[rustfmt::skip]
pub(super) fn language_indicators() -> Vec<Indicator> {
    vec![
        indicator("Rust", &["*.rs", "rust-toolchain.toml", "rust-toolchain"],
            "#dea584", "e7a8", &[Cargo],
            &[root("Cargo.toml", 0.95, LanguageRoot), root("Cargo.lock", 0.8, LanguageRoot)]),
        node_indicator("JavaScript", &["*.js", "*.jsx", "*.mjs", "*.cjs"], "#f7df1e", "e781", &[]),
        node_indicator("TypeScript", &["*.ts", "*.tsx", "*.mts", "*.cts"], "#3178c6", "e8ca",
            &[root(TSCONFIG_JSON, 0.95, LanguageRoot)]),
        indicator("Scala", &["*.scala", "src/main/scala"],
            "#dc322f", "e737", &[Sbt], &[
            root("src/main/scala", 0.9, LanguageRoot),
            // Mill's build file: a build-system marker like build.sbt
            root("build.sc", 0.9, BuildSystem),
            root("build.sbt", 0.95, BuildSystem),
            root("project/build.properties", 0.8, BuildSystem),
        ]),
        indicator("Go", &["*.go"], "#00add8", "e724", &[Go],
            &[root("go.mod", 0.9, LanguageRoot), root("go.sum", 0.7, LanguageRoot),
              root("go.work", 0.75, LanguageRoot)]),
        // Listed below JavaScript: a stray *.gleam file must not outrank a
        // real package.json project
        indicator("Gleam", &["*.gleam"], "#FA1C7B", "f04ce", &[],
            &[root("gleam.toml", 0.95, LanguageRoot)]),
        // *.v is also Verilog/Coq's extension — an accepted collision, since
        // neither is in the catalog; v.mod is the decisive signal
        indicator("V", &["*.v", "*.vsh"], "#5D87BF", "e62b", &[],
            &[root("v.mod", 0.95, LanguageRoot)]),
        indicator("Julia", &["*.jl"], "#9558b2", "e80d", &[],
            &[root("Project.toml", 0.95, LanguageRoot), root("Manifest.toml", 0.8, LanguageRoot)]),
        indicator("Python",
            &["*.py", "*.pyw", "*.pyi", ".python-version"],
            "#3776ab", "e73c", &[Pypi], &[
            root(PYPROJECT_TOML, 0.95, LanguageRoot),
            root("requirements.txt", 0.9, LanguageRoot),
            root("setup.py", 0.85, LanguageRoot),
            root("Pipfile", 0.9, LanguageRoot),
            // poetry.lock vs uv.lock are alternatives: both lock the same
            // pyproject.toml via a different tool, never both at once.
            root_grouped("poetry.lock", 0.8, LanguageRoot, "python-pyproject-lockfile"),
            root_grouped("uv.lock", 0.65, LanguageRoot, "python-pyproject-lockfile"),
            // Pipfile.lock co-occurs with Pipfile (generated from it), not an
            // alternative to it.
            root("Pipfile.lock", 0.75, LanguageRoot),
        ]),
        indicator("PHP",
            &["*.php", "phpunit.xml", "phpunit.xml.dist"],
            "#777bb4", "e73d", &[Packagist],
            &[root("composer.json", 0.95, LanguageRoot), root("composer.lock", 0.8, LanguageRoot)]),
        indicator("Swift", &["*.swift", "*.xcworkspace", ".swift-version"],
            "#fa7343", "e755", &[Swiftpm],
            &[root("Package.swift", 0.95, LanguageRoot), root("Package.resolved", 0.6, LanguageRoot)]),
        indicator("Dart", &["*.dart", "analysis_options.yaml"],
            "#0175c2", "e798", &[Pub],
            &[root("pubspec.yaml", 0.95, LanguageRoot), root("pubspec.lock", 0.8, LanguageRoot)]),
        indicator("Elixir", &["*.ex", "*.exs", ".formatter.exs"],
            "#6e4a7e", "e7cd", &[Hex],
            &[root("mix.exs", 0.95, LanguageRoot), root("mix.lock", 0.8, LanguageRoot)]),
        // Odin has no canonical manifest; ols.json (the language-server
        // config) is the closest common root marker; an editor-config file
        // is weak evidence, hence the 0.5 weight
        indicator("Odin", &["*.odin"], "#3c73a8", "e6b0", &[],
            &[root("ols.json", 0.5, LanguageRoot)]),
        indicator("Zig", &["*.zig"], "#f7a41d", "e8ef", &[],
            &[root("build.zig", 0.95, BuildSystem), root("build.zig.zon", 0.8, BuildSystem)]),
        indicator("R", &["*.r", "*.R", "*.Rmd", "*.Rnw", "NAMESPACE"],
            "#276dc3", "e881", &[],
            &[root("DESCRIPTION", 0.95, LanguageRoot), root("renv.lock", 0.8, LanguageRoot)]),
        indicator("Lua",
            &["*.lua", "*.luac", "*.moon", "conf.lua"],
            "#000080", "e826", &[Luarocks],
            &[root("init.lua", 0.95, LanguageRoot), root("main.lua", 0.9, LanguageRoot),
              root("*.rockspec", 0.85, LanguageRoot), root("luarocks.lock", 0.8, LanguageRoot)]),
        // C has no manifest or lockfile, so it declares no root indicators:
        // source files alone identify it (the markerless path), and beside a
        // shared build file (CMakeLists.txt) the source-file arbitration rule
        // decides.
        indicator("C", &["*.c", "*.h"], "#a8b9cc", "e61e", &[], &[]),
        indicator("C++",
            &["*.cpp", "*.cxx", "*.cc", "*.hpp", "*.hxx", "Makefile", "*.vcxproj"],
            "#00599c", "e7a3", &[],
            &[root("CMakeLists.txt", 0.95, BuildSystem)]),
        indicator("Ruby",
            &["*.rb", ".ruby-version"],
            "#cc342d", "e739", &[Rubygems],
            &[root("Gemfile", 0.95, LanguageRoot), root("Gemfile.lock", 0.8, LanguageRoot),
              root("Rakefile", 0.85, BuildSystem), root("*.gemspec", 0.9, LanguageRoot)]),
        indicator("Kotlin",
            &["*.kt", "*.kts", "src/main/kotlin"],
            "#7f52ff", "e81b", &[Gradle, Maven], &[
            root("src/main/kotlin", 0.9, LanguageRoot),
            root("build.gradle.kts", 0.95, BuildSystem),
            // gradle init (Kotlin DSL) always writes it; as a marker it is
            // excluded from source evidence (build config, not sources)
            root("settings.gradle.kts", 0.8, BuildSystem),
        ]),
        indicator("C#",
            &["*.cs", "Program.cs", "global.json", "Directory.Build.props"],
            "#239120", "e7b2", &[Nuget],
            &[root("*.csproj", 0.95, LanguageRoot), root("*.sln", 0.9, LanguageRoot)]),
        indicator("Java",
            // build.gradle.kts is Kotlin's marker, deliberately in Java's
            // scan set (the union only derives an indicator's OWN markers)
            &["*.java", "build.gradle.kts",
              // Standard source root: Gradle/Maven are shared build tools, so
              // the language is decided by what the sources are written in
              "src/main/java"],
            "#ed8b00", "e738", &[Gradle, Maven], &[
            root("src/main/java", 0.9, LanguageRoot),
            // pom.xml vs build.gradle are alternative build systems: a real
            // Java project uses exactly one, so they contribute their max
            // weight rather than being summed.
            root_grouped("pom.xml", 0.95, BuildSystem, "jvm-build-system"),
            root_grouped("build.gradle", 0.95, BuildSystem, "jvm-build-system"),
            // Not grouped: settings.gradle commonly co-occurs with
            // build.gradle in multi-module projects, it isn't an alternative
            // to it.
            root("settings.gradle", 0.8, BuildSystem),
        ]),
    ]
}

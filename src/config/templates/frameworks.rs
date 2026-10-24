//! The framework catalog: every framework defined exactly once, keyed by the
//! ecosystems it belongs to. Indicators surface frameworks whose ecosystems
//! intersect their own.

use super::csharp::csharp_frameworks;
use super::dart::dart_frameworks;
use super::deno::deno_frameworks;
use super::elixir::elixir_frameworks;
use super::go::go_frameworks;
use super::java::java_frameworks;
use super::kotlin::kotlin_frameworks;
use super::lua::lua_frameworks;
use super::php::php_frameworks;
use super::python::python_frameworks;
use super::ruby::ruby_frameworks;
use super::rust::rust_frameworks;
use super::scala::scala_frameworks;
use super::shared::{dependency_framework, root_indicator, with_roots};
use super::swift::swift_frameworks;
use crate::types::{Ecosystem, Framework, IndicatorContext};

/// The complete catalog. Each framework appears exactly once — enforced by
/// `test_framework_catalog_has_no_duplicates`.
pub fn framework_catalog() -> Vec<Framework> {
    let mut catalog = node_frameworks();

    catalog.extend(rust_frameworks());
    catalog.extend(go_frameworks());
    catalog.extend(python_frameworks());
    catalog.extend(java_frameworks());
    catalog.extend(kotlin_frameworks());
    catalog.extend(csharp_frameworks());
    catalog.extend(php_frameworks());
    catalog.extend(ruby_frameworks());
    catalog.extend(swift_frameworks());
    catalog.extend(dart_frameworks());
    catalog.extend(deno_frameworks());
    catalog.extend(elixir_frameworks());
    catalog.extend(scala_frameworks());
    catalog.extend(lua_frameworks());

    catalog
}

/// Meta-frameworks (Next.js, Astro, NestJS) carry priority 1 and the UI
/// libraries they build on 2+: the prompt icon shows the more specific one.
fn node_frameworks() -> Vec<Framework> {
    let config = |file: &str| root_indicator(file, 0.9, IndicatorContext::FrameworkRoot);
    let npm = [Ecosystem::Npm];
    vec![
        dependency_framework("React", &npm, &["react"], Some("e7ba"), Some("#61dafb"), 2),
        dependency_framework("Vue", &npm, &["vue"], Some("e8dc"), Some("#4fc08d"), 2),
        with_roots(
            dependency_framework(
                "Angular",
                &npm,
                &["@angular/core"],
                Some("e753"),
                Some("#dd0031"),
                2,
            ),
            vec![config("angular.json")],
        ),
        with_roots(
            // Neutral gray: the brand black is invisible on dark terminals
            dependency_framework("Next.js", &npm, &["next"], Some("e83e"), Some("#8b949e"), 1),
            vec![config("next.config.js"), config("next.config.ts")],
        ),
        with_roots(
            dependency_framework("Astro", &npm, &["astro"], Some("e735"), Some("#ff5d01"), 1),
            vec![config("astro.config.mjs")],
        ),
        with_roots(
            dependency_framework(
                "NestJS",
                &npm,
                &["@nestjs/core"],
                Some("e83b"),
                Some("#e0234e"),
                1,
            ),
            vec![config("nest-cli.json")],
        ),
        with_roots(
            dependency_framework(
                "Svelte",
                &npm,
                &["svelte", "@sveltejs/kit"],
                None,
                Some("#ff3e00"),
                2,
            ),
            vec![config("svelte.config.js")],
        ),
        dependency_framework("SolidJS", &npm, &["solid-js"], None, Some("#2c4f7c"), 2),
        // Build tooling: app frameworks (React, Svelte, …) win the display
        dependency_framework("Vite", &npm, &["vite"], None, Some("#646cff"), 5),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_framework_catalog_has_no_duplicates() {
        let catalog = framework_catalog();
        let names: HashSet<&str> = catalog.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names.len(),
            catalog.len(),
            "framework catalog contains duplicate definitions"
        );
    }

    #[test]
    fn test_no_framework_is_invisible_on_a_dark_terminal() {
        for framework in framework_catalog() {
            assert_ne!(
                framework.color.as_deref().map(str::to_lowercase).as_deref(),
                Some("#000000"),
                "{} is pure black",
                framework.name
            );
        }
    }

    #[test]
    fn test_meta_frameworks_outrank_the_ui_libraries_they_build_on() {
        // best_framework picks the LOWEST priority number for the prompt
        // icon: the more specific framework must win over its base library
        let catalog = framework_catalog();
        let prio = |name: &str| -> u8 {
            match catalog.iter().find(|f| f.name == name) {
                Some(f) => f.priority,
                None => panic!("{name} missing from catalog"),
            }
        };
        for (meta, base) in [
            ("Next.js", "React"),
            ("Astro", "React"),
            ("NestJS", "Angular"),
        ] {
            assert!(
                prio(meta) < prio(base),
                "{meta} ({}) must outrank {base} ({})",
                prio(meta),
                prio(base)
            );
        }
    }
}

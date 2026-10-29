//! The framework catalog: every framework defined exactly once, keyed by the
//! ecosystems it belongs to. Indicators surface frameworks whose ecosystems
//! intersect their own.

use super::shared::{dependency_framework as dep, file_framework as file_fw};
use crate::types::Ecosystem::{
    Cargo, Go, Gradle, Hex, Luarocks, Maven, Npm, Nuget, Packagist, Pub, Pypi, Rubygems, Sbt,
    Swiftpm,
};
use crate::types::Framework;

/// The complete catalog. Each framework appears exactly once — enforced by
/// `test_framework_catalog_has_no_duplicates`.
#[rustfmt::skip]
pub(super) fn framework_catalog() -> Vec<Framework> {
    vec![
        // Meta-frameworks (Next.js, Astro, NestJS) carry priority 1 and the UI
        // libraries they build on 2+: the prompt icon shows the more specific one.
        dep("React", &[Npm], &["react"], Some("e7ba"), Some("#61dafb"), 2),
        dep("Vue", &[Npm], &["vue"], Some("e8dc"), Some("#4fc08d"), 2),
        dep("Angular", &[Npm], &["@angular/core"], Some("e753"), Some("#dd0031"), 2),
        // Neutral gray: the brand black is invisible on dark terminals
        dep("Next.js", &[Npm], &["next"], Some("e83e"), Some("#8b949e"), 1),
        dep("Nuxt", &[Npm], &["nuxt"], Some("f1106"), Some("#00dc82"), 1),
        dep("Astro", &[Npm], &["astro"], Some("e735"), Some("#ff5d01"), 1),
        dep("NestJS", &[Npm], &["@nestjs/core"], Some("e83b"), Some("#e0234e"), 1),
        dep("Svelte", &[Npm], &["svelte", "@sveltejs/kit"], Some("e8b7"), Some("#ff3e00"), 2),
        dep("SolidJS", &[Npm], &["solid-js"], Some("e8a7"), Some("#2c4f7c"), 2),
        dep("React Native", &[Npm], &["react-native"], Some("e93c"), Some("#61dafb"), 1),
        dep("Electron", &[Npm], &["electron"], Some("e7cb"), Some("#47848f"), 1),
        // Neutral gray: the brand black is invisible on dark terminals
        dep("Express", &[Npm], &["express"], Some("e7d3"), Some("#8b949e"), 2),
        // Build tooling: app frameworks (React, Svelte, …) win the display
        dep("Vite", &[Npm], &["vite"], Some("e8d7"), Some("#646cff"), 5),
        dep("Hono", &[Npm], &["hono"], None, Some("#E36002"), 2),
        dep("Axum", &[Cargo], &["axum"], None, Some("#7c3aed"), 1),
        dep("Actix Web", &[Cargo], &["actix-web"], None, Some("#8b949e"), 1),
        dep("Rocket", &[Cargo], &["rocket"], Some("f135"), Some("#d33847"), 1),
        // Every Tauri app has a tauri.conf.json — at the root or in
        // src-tauri/ — which is more reliable than the root Cargo.toml
        // (the dependency lives in src-tauri/ for JS-rooted projects).
        // Npm too, so JavaScript-rooted projects surface it.
        file_fw("Tauri", &[Cargo, Npm], &["tauri.conf.json", "src-tauri/tauri.conf.json"],
            Some("e8bb"), Some("#24c8db"), 2),
        dep("Gin", &[Go], &["github.com/gin-gonic/gin"], None, Some("#008ecf"), 1),
        // Major-version module paths (…/vN) are listed explicitly: go.mod matching
        // is line-anchored, so a bare prefix won't match them
        dep("Echo", &[Go], &["github.com/labstack/echo/v4", "github.com/labstack/echo/v5"],
            None, Some("#00abda"), 1),
        dep("Fiber", &[Go], &["github.com/gofiber/fiber/v2", "github.com/gofiber/fiber/v3"],
            Some("e90d"), Some("#28b6f6"), 1),
        dep("Django", &[Pypi], &["django"], Some("e71d"), Some("#092e20"), 1),
        dep("Flask", &[Pypi], &["flask"], Some("e7dc"), Some("#8b949e"), 2),
        dep("FastAPI", &[Pypi], &["fastapi"], Some("e7d5"), Some("#009688"), 3),
        dep("Tornado", &[Pypi], &["tornado"], Some("ef2d"), Some("#3bb9ff"), 4),
        dep("Pyramid", &[Pypi], &["pyramid"], Some("f1952"), Some("#ff0000"), 5),
        dep("Sanic", &[Pypi], &["sanic"], None, Some("#ff8c00"), 6),
        dep("Spring Boot", &[Maven, Gradle],
            &["spring-boot-starter", "spring-boot-starter-web", "spring-boot-starter-data-jpa",
              "spring-boot-starter-test"],
            Some("e8ac"), Some("#6db33f"), 1),
        dep("Ktor", &[Gradle], &["ktor-server-core", "ktor-server-netty", "ktor-server-cio"],
            Some("e81c"), Some("#087cfa"), 1),
        // Gradle puts the manifest under src/main/ of the app module
        file_fw("Android", &[Gradle],
            &["AndroidManifest.xml", "src/main/AndroidManifest.xml",
              "app/src/main/AndroidManifest.xml"],
            Some("e70e"), Some("#3ddc84"), 3),
        dep("ASP.NET Core", &[Nuget],
            &["Microsoft.AspNetCore", "Microsoft.AspNetCore.App", "Microsoft.AspNetCore.Mvc"],
            Some("e77f"), Some("#512bd4"), 1),
        dep("Xamarin", &[Nuget], &["Xamarin.Forms", "Xamarin.Essentials"],
            Some("e8e7"), Some("#3498db"), 3),
        dep("Laravel", &[Packagist], &["laravel/framework"], Some("e73f"), Some("#ff2d20"), 1),
        dep("Symfony", &[Packagist], &["symfony/framework-bundle"], Some("e757"), Some("#8b949e"), 2),
        dep("CodeIgniter", &[Packagist], &["codeigniter4/framework"], Some("e780"), Some("#ee4623"), 3),
        dep("Yii", &[Packagist], &["yiisoft/yii2"], Some("e782"), Some("#0073bb"), 5),
        dep("Hanami", &[Rubygems], &["hanami"], None, Some("#f0446c"), 1),
        // Neutral gray: the brand black is invisible on dark terminals
        dep("Sinatra", &[Rubygems], &["sinatra"], None, Some("#8b949e"), 2),
        dep("Rails", &[Rubygems], &["rails"], Some("e73b"), Some("#cc0000"), 1),
        dep("Vapor", &[Swiftpm], &["vapor"], None, Some("#8d59f2"), 1),
        dep("Flutter", &[Pub], &["flutter", "flutter_test", "cupertino_icons"],
            Some("e7dd"), Some("#02569b"), 1),
        dep("Phoenix", &[Hex], &["phoenix", "phoenix_html", "phoenix_live_view"],
            Some("e860"), Some("#ff6600"), 1),
        // akka-actor / akka-stream alone are Akka, not Akka HTTP
        dep("Akka HTTP", &[Sbt], &["akka-http"], Some("e708"), Some("#0b5394"), 1),
        file_fw("LÖVE", &[Luarocks], &["conf.lua"], None, Some("#e74a99"), 1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::templates::shared::nerd_icon;
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

    /// `--format full` skips a framework without an icon, so these (which
    /// have a Nerd Fonts glyph) would be invisible in a prompt.
    #[test]
    fn test_frameworks_with_a_nerd_font_glyph_have_an_icon() {
        for (name, glyph) in [
            ("Svelte", "e8b7"),
            ("SolidJS", "e8a7"),
            ("Vite", "e8d7"),
            ("Tauri", "e8bb"),
        ] {
            let icon = framework_catalog()
                .into_iter()
                .find(|f| f.name == name)
                .and_then(|f| f.icon);
            assert_eq!(icon, Some(nerd_icon(glyph)), "{name}");
        }
    }

    #[test]
    fn test_meta_frameworks_outrank_the_ui_libraries_they_build_on() {
        // best_framework_of picks the LOWEST priority number for the prompt
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
            ("Nuxt", "Vue"),
            ("React Native", "React"),
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

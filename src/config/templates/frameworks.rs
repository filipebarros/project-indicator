//! The framework catalog: every framework defined exactly once, keyed by the
//! ecosystems it belongs to. Indicators surface frameworks whose ecosystems
//! intersect their own.

use super::csharp::csharp_frameworks;
use super::dart::dart_frameworks;
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
use super::shared::{
    create_angular_framework, create_astro_framework, create_nestjs_framework,
    create_nextjs_framework, create_react_framework, create_solid_framework,
    create_svelte_framework, create_vite_framework, create_vue_framework,
};
use super::swift::swift_frameworks;
use crate::types::Framework;

/// The complete catalog. Each framework appears exactly once — enforced by
/// `test_framework_catalog_has_no_duplicates`.
pub fn framework_catalog() -> Vec<Framework> {
    let mut catalog = vec![
        create_react_framework(),
        create_vue_framework(),
        create_angular_framework(),
        create_nextjs_framework(),
        create_astro_framework(),
        create_nestjs_framework(),
        create_svelte_framework(),
        create_solid_framework(),
        create_vite_framework(),
    ];

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
    catalog.extend(elixir_frameworks());
    catalog.extend(scala_frameworks());
    catalog.extend(lua_frameworks());

    catalog
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
    fn test_every_framework_declares_scoping() {
        // Every catalog entry needs at least one ecosystem, otherwise no
        // indicator can ever surface it
        for framework in framework_catalog() {
            assert!(
                !framework.ecosystems.is_empty(),
                "{} has no ecosystems",
                framework.name
            );
        }
    }
}

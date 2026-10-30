use crate::types::{
    DetectionEvidence, DetectionResult, Framework, FrameworkMatch, Indicator, NestedProject,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize)]
pub(super) struct CachedDetectionResultRef<'a> {
    indicator: Option<usize>,
    frameworks: Vec<CachedFrameworkMatchRef<'a>>,
    confidence: f32,
    evidence: &'a DetectionEvidence,
    nested: Vec<CachedNestedProjectRef<'a>>,
    indicators: Vec<&'a Indicator>,
    framework_definitions: Vec<&'a Framework>,
}

#[derive(Serialize)]
struct CachedNestedProjectRef<'a> {
    name: &'a str,
    indicator: usize,
    frameworks: Vec<CachedFrameworkMatchRef<'a>>,
}

#[derive(Serialize)]
struct CachedFrameworkMatchRef<'a> {
    framework: usize,
    confidence: f32,
    evidence: &'a [String],
}

impl<'a> From<&'a DetectionResult> for CachedDetectionResultRef<'a> {
    fn from(result: &'a DetectionResult) -> Self {
        let mut cached = Self {
            indicator: None,
            frameworks: Vec::new(),
            confidence: result.confidence,
            evidence: &result.evidence,
            nested: Vec::new(),
            indicators: Vec::new(),
            framework_definitions: Vec::new(),
        };
        cached.indicator = result
            .indicator
            .as_ref()
            .map(|indicator| cached.indicator_index(indicator));
        cached.frameworks = cached.framework_matches(&result.frameworks);
        cached.nested = result
            .nested
            .iter()
            .map(|project| {
                let indicator = cached.indicator_index(&project.indicator);
                let frameworks = cached.framework_matches(&project.frameworks);
                CachedNestedProjectRef {
                    name: &project.name,
                    indicator,
                    frameworks,
                }
            })
            .collect();
        cached
    }
}

impl<'a> CachedDetectionResultRef<'a> {
    fn indicator_index(&mut self, indicator: &'a Arc<Indicator>) -> usize {
        if let Some(index) = self
            .indicators
            .iter()
            .position(|candidate| std::ptr::eq(*candidate, indicator.as_ref()))
        {
            return index;
        }
        self.indicators.push(indicator.as_ref());
        self.indicators.len() - 1
    }

    fn framework_index(&mut self, framework: &'a Framework) -> usize {
        if let Some(index) = self
            .framework_definitions
            .iter()
            .position(|candidate| *candidate == framework)
        {
            return index;
        }
        self.framework_definitions.push(framework);
        self.framework_definitions.len() - 1
    }

    fn framework_matches(
        &mut self,
        matches: &'a [FrameworkMatch],
    ) -> Vec<CachedFrameworkMatchRef<'a>> {
        matches
            .iter()
            .map(|matched| {
                let framework = self.framework_index(&matched.framework);
                CachedFrameworkMatchRef {
                    framework,
                    confidence: matched.confidence,
                    evidence: &matched.evidence,
                }
            })
            .collect()
    }
}

#[derive(Deserialize)]
pub(super) struct CachedDetectionResult {
    indicator: Option<usize>,
    frameworks: Vec<CachedFrameworkMatch>,
    confidence: f32,
    pub(super) evidence: DetectionEvidence,
    nested: Vec<CachedNestedProject>,
    indicators: Vec<Indicator>,
    framework_definitions: Vec<Framework>,
}

#[derive(Deserialize)]
struct CachedNestedProject {
    name: String,
    indicator: usize,
    frameworks: Vec<CachedFrameworkMatch>,
}

#[derive(Deserialize)]
struct CachedFrameworkMatch {
    framework: usize,
    confidence: f32,
    evidence: Vec<String>,
}

impl CachedDetectionResult {
    pub(super) fn into_result(self) -> Option<DetectionResult> {
        let Self {
            indicator,
            frameworks,
            confidence,
            evidence,
            nested,
            indicators,
            framework_definitions,
        } = self;
        let indicators: Vec<Arc<Indicator>> = indicators.into_iter().map(Arc::new).collect();
        let indicator = match indicator {
            Some(index) => Some(indicators.get(index).cloned()?),
            None => None,
        };
        let frameworks = framework_matches(frameworks, &framework_definitions)?;
        let nested = nested
            .into_iter()
            .map(|project| {
                let indicator = indicators.get(project.indicator).cloned()?;
                let frameworks = framework_matches(project.frameworks, &framework_definitions)?;
                Some(NestedProject {
                    name: project.name,
                    indicator,
                    frameworks,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let mut result =
            DetectionResult::new_with_evidence(indicator, frameworks, confidence, evidence);
        result.nested = nested;
        Some(result)
    }
}

fn framework_matches(
    matches: Vec<CachedFrameworkMatch>,
    definitions: &[Framework],
) -> Option<Vec<FrameworkMatch>> {
    matches
        .into_iter()
        .map(|matched| {
            let framework = definitions.get(matched.framework)?.clone();
            Some(FrameworkMatch::new(
                framework,
                matched.confidence,
                matched.evidence,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionType;

    fn indicator(color: &str) -> Arc<Indicator> {
        Arc::new(Indicator::new(
            "Rust".to_string(),
            vec!["*.rs".to_string()],
            color.to_string(),
            "R".to_string(),
            Vec::new(),
        ))
    }

    fn framework(priority: u8) -> Framework {
        Framework {
            name: "Shared name".to_string(),
            ecosystems: Vec::new(),
            detection: DetectionType::FileExists(vec!["Cargo.toml".to_string()]),
            icon: None,
            color: None,
            priority,
        }
    }

    fn project(name: &str, indicator: Arc<Indicator>, framework: Framework) -> NestedProject {
        NestedProject {
            name: name.to_string(),
            indicator,
            frameworks: vec![FrameworkMatch::new(
                framework,
                1.0,
                vec!["Cargo.toml".to_string()],
            )],
        }
    }

    fn sample_result() -> DetectionResult {
        let shared = indicator("#aaa");
        let mut result = DetectionResult::new(
            Some(shared.clone()),
            vec![FrameworkMatch::new(framework(1), 1.0, Vec::new())],
            1.0,
        );
        result.nested = vec![
            project("one", shared.clone(), framework(1)),
            project("two", shared, framework(1)),
            project("distinct", indicator("#bbb"), framework(2)),
        ];
        result
    }

    #[test]
    fn compact_result_round_trip_preserves_values_and_sharing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = sample_result();
        let json = serde_json::to_string(&CachedDetectionResultRef::from(&result))?;
        let cached: CachedDetectionResult = serde_json::from_str(&json)?;
        let decoded = cached.into_result().ok_or("invalid compact result")?;

        assert_eq!(decoded, result);
        assert!(Arc::ptr_eq(
            decoded.indicator.as_ref().ok_or("root indicator")?,
            &decoded.nested[0].indicator
        ));
        assert!(Arc::ptr_eq(
            &decoded.nested[0].indicator,
            &decoded.nested[1].indicator
        ));
        assert!(!Arc::ptr_eq(
            &decoded.nested[0].indicator,
            &decoded.nested[2].indicator
        ));
        Ok(())
    }

    #[test]
    fn compact_result_rejects_invalid_catalog_indexes() -> Result<(), Box<dyn std::error::Error>> {
        let result = sample_result();
        for pointer in [
            "/indicator",
            "/frameworks/0/framework",
            "/nested/0/indicator",
            "/nested/0/frameworks/0/framework",
        ] {
            let mut json = serde_json::to_value(CachedDetectionResultRef::from(&result))?;
            *json
                .pointer_mut(pointer)
                .ok_or("missing catalog reference")? = 999.into();
            let cached: CachedDetectionResult = serde_json::from_value(json)?;
            assert!(cached.into_result().is_none(), "{pointer}");
        }
        Ok(())
    }
}

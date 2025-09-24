use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_python_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
        "Python".to_string(),
        vec![
            "*.py".to_string(),
            "*.pyw".to_string(),
            "*.pyi".to_string(),
            "requirements.txt".to_string(),
            "pyproject.toml".to_string(),
            "setup.py".to_string(),
            "Pipfile".to_string(),
            "poetry.lock".to_string(),
        ],
        "#3776ab".to_string(),
        nerd_icon("e73c"),
        8,
        vec![
            framework(
                "Django",
                DetectionType::PythonEcosystem {
                    dependencies: vec!["django".to_string()],
                },
                Some(nerd_icon("e71d")),
                Some("#092e20"),
                1,
                vec![root_indicator(
                    "manage.py",
                    0.9,
                    IndicatorContext::FrameworkRoot,
                )],
            ),
            simple_framework(
                "Flask",
                DetectionType::PythonEcosystem {
                    dependencies: vec!["flask".to_string()],
                },
                Some(nerd_icon("e7dc")),
                Some("#000000"),
                2,
            ),
            simple_framework(
                "FastAPI",
                DetectionType::PythonEcosystem {
                    dependencies: vec!["fastapi".to_string()],
                },
                Some(nerd_icon("e7d5")),
                Some("#009688"),
                3,
            ),
        ],
        vec![
            root_indicator("pyproject.toml", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("requirements.txt", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("setup.py", 0.85, IndicatorContext::LanguageRoot),
            root_indicator("Pipfile", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("poetry.lock", 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

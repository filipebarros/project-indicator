use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::constants::{
    PIPFILE, POETRY_LOCK, PYPROJECT_TOML, PYTHON_INTERFACE_EXTENSION, PYTHON_WINDOWS_EXTENSION,
    PY_EXTENSION, REQUIREMENTS_TXT, SETUP_PY,
};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_python_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "Python".to_string(),
        vec![
            PY_EXTENSION.to_string(),
            PYTHON_WINDOWS_EXTENSION.to_string(),
            PYTHON_INTERFACE_EXTENSION.to_string(),
            REQUIREMENTS_TXT.to_string(),
            PYPROJECT_TOML.to_string(),
            SETUP_PY.to_string(),
            PIPFILE.to_string(),
            POETRY_LOCK.to_string(),
        ],
        "#3776ab".to_string(),
        nerd_icon("e73c"),
        8,
        vec![Ecosystem::Pypi],
        vec![
            root_indicator(PYPROJECT_TOML, 0.95, IndicatorContext::LanguageRoot),
            root_indicator(REQUIREMENTS_TXT, 0.9, IndicatorContext::LanguageRoot),
            root_indicator(SETUP_PY, 0.85, IndicatorContext::LanguageRoot),
            root_indicator(PIPFILE, 0.9, IndicatorContext::LanguageRoot),
            root_indicator(POETRY_LOCK, 0.8, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn python_frameworks() -> Vec<Framework> {
    vec![
        framework(
            "Django",
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
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
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
                dependencies: vec!["flask".to_string()],
            },
            Some(nerd_icon("e7dc")),
            Some("#000000"),
            2,
        ),
        simple_framework(
            "FastAPI",
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
                dependencies: vec!["fastapi".to_string()],
            },
            Some(nerd_icon("e7d5")),
            Some("#009688"),
            3,
        ),
        simple_framework(
            "Tornado",
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
                dependencies: vec!["tornado".to_string()],
            },
            None,
            Some("#3bb9ff"),
            4,
        ),
        simple_framework(
            "Pyramid",
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
                dependencies: vec!["pyramid".to_string()],
            },
            None,
            Some("#ff0000"),
            5,
        ),
        simple_framework(
            "Sanic",
            vec![Ecosystem::Pypi],
            DetectionType::Dependencies {
                dependencies: vec!["sanic".to_string()],
            },
            None,
            Some("#ff8c00"),
            6,
        ),
    ]
}

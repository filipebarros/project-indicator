use super::shared::{
    dependency_framework, nerd_icon, root_indicator, root_indicator_grouped, with_roots,
};
use crate::constants::{
    PIPFILE, POETRY_LOCK, PYPROJECT_TOML, PYTHON_INTERFACE_EXTENSION, PYTHON_WINDOWS_EXTENSION,
    PY_EXTENSION, REQUIREMENTS_TXT, SETUP_PY,
};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

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
            "Pipfile.lock".to_string(),
            "uv.lock".to_string(),
            ".python-version".to_string(),
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
            // poetry.lock vs uv.lock are alternatives: both lock the same
            // pyproject.toml via a different tool, never both at once.
            root_indicator_grouped(
                POETRY_LOCK,
                0.8,
                IndicatorContext::LanguageRoot,
                "python-pyproject-lockfile",
            ),
            root_indicator_grouped(
                "uv.lock",
                0.65,
                IndicatorContext::LanguageRoot,
                "python-pyproject-lockfile",
            ),
            // Pipfile.lock co-occurs with Pipfile (generated from it), not an
            // alternative to it.
            root_indicator("Pipfile.lock", 0.75, IndicatorContext::LanguageRoot),
            // A pyenv version pin — weak alone, but real.
            root_indicator(".python-version", 0.5, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn python_frameworks() -> Vec<Framework> {
    vec![
        with_roots(
            dependency_framework(
                "Django",
                &[Ecosystem::Pypi],
                &["django"],
                Some("e71d"),
                Some("#092e20"),
                1,
            ),
            vec![root_indicator(
                "manage.py",
                0.9,
                IndicatorContext::FrameworkRoot,
            )],
        ),
        dependency_framework(
            "Flask",
            &[Ecosystem::Pypi],
            &["flask"],
            Some("e7dc"),
            Some("#8b949e"),
            2,
        ),
        dependency_framework(
            "FastAPI",
            &[Ecosystem::Pypi],
            &["fastapi"],
            Some("e7d5"),
            Some("#009688"),
            3,
        ),
        dependency_framework(
            "Tornado",
            &[Ecosystem::Pypi],
            &["tornado"],
            None,
            Some("#3bb9ff"),
            4,
        ),
        dependency_framework(
            "Pyramid",
            &[Ecosystem::Pypi],
            &["pyramid"],
            None,
            Some("#ff0000"),
            5,
        ),
        dependency_framework(
            "Sanic",
            &[Ecosystem::Pypi],
            &["sanic"],
            None,
            Some("#ff8c00"),
            6,
        ),
    ]
}

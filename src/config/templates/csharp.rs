use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_csharp_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "C#".to_string(),
        vec![
            "*.cs".to_string(),
            "*.csproj".to_string(),
            "*.sln".to_string(),
            "Program.cs".to_string(),
        ],
        "#239120".to_string(),
        nerd_icon("e7b2"),
        10,
        vec![Ecosystem::Nuget],
        vec![
            root_indicator("*.csproj", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("*.sln", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("Program.cs", 0.85, IndicatorContext::LanguageRoot),
        ],
    )
}

pub fn csharp_frameworks() -> Vec<Framework> {
    vec![
        simple_framework(
            "ASP.NET Core",
            vec![Ecosystem::Nuget],
            DetectionType::Dependencies {
                dependencies: vec![
                    "Microsoft.AspNetCore".to_string(),
                    "Microsoft.AspNetCore.App".to_string(),
                    "Microsoft.AspNetCore.Mvc".to_string(),
                ],
            },
            Some(nerd_icon("e77f")),
            Some("#512bd4"),
            1,
        ),
        framework(
            "Xamarin",
            vec![Ecosystem::Nuget],
            DetectionType::Dependencies {
                dependencies: vec![
                    "Xamarin.Forms".to_string(),
                    "Xamarin.Essentials".to_string(),
                ],
            },
            Some(nerd_icon("e8e7")),
            Some("#3498db"),
            3,
            vec![root_indicator(
                "*.xaml",
                0.9,
                IndicatorContext::FrameworkRoot,
            )],
        ),
    ]
}

use super::shared::{framework, nerd_icon, root_indicator, simple_framework};
use crate::types::{DetectionType, IndicatorContext, ProjectIndicator};

pub fn create_csharp_language() -> ProjectIndicator {
    ProjectIndicator::with_root_indicators(
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
        vec![
            simple_framework(
                "ASP.NET Core",
                DetectionType::DotNetEcosystem {
                    packages: vec![
                        "Microsoft.AspNetCore".to_string(),
                        "Microsoft.AspNetCore.App".to_string(),
                        "Microsoft.AspNetCore.Mvc".to_string(),
                    ],
                },
                Some(nerd_icon("e7c6")),
                Some("#512bd4"),
                1,
            ),
            framework(
                "Xamarin",
                DetectionType::DotNetEcosystem {
                    packages: vec![
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
            framework(
                ".NET MAUI",
                DetectionType::DotNetEcosystem {
                    packages: vec![
                        "Microsoft.Maui".to_string(),
                        "Microsoft.Maui.Controls".to_string(),
                    ],
                },
                Some(nerd_icon("e77f")),
                Some("#512bd4"),
                4,
                vec![root_indicator(
                    "Platforms/",
                    0.9,
                    IndicatorContext::FrameworkRoot,
                )],
            ),
        ],
        vec![
            root_indicator("*.csproj", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("*.sln", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("Program.cs", 0.85, IndicatorContext::LanguageRoot),
        ],
    )
}

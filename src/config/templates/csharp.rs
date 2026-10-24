use super::shared::{dependency_framework, nerd_icon, root_indicator, with_roots};
use crate::types::{Ecosystem, Framework, Indicator, IndicatorContext};

pub fn create_csharp_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "C#".to_string(),
        vec![
            "*.cs".to_string(),
            "*.csproj".to_string(),
            "*.sln".to_string(),
            "Program.cs".to_string(),
            "global.json".to_string(),
            "Directory.Build.props".to_string(),
        ],
        "#239120".to_string(),
        nerd_icon("e7b2"),
        10,
        vec![Ecosystem::Nuget],
        vec![
            root_indicator("*.csproj", 0.95, IndicatorContext::LanguageRoot),
            root_indicator("*.sln", 0.9, IndicatorContext::LanguageRoot),
            root_indicator("Program.cs", 0.85, IndicatorContext::LanguageRoot),
            root_indicator("global.json", 0.45, IndicatorContext::LanguageRoot),
            root_indicator(
                "Directory.Build.props",
                0.45,
                IndicatorContext::LanguageRoot,
            ),
        ],
    )
}

pub fn csharp_frameworks() -> Vec<Framework> {
    vec![
        dependency_framework(
            "ASP.NET Core",
            &[Ecosystem::Nuget],
            &[
                "Microsoft.AspNetCore",
                "Microsoft.AspNetCore.App",
                "Microsoft.AspNetCore.Mvc",
            ],
            Some("e77f"),
            Some("#512bd4"),
            1,
        ),
        with_roots(
            dependency_framework(
                "Xamarin",
                &[Ecosystem::Nuget],
                &["Xamarin.Forms", "Xamarin.Essentials"],
                Some("e8e7"),
                Some("#3498db"),
                3,
            ),
            vec![root_indicator(
                "*.xaml",
                0.9,
                IndicatorContext::FrameworkRoot,
            )],
        ),
    ]
}

//! Color theme support for terminal output

/// Trait for applying color themes to text output
pub trait ColorTheme: Send + Sync {
    /// Apply color to text, returning colored text if supported
    fn apply_color(&self, text: &str, color: &str) -> String;

    /// Check if colors are supported in the current environment
    fn supports_color(&self) -> bool;

    /// Get theme name
    fn name(&self) -> &str;
}

/// Default theme with basic color support
pub struct DefaultTheme;

impl ColorTheme for DefaultTheme {
    fn apply_color(&self, text: &str, color: &str) -> String {
        if !self.supports_color() {
            return text.to_string();
        }

        // Convert hex color to ANSI escape code
        if let Some(ansi_code) = hex_to_ansi(color) {
            format!("\x1b[{}m{}\x1b[0m", ansi_code, text)
        } else {
            text.to_string()
        }
    }

    fn supports_color(&self) -> bool {
        // Check common environment variables that indicate color support
        std::env::var("NO_COLOR").is_err()
            && (is_terminal()
                || std::env::var("FORCE_COLOR").is_ok()
                || std::env::var("COLORTERM").is_ok())
    }

    fn name(&self) -> &str {
        "default"
    }
}

/// No-color theme that never applies colors
pub struct NoColorTheme;

impl ColorTheme for NoColorTheme {
    fn apply_color(&self, text: &str, _color: &str) -> String {
        text.to_string()
    }

    fn supports_color(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "none"
    }
}

/// RGB color structure for advanced themes
#[derive(Debug, Clone, Copy)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// Create RGB color from hex string (e.g., "#FF0000")
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#')?;

        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(RgbColor { r, g, b })
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(RgbColor { r, g, b })
        } else {
            None
        }
    }

    /// Convert to 24-bit ANSI color code
    pub fn to_ansi_fg(&self) -> String {
        format!("38;2;{};{};{}", self.r, self.g, self.b)
    }

    /// Convert to 8-bit ANSI color (256-color mode)
    pub fn to_ansi_8bit(&self) -> String {
        let color_code = rgb_to_256(self.r, self.g, self.b);
        format!("38;5;{}", color_code)
    }
}

/// Enhanced theme with 24-bit color support
pub struct TrueColorTheme;

impl ColorTheme for TrueColorTheme {
    fn apply_color(&self, text: &str, color: &str) -> String {
        if !self.supports_color() {
            return text.to_string();
        }

        if let Some(rgb) = RgbColor::from_hex(color) {
            format!("\x1b[{}m{}\x1b[0m", rgb.to_ansi_fg(), text)
        } else {
            text.to_string()
        }
    }

    fn supports_color(&self) -> bool {
        // Check for 24-bit color support
        std::env::var("NO_COLOR").is_err()
            && (std::env::var("COLORTERM").as_deref() == Ok("truecolor")
                || std::env::var("COLORTERM").as_deref() == Ok("24bit")
                || (is_terminal() && supports_truecolor()))
    }

    fn name(&self) -> &str {
        "truecolor"
    }
}

/// Convert hex color to basic ANSI color code
fn hex_to_ansi(hex: &str) -> Option<&'static str> {
    match hex.to_uppercase().as_str() {
        "#000000" | "#000" => Some("30"), // Black
        "#800000" | "#800" => Some("31"), // Red
        "#008000" | "#080" => Some("32"), // Green
        "#808000" | "#880" => Some("33"), // Yellow
        "#000080" | "#008" => Some("34"), // Blue
        "#800080" | "#808" => Some("35"), // Magenta
        "#008080" | "#088" => Some("36"), // Cyan
        "#C0C0C0" | "#CCC" => Some("37"), // White
        "#808080" | "#888" => Some("90"), // Bright Black
        "#FF0000" | "#F00" => Some("91"), // Bright Red
        "#00FF00" | "#0F0" => Some("92"), // Bright Green
        "#FFFF00" | "#FF0" => Some("93"), // Bright Yellow
        "#0000FF" | "#00F" => Some("94"), // Bright Blue
        "#FF00FF" | "#F0F" => Some("95"), // Bright Magenta
        "#00FFFF" | "#0FF" => Some("96"), // Bright Cyan
        "#FFFFFF" | "#FFF" => Some("97"), // Bright White
        _ => None,
    }
}

/// Check if we're running in a terminal
fn is_terminal() -> bool {
    std::env::var("TERM").is_ok() && std::env::var("TERM").as_deref() != Ok("dumb")
}

/// Check if terminal supports 24-bit color
fn supports_truecolor() -> bool {
    // Common terminals that support truecolor
    if let Ok(term) = std::env::var("TERM") {
        matches!(
            term.as_str(),
            "xterm-256color"
                | "screen-256color"
                | "tmux-256color"
                | "alacritty"
                | "kitty"
                | "wezterm"
                | "iterm2"
        )
    } else {
        false
    }
}

/// Convert RGB to 256-color palette
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Convert to 6x6x6 color cube (216 colors) + grayscale (24 colors)
    if r == g && g == b {
        // Grayscale
        if r < 8 {
            16
        } else if r > 248 {
            231
        } else {
            232 + ((r - 8) / 10)
        }
    } else {
        // Color cube: 16 + 36*r + 6*g + b (where r,g,b are 0-5)
        let r_cube = (r as u16 * 5 / 255) as u8;
        let g_cube = (g as u16 * 5 / 255) as u8;
        let b_cube = (b as u16 * 5 / 255) as u8;
        16 + 36 * r_cube + 6 * g_cube + b_cube
    }
}

/// Create appropriate theme based on environment
pub fn create_theme() -> Box<dyn ColorTheme> {
    if std::env::var("NO_COLOR").is_ok() {
        Box::new(NoColorTheme)
    } else if TrueColorTheme.supports_color() {
        Box::new(TrueColorTheme)
    } else {
        Box::new(DefaultTheme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_color_from_hex() {
        let red = RgbColor::from_hex("#FF0000").unwrap();
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);

        let short_red = RgbColor::from_hex("#F00").unwrap();
        assert_eq!(short_red.r, 255);
        assert_eq!(short_red.g, 0);
        assert_eq!(short_red.b, 0);

        let blue = RgbColor::from_hex("#0000FF").unwrap();
        assert_eq!(blue.r, 0);
        assert_eq!(blue.g, 0);
        assert_eq!(blue.b, 255);

        assert!(RgbColor::from_hex("invalid").is_none());
        assert!(RgbColor::from_hex("#GGGGGG").is_none());
    }

    #[test]
    fn test_rgb_to_ansi() {
        let red = RgbColor { r: 255, g: 0, b: 0 };
        assert_eq!(red.to_ansi_fg(), "38;2;255;0;0");

        let green = RgbColor { r: 0, g: 255, b: 0 };
        assert_eq!(green.to_ansi_fg(), "38;2;0;255;0");
    }

    #[test]
    fn test_hex_to_ansi_basic_colors() {
        assert_eq!(hex_to_ansi("#FF0000"), Some("91")); // Bright Red
        assert_eq!(hex_to_ansi("#00FF00"), Some("92")); // Bright Green
        assert_eq!(hex_to_ansi("#0000FF"), Some("94")); // Bright Blue
        assert_eq!(hex_to_ansi("#FFFFFF"), Some("97")); // Bright White
        assert_eq!(hex_to_ansi("#000000"), Some("30")); // Black

        // Short form
        assert_eq!(hex_to_ansi("#F00"), Some("91")); // Bright Red
        assert_eq!(hex_to_ansi("#0F0"), Some("92")); // Bright Green

        // Unknown color
        assert_eq!(hex_to_ansi("#123456"), None);
    }

    #[test]
    fn test_rgb_to_256() {
        // Test pure colors
        assert_eq!(rgb_to_256(255, 0, 0), 196); // Red
        assert_eq!(rgb_to_256(0, 255, 0), 46); // Green
        assert_eq!(rgb_to_256(0, 0, 255), 21); // Blue

        // Test grayscale
        assert_eq!(rgb_to_256(128, 128, 128), 244); // Gray
        assert_eq!(rgb_to_256(0, 0, 0), 16); // Black
        assert_eq!(rgb_to_256(255, 255, 255), 231); // White
    }

    #[test]
    fn test_no_color_theme() {
        let theme = NoColorTheme;
        assert!(!theme.supports_color());
        assert_eq!(theme.apply_color("test", "#FF0000"), "test");
        assert_eq!(theme.name(), "none");
    }

    #[test]
    fn test_default_theme() {
        let theme = DefaultTheme;
        assert_eq!(theme.name(), "default");

        // Test with a known color
        if theme.supports_color() {
            let colored = theme.apply_color("test", "#FF0000");
            assert!(colored.contains("test"));
            assert!(colored.contains("\x1b["));
        } else {
            // If no color support, should return plain text
            assert_eq!(theme.apply_color("test", "#FF0000"), "test");
        }
    }

    #[test]
    fn test_truecolor_theme() {
        let theme = TrueColorTheme;
        assert_eq!(theme.name(), "truecolor");

        // Test with a hex color
        if theme.supports_color() {
            let colored = theme.apply_color("test", "#3178C6");
            assert!(colored.contains("test"));
            assert!(colored.contains("38;2;")); // 24-bit color escape
        }
    }

    #[test]
    fn test_theme_creation() {
        let theme = create_theme();
        assert!(!theme.name().is_empty());

        // Should create some kind of theme
        let result = theme.apply_color("test", "#FF0000");
        assert!(result.contains("test"));
    }
}

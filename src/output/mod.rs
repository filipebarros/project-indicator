//! Output formatting for detection results

pub mod formatters;
pub mod themes;

pub use formatters::{OutputFormat, OutputFormatter};
pub use themes::{ColorTheme, DefaultTheme};

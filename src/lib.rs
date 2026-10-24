#![forbid(unsafe_code)]

pub mod cache;
pub mod cli;
pub mod config;
pub mod constants;
pub mod detection;
pub mod output;
pub mod patterns;
pub mod performance;
pub mod types;

#[cfg(test)]
pub mod test_utils;

pub use config::Config;
pub use detection::DetectionEngine;
pub type Result<T, E = anyhow::Error> = std::result::Result<T, E>;

//! DNS module.
//!
//! This module provides DNS-related functionality including:
//! - Speed testing via ICMP ping
//! - Pollution detection
//! - Core data types

pub mod pollution;
pub mod speedtest;
pub mod types;

pub use pollution::PollutionChecker;
pub use speedtest::SpeedTester;
pub use types::*;

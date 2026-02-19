//! dnstest - A modern DNS speed testing and pollution detection tool.
//!
//! This crate provides both a library API and a CLI tool for:
//! - Testing DNS server response times using ICMP ping
//! - Detecting DNS pollution (censorship or hijacking)
//! - Interactive TUI for easy navigation
//! - Multiple output formats (table, JSON, CSV, TSV)
//!
//! # Library Usage
//!
//! ```ignore
//! use dnstest::{DnsServer, SpeedTester, PollutionChecker};
//!
//! // Test DNS speed
//! let tester = SpeedTester::new()?;
//! let result = tester.test_latency(&server).await;
//!
//! // Check DNS pollution
//! let checker = PollutionChecker::new()?;
//! let result = checker.check("google.com").await?;
//! ```
//!
//! # CLI Usage
//!
//! ```bash
//! # Interactive TUI mode (default)
//! dnstest
//!
//! # DNS speed test
//! dnstest speed
//! dnstest speed --sort
//! dnstest speed --dns 8.8.8.8#Google
//!
//! # DNS pollution check
//! dnstest check google.com
//!
//! # List DNS servers
//! dnstest list
//! dnstest list --ipv4
//!
//! # Export DNS list
//! dnstest export --output mylist.json
//! ```
//!
//! # Features
//!
//! - **DNS Speed Testing**: Measure latency to DNS servers using ICMP ping
//! - **Pollution Detection**: Compare system DNS with public DNS to detect tampering
//! - **Interactive TUI**: User-friendly terminal interface
//! - **Multiple Formats**: Output results in table, JSON, CSV, or TSV format
//! - **IPv4/IPv6 Support**: Works with both address families

pub mod cli;
pub mod config;
pub mod dns;
pub mod error;
pub mod tui;

// Re-export commonly used types
pub use cli::{Cli, Commands, OutputFormat};
pub use config::ConfigLoader;
pub use dns::types::{DnsList, DnsServer, PollutionResult, SpeedTestResult, TestSummary};
pub use dns::{PollutionChecker, SpeedTester};
pub use error::{Error, Result};

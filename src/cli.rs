//! Command-line interface (CLI) argument parsing module.
//!
//! This module provides CLI argument parsing using `clap`.
//! It supports multiple commands: interactive mode, speed test, pollution check,
//! listing DNS servers, and exporting DNS lists.

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CLI argument parser using clap derive macro.
///
/// # Example
///
/// ```ignore
/// let cli = Cli::parse();
/// match cli.command {
///     Some(Commands::Speed { file, .. }) => { /* ... */ }
///     Some(Commands::Check { domain, .. }) => { /* ... */ }
///     None => { /* interactive mode */ }
/// }
/// ```
#[derive(Parser, Debug)]
#[command(
    name = "dnstest",
    version,
    about = "DNS测速与污染检测工具",
    long_about = "一款现代化DNS测速和污染检测CLI工具，支持TUI交互界面",
    infer_subcommands = true
)]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Quiet mode (only errors)
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Output format for CLI commands.
///
/// This enum represents different output formats that can be used
/// when displaying DNS test results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Table format (default, human-readable)
    #[default]
    Table,
    /// JSON format
    Json,
    /// CSV format
    Csv,
    /// TSV format (tab-separated)
    Tsv,
}

impl OutputFormat {
    /// Get all available output format names.
    #[must_use]
    pub fn names() -> &'static [&'static str] {
        &["table", "json", "csv", "tsv"]
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            _ => Err(format!(
                "Unknown format: {}. Valid options are: {:?}",
                s,
                Self::names()
            )),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Json => write!(f, "json"),
            Self::Csv => write!(f, "csv"),
            Self::Tsv => write!(f, "tsv"),
        }
    }
}

/// Available commands for the dnstest CLI.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 启动交互式TUI界面
    ///
    /// Launch the interactive terminal user interface (TUI).
    /// This provides a menu-based interface for DNS testing.
    #[command(alias = "i")]
    Interactive {
        /// Load custom DNS list file (JSON format)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// DNS测速
    ///
    /// Test DNS server response times using ICMP ping.
    /// Results can be sorted by latency and displayed in various formats.
    #[command(alias = "s")]
    Speed {
        /// DNS list file (JSON format)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Number of pings per server
        #[arg(short, long, default_value = "3")]
        count: usize,

        /// Timeout in seconds
        #[arg(short, long, default_value = "5")]
        timeout: u64,

        /// Custom DNS servers (format: IP#Name)
        #[arg(long = "dns")]
        dns_servers: Vec<String>,

        /// Sort by latency (fastest first)
        #[arg(long = "sort")]
        sort_by_latency: bool,
    },

    /// DNS污染检测
    ///
    /// Check if DNS responses are being polluted (censored or hijacked).
    /// Compares system DNS resolution results with public DNS servers.
    #[command(alias = "c")]
    Check {
        /// Domain to check (default: google.com)
        #[arg(short, long, default_value = "google.com")]
        domain: String,

        /// Check multiple domains from file
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// 列出可用的DNS服务器
    ///
    /// List all available DNS servers from the default list or a custom file.
    /// Can filter by IP version (IPv4/IPv6).
    #[command(alias = "l")]
    List {
        /// DNS list file
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Show only IPv4 servers
        #[arg(long = "ipv4")]
        ipv4_only: bool,

        /// Show only IPv6 servers
        #[arg(long = "ipv6")]
        ipv6_only: bool,
    },

    /// 从网络更新 DNS 列表
    ///
    /// Update DNS list from remote URL (GitHub Pages).
    /// Downloads the latest DNS list from the configured URL.
    #[command(alias = "u")]
    Update {
        /// URL to download DNS list from (default: GitHub Pages)
        #[arg(short, long)]
        url: Option<String>,

        /// Output file path (default: dnslist.json in current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// 导出DNS列表
    ///
    /// Export the merged DNS server list to a JSON file.
    /// Includes both IPv4 and IPv6 servers by default.
    #[command(alias = "e")]
    Export {
        /// Output file path
        #[arg(short, long, default_value = "dnslist.json")]
        output: PathBuf,

        /// Include IPv6 servers in export
        #[arg(long = "ipv6")]
        include_ipv6: bool,
    },
}

/// Parse CLI arguments without verbose flag.
///
/// # Returns
///
/// Returns the parsed `Cli` struct.
#[must_use]
pub fn parse() -> Cli {
    Cli::parse()
}

/// Parse CLI arguments and return verbose flag.
///
/// # Returns
///
/// Returns a tuple of `(Cli, verbose)` where `verbose` indicates
/// whether verbose logging was enabled.
#[must_use]
pub fn parse_verbose() -> (Cli, bool) {
    let cli = Cli::parse();
    let verbose = cli.verbose;
    (cli, verbose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parse() {
        assert_eq!("table".parse::<OutputFormat>(), Ok(OutputFormat::Table));
        assert_eq!("json".parse::<OutputFormat>(), Ok(OutputFormat::Json));
        assert_eq!("csv".parse::<OutputFormat>(), Ok(OutputFormat::Csv));
        assert_eq!("tsv".parse::<OutputFormat>(), Ok(OutputFormat::Tsv));
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Csv.to_string(), "csv");
        assert_eq!(OutputFormat::Tsv.to_string(), "tsv");
    }

    #[test]
    fn test_output_format_default() {
        assert_eq!(OutputFormat::default(), OutputFormat::Table);
    }
}

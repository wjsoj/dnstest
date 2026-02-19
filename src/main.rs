//! dnstest - DNS测速与污染检测工具
//!
//! Binary entry point for the dnstest CLI application.

#![warn(clippy::all, warnings)]
#![warn(clippy::pedantic, clippy::nursery)]

use dnstest::cli::{Commands, OutputFormat};
use dnstest::config::ConfigLoader;
use dnstest::dns::{self, DnsServer, PollutionChecker, SpeedTester};
use dnstest::error::Result;
use dnstest::tui::App;
use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Set up logging based on verbosity level.
///
/// # Arguments
///
/// * `verbose` - Enable debug-level logging
/// * `quiet` - Enable error-level only logging
fn setup_logging(verbose: bool, quiet: bool) {
    let filter = if quiet {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"))
    } else if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().without_time())
        .init();
}

/// Load DNS server list from file or command-line arguments.
///
/// # Arguments
///
/// * `file` - Optional path to DNS list JSON file
/// * `dns_args` - Optional command-line DNS server specifications (IP#Name)
fn load_dns_list(file: Option<PathBuf>, dns_args: Vec<String>) -> Result<Vec<DnsServer>> {
    if !dns_args.is_empty() {
        let list = ConfigLoader::from_args(dns_args)?;
        return Ok(list.servers);
    }

    if let Some(path) = file {
        let list = ConfigLoader::load_from_file(path)?;
        return Ok(list.servers);
    }

    // Try to load default
    let lists = ConfigLoader::load_all()?;
    Ok(ConfigLoader::merge(lists).servers)
}

/// Run DNS speed test and output results.
///
/// # Arguments
///
/// * `file` - Optional DNS list file
/// * `dns_servers` - Optional custom DNS servers
/// * `sort_by_latency` - Whether to sort results by latency
/// * `format` - Output format
async fn run_speed_test(
    file: Option<PathBuf>,
    dns_servers: Vec<String>,
    sort_by_latency: bool,
    format: OutputFormat,
) -> Result<()> {
    println!("加载DNS列表...");
    let servers = load_dns_list(file, dns_servers)?;

    println!("开始DNS测速 (共 {} 个服务器)...\n", servers.len());

    let tester = SpeedTester::new()?;
    let mut results = Vec::new();
    let total = servers.len();

    for (idx, server) in servers.iter().enumerate() {
        print!(
            "\r测速中 [{:>3}/{}] {} ({})",
            idx + 1,
            total,
            server.name,
            server.ip
        );
        std::io::Write::flush(&mut std::io::stdout())?;

        let result = tester.test_latency(server).await;
        results.push(result);
    }

    println!("\n");

    // Sort if requested
    if sort_by_latency {
        results.sort_by(|a, b| {
            let a_lat = a.latency_ms.unwrap_or(f64::MAX);
            let b_lat = b.latency_ms.unwrap_or(f64::MAX);
            a_lat.partial_cmp(&b_lat).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Output results
    match format {
        OutputFormat::Table => print_results_table(&results),
        OutputFormat::Json => print_results_json(&results),
        OutputFormat::Csv => print_results_csv(&results),
        OutputFormat::Tsv => print_results_tsv(&results),
    }

    // Summary
    let summary = SpeedTester::summarize(&results);
    println!("\n=== 统计 ===");
    println!("总服务器数: {}", summary.total);
    println!("成功: {}", summary.success);
    println!("失败/超时: {}", summary.failed + summary.timeout);
    if let Some(avg) = summary.avg_latency {
        println!("平均延迟: {:.2} ms", avg);
    }
    if let Some(min) = summary.min_latency {
        println!("最低延迟: {:.2} ms", min);
    }
    if let Some(max) = summary.max_latency {
        println!("最高延迟: {:.2} ms", max);
    }

    Ok(())
}

/// Print results in table format.
fn print_results_table(results: &[dns::SpeedTestResult]) {
    println!("{:<4} {:<20} {:<18} {:<12}", "#", "名称", "IP", "延迟");
    println!("{}", "-".repeat(60));

    for (idx, r) in results.iter().enumerate() {
        let latency = r
            .latency_ms
            .map(|l| format!("{:.1} ms", l))
            .unwrap_or_else(|| "Timeout".to_string());

        let status = if r.success { "" } else { "[失败] " };

        println!(
            "{:<4} {:<20} {:<18} {:<12}",
            idx + 1,
            format!("{}{}", status, r.server.name),
            r.server.ip,
            latency
        );
    }
}

/// Print results in JSON format.
fn print_results_json(results: &[dns::SpeedTestResult]) {
    let json = serde_json::to_string_pretty(results).unwrap();
    println!("{json}");
}

/// Print results in CSV format.
fn print_results_csv(results: &[dns::SpeedTestResult]) {
    println!("#Idx,Name,IP,Latency(ms),Success");
    for (idx, r) in results.iter().enumerate() {
        let latency = r.latency_ms.unwrap_or(-1.0);
        println!(
            "{},{},{},{:.1},{}",
            idx + 1,
            r.server.name,
            r.server.ip,
            latency,
            r.success
        );
    }
}

/// Print results in TSV format.
fn print_results_tsv(results: &[dns::SpeedTestResult]) {
    println!("#\tName\tIP\tLatency(ms)\tSuccess");
    for (idx, r) in results.iter().enumerate() {
        let latency = r.latency_ms.unwrap_or(-1.0);
        println!(
            "{}\t{}\t{}\t{:.1}\t{}",
            idx + 1,
            r.server.name,
            r.server.ip,
            latency,
            r.success
        );
    }
}

/// Run DNS pollution check for a domain.
///
/// # Arguments
///
/// * `domain` - Domain name to check
/// * `format` - Output format
async fn run_pollution_check(domain: String, format: OutputFormat) -> Result<()> {
    println!("检测域名: {}", domain);
    println!("正在解析...\n");

    let checker = PollutionChecker::new()?;
    let result = checker.check(&domain).await?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result).unwrap();
            println!("{json}");
        }
        _ => {
            println!("域名: {}", result.domain);
            println!("系统DNS解析: {:?}", result.system_ips);
            println!("公共DNS解析: {:?}", result.public_ips);
            println!(
                "污染检测: {}",
                if result.is_polluted { "可能污染" } else { "正常" }
            );
            println!("详情: {}", result.details);
        }
    }

    Ok(())
}

/// List DNS servers with optional filtering.
///
/// # Arguments
///
/// * `file` - Optional DNS list file
/// * `ipv4_only` - Show only IPv4 servers
/// * `ipv6_only` - Show only IPv6 servers
fn run_list_dns(file: Option<PathBuf>, ipv4_only: bool, ipv6_only: bool) -> Result<()> {
    let servers = if let Some(path) = file {
        ConfigLoader::load_from_file(path)?.servers
    } else {
        let lists = ConfigLoader::load_all()?;
        ConfigLoader::merge(lists).servers
    };

    let filtered: Vec<_> = servers
        .into_iter()
        .filter(|s| {
            let ip: std::net::IpAddr =
                s.ip.parse().unwrap_or_else(|_| "0.0.0.0".parse().unwrap());
            let is_v4 = ip.is_ipv4();
            let is_v6 = ip.is_ipv6();

            if ipv4_only && !is_v4 {
                return false;
            }
            if ipv6_only && !is_v6 {
                return false;
            }
            true
        })
        .collect();

    println!("DNS服务器列表 (共 {} 个):\n", filtered.len());
    println!("{:<4} {:<20} {:<20}", "#", "名称", "IP");
    println!("{}", "-".repeat(50));

    for (idx, s) in filtered.iter().enumerate() {
        println!("{:<4} {:<20} {:<20}", idx + 1, s.name, s.ip);
    }

    Ok(())
}

/// Run interactive TUI mode.
async fn run_interactive(file: Option<PathBuf>) -> Result<()> {
    let mut app = App::new();

    // Load custom file if provided
    if let Some(path) = file {
        if let Ok(list) = ConfigLoader::load_from_file(&path) {
            app.set_dns_servers(list.servers);
        }
    }

    app.run().await?;
    Ok(())
}

/// Main entry point for the dnstest CLI application.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook for better error reporting
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("程序崩溃: {}", panic_info);
    }));

    let (cli, verbose) = dnstest::cli::parse_verbose();
    setup_logging(verbose, cli.quiet);

    tracing::info!("dnstest starting...");

    match cli.command {
        Some(Commands::Interactive { file }) => {
            run_interactive(file).await?;
        }

        Some(Commands::Speed {
            file,
            count: _,
            timeout: _,
            dns_servers,
            sort_by_latency,
        }) => {
            run_speed_test(file, dns_servers, sort_by_latency, cli.format).await?;
        }

        Some(Commands::Check { domain, file: _ }) => {
            run_pollution_check(domain, cli.format).await?;
        }

        Some(Commands::List {
            file,
            ipv4_only,
            ipv6_only,
        }) => {
            run_list_dns(file, ipv4_only, ipv6_only)?;
        }

        Some(Commands::Export {
            output,
            include_ipv6: _,
        }) => {
            let lists = ConfigLoader::load_all()?;
            let merged = ConfigLoader::merge(lists);
            let json = serde_json::to_string_pretty(&merged)?;
            std::fs::write(&output, json)?;
            println!("已导出到: {}", output.display());
        }

        None => {
            // Default to interactive mode
            run_interactive(None).await?;
        }
    }

    Ok(())
}

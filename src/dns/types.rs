//! DNS types and data structures.
//!
//! This module provides the core types used for DNS server representation,
//! test results, and pollution detection results.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// DNS server information.
///
/// Represents a single DNS server with its name, IP address,
/// and optional delay/status information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DnsServer {
    /// Server name (e.g., "Cloudflare DNS", "Google Public DNS")
    pub name: String,
    /// IP address of the DNS server
    #[serde(rename = "IP")]
    pub ip: String,
    /// Response delay in milliseconds (optional)
    #[serde(default)]
    pub delay: Option<f64>,
    /// Current status of the server
    #[serde(default)]
    pub status: DnsStatus,
}

impl DnsServer {
    /// Create a new DNS server.
    ///
    /// # Arguments
    ///
    /// * `name` - Server name
    /// * `ip` - IP address (IPv4 or IPv6)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let server = DnsServer::new("Cloudflare", "1.1.1.1");
    /// ```
    pub fn new(name: impl Into<String>, ip: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ip: ip.into(),
            delay: None,
            status: DnsStatus::Pending,
        }
    }

    /// Parse the IP address string into an `IpAddr`.
    ///
    /// # Returns
    ///
    /// Returns `Some(IpAddr)` if parsing succeeds, `None` otherwise.
    #[must_use]
    pub fn ip_addr(&self) -> Option<IpAddr> {
        self.ip.parse().ok()
    }

    /// Check if the server uses IPv4.
    #[must_use]
    pub fn is_ipv4(&self) -> bool {
        self.ip_addr().is_some_and(|ip| ip.is_ipv4())
    }

    /// Check if the server uses IPv6.
    #[must_use]
    pub fn is_ipv6(&self) -> bool {
        self.ip_addr().is_some_and(|ip| ip.is_ipv6())
    }
}

/// DNS server testing status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DnsStatus {
    /// Server is pending testing
    #[default]
    Pending,
    /// Server is currently being tested
    Testing,
    /// Server test completed successfully
    Success,
    /// Server test failed
    Failed,
    /// Server test timed out
    Timeout,
}

impl DnsStatus {
    /// Check if the status indicates a successful test.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if the status indicates a failure.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Timeout)
    }
}

/// DNS server list container.
///
/// Represents a collection of DNS servers, typically loaded from
/// a JSON configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsList {
    /// List of DNS servers
    #[serde(rename = "list")]
    pub servers: Vec<DnsServer>,
}

impl DnsList {
    /// Create a new empty DNS list.
    #[must_use]
    pub fn new() -> Self {
        Self { servers: vec![] }
    }

    /// Create a DNS list from a vector of servers.
    #[must_use]
    pub fn from_servers(servers: Vec<DnsServer>) -> Self {
        Self { servers }
    }

    /// Get the number of servers in the list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.servers.len()
    }

    /// Check if the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
}

impl Default for DnsList {
    fn default() -> Self {
        Self::new()
    }
}

/// DNS speed test result.
///
/// Contains the results of testing a single DNS server's response time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestResult {
    /// The DNS server that was tested
    pub server: DnsServer,
    /// Latency in milliseconds (None if failed/timeout)
    pub latency_ms: Option<f64>,
    /// Packet loss ratio (0.0 = no loss, 1.0 = all lost)
    pub packet_loss: f64,
    /// Whether the test was successful
    pub success: bool,
    /// Error message if the test failed
    pub error: Option<String>,
}

impl SpeedTestResult {
    /// Create a successful result.
    #[must_use]
    pub fn success(server: DnsServer, latency_ms: f64, packet_loss: f64) -> Self {
        Self {
            server,
            latency_ms: Some(latency_ms),
            packet_loss,
            success: true,
            error: None,
        }
    }

    /// Create a failed result.
    pub fn failure(server: DnsServer, error: impl Into<String>) -> Self {
        Self {
            server,
            latency_ms: None,
            packet_loss: 1.0,
            success: false,
            error: Some(error.into()),
        }
    }

    /// Check if the result indicates a timeout.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        !self.success && matches!(self.error.as_deref(), Some("timeout"))
    }
}

/// DNS pollution check result.
///
/// Contains the results of comparing system DNS resolution
/// with public DNS servers to detect potential pollution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollutionResult {
    /// Domain name that was checked
    pub domain: String,
    /// IP addresses returned by system DNS
    pub system_ips: Vec<IpAddr>,
    /// IP addresses returned by public DNS servers
    pub public_ips: Vec<IpAddr>,
    /// Whether pollution was detected
    pub is_polluted: bool,
    /// Human-readable details about the result
    pub details: String,
}

impl PollutionResult {
    /// Create a pollution check result.
    #[allow(dead_code)]
    #[must_use]
    pub fn new(
        domain: String,
        system_ips: Vec<IpAddr>,
        public_ips: Vec<IpAddr>,
        is_polluted: bool,
        details: String,
    ) -> Self {
        Self {
            domain,
            system_ips,
            public_ips,
            is_polluted,
            details,
        }
    }
}

/// Overall test summary statistics.
///
/// Aggregated results from multiple DNS speed tests.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestSummary {
    /// Total number of servers tested
    pub total: usize,
    /// Number of successful tests
    pub success: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of timeouts
    pub timeout: usize,
    /// Average latency in milliseconds
    pub avg_latency: Option<f64>,
    /// Minimum latency in milliseconds
    pub min_latency: Option<f64>,
    /// Maximum latency in milliseconds
    pub max_latency: Option<f64>,
}

impl TestSummary {
    /// Create a new empty summary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a test result to the summary.
    pub fn add_result(&mut self, result: &SpeedTestResult) {
        self.total += 1;
        if result.success {
            self.success += 1;
            if let Some(latency) = result.latency_ms {
                self.avg_latency = Some(
                    self.avg_latency
                        .map(|a| {
                            a.mul_add((self.success - 1) as f64, latency) / self.success as f64
                        })
                        .unwrap_or(latency),
                );
                self.min_latency =
                    Some(self.min_latency.map(|m| m.min(latency)).unwrap_or(latency));
                self.max_latency =
                    Some(self.max_latency.map(|m| m.max(latency)).unwrap_or(latency));
            }
        } else if result.is_timeout() {
            self.timeout += 1;
        } else {
            self.failed += 1;
        }
    }

    /// Calculate success rate as a percentage.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success as f64 / self.total as f64) * 100.0
        }
    }
}

//! DNS speed test using ICMP ping.
//!
//! This module provides functionality to test DNS server response times
//! using ICMP ping (Internet Control Message Protocol).

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::items_after_statements)]

use crate::dns::types::{DnsServer, SpeedTestResult, TestSummary};
use crate::error::{Error, Result};
use std::time::{Duration, Instant};
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tokio::time::timeout;

/// Default packet size for ping in bytes.
const DEFAULT_PACKET_SIZE: usize = 32;

/// Default timeout for each ping attempt in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 5;

/// Default number of ping attempts per server.
const DEFAULT_PING_COUNT: usize = 3;

/// DNS speed tester.
///
/// This struct provides methods to test DNS server response times
/// using ICMP ping. It requires appropriate permissions to send
/// ICMP packets (typically needs root or raw socket access).
///
/// # Example
///
/// ```ignore
/// let tester = SpeedTester::new()?;
/// let server = DnsServer::new("Cloudflare", "1.1.1.1");
/// let result = tester.test_latency(&server).await;
/// ```
pub struct SpeedTester {
    client: Client,
    timeout: Duration,
    ping_count: usize,
}

impl SpeedTester {
    /// Create a new `SpeedTester` with default settings.
    ///
    /// # Errors
    ///
    /// Returns an error if the ICMP client cannot be initialized
    /// (e.g., due to insufficient permissions or system limitations).
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let client = Client::new(&config).map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self {
            client,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            ping_count: DEFAULT_PING_COUNT,
        })
    }

    /// Create a new `SpeedTester` with custom settings.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Timeout for each ping attempt
    /// * `ping_count` - Number of ping attempts per server
    ///
    /// # Errors
    ///
    /// Returns an error if the ICMP client cannot be initialized.
    pub fn with_settings(timeout: Duration, ping_count: usize) -> Result<Self> {
        let config = Config::default();
        let client = Client::new(&config).map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self {
            client,
            timeout,
            ping_count,
        })
    }

    /// Test latency to a single DNS server using ICMP ping.
    ///
    /// Performs multiple ping attempts and calculates the average latency.
    ///
    /// # Arguments
    ///
    /// * `server` - The DNS server to test
    ///
    /// # Returns
    ///
    /// Returns a `SpeedTestResult` containing the test outcome.
    pub async fn test_latency(&self, server: &DnsServer) -> SpeedTestResult {
        let ip = match server.ip_addr() {
            Some(ip) => ip,
            None => {
                return SpeedTestResult::failure(server.clone(), "Invalid IP address");
            }
        };

        // Skip IPv6 for now as it requires special handling
        if ip.is_ipv6() {
            return SpeedTestResult::failure(server.clone(), "IPv6 not supported yet");
        }

        let payload = [0u8; DEFAULT_PACKET_SIZE];
        let mut latencies = Vec::new();
        let mut success_count = 0;

        for seq in 0..self.ping_count {
            let mut pinger = self.client.pinger(ip, PingIdentifier(rand_id())).await;

            pinger.timeout(self.timeout);

            let start = Instant::now();
            let result = timeout(
                self.timeout,
                pinger.ping(PingSequence(seq as u16), &payload),
            )
            .await;

            match result {
                Ok(Ok(_response)) => {
                    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                    latencies.push(elapsed);
                    success_count += 1;
                }
                Ok(Err(e)) => {
                    tracing::debug!("Ping error for {ip}: {e}");
                }
                Err(_) => {
                    // Timeout
                }
            }
        }

        let packet_loss = 1.0 - (success_count as f64 / self.ping_count as f64);

        if success_count > 0 {
            let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
            SpeedTestResult::success(server.clone(), avg_latency, packet_loss)
        } else {
            SpeedTestResult::failure(server.clone(), "timeout")
        }
    }

    /// Test multiple DNS servers sequentially.
    ///
    /// # Arguments
    ///
    /// * `servers` - Slice of DNS servers to test
    /// * `progress_callback` - Optional callback for progress updates
    ///
    /// # Returns
    ///
    /// Returns a vector of test results.
    pub async fn test_all(
        &self,
        servers: &[DnsServer],
        progress_callback: Option<impl Fn(usize, usize, &DnsServer)>,
    ) -> Vec<SpeedTestResult> {
        let total = servers.len();
        let mut results = Vec::with_capacity(total);

        // Process in batches to avoid overwhelming the network
        const BATCH_SIZE: usize = 20;

        for (idx, server) in servers.iter().enumerate() {
            if let Some(ref cb) = progress_callback {
                cb(idx, total, server);
            }

            let result = self.test_latency(server).await;
            results.push(result);

            // Small delay between batches
            if (idx + 1) % BATCH_SIZE == 0 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        results
    }

    /// Calculate summary statistics from results.
    ///
    /// # Arguments
    ///
    /// * `results` - Slice of speed test results
    ///
    /// # Returns
    ///
    /// Returns a `TestSummary` with aggregated statistics.
    #[must_use]
    pub fn summarize(results: &[SpeedTestResult]) -> TestSummary {
        let mut summary = TestSummary::new();
        for result in results {
            summary.add_result(result);
        }
        summary
    }
}

impl Default for SpeedTester {
    fn default() -> Self {
        Self::new().expect("Failed to create default SpeedTester")
    }
}

/// Generate a random ping identifier.
fn rand_id() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 65536) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ping_localhost() {
        // This test requires ICMP socket permissions which are not available in CI
        // Skip if CI environment variable is set
        if std::env::var("CI").is_ok() {
            return;
        }

        let tester = SpeedTester::new().unwrap();
        let server = DnsServer::new("localhost", "127.0.0.1");
        let result = tester.test_latency(&server).await;

        // Localhost should respond quickly
        if result.success {
            assert!(result.latency_ms.is_some());
            assert!(result.latency_ms.unwrap() < 10.0);
        }
    }

    #[test]
    fn test_speedtest_result() {
        let server = DnsServer::new("Test", "8.8.8.8");

        let success_result = SpeedTestResult::success(server.clone(), 10.0, 0.0);
        assert!(success_result.success);
        assert_eq!(success_result.latency_ms, Some(10.0));
        assert!(success_result.error.is_none());

        let failure_result = SpeedTestResult::failure(server.clone(), "timeout");
        assert!(!failure_result.success);
        assert!(failure_result.latency_ms.is_none());
        assert!(failure_result.error.is_some());
    }

    #[test]
    fn test_test_summary() {
        let server = DnsServer::new("Test", "8.8.8.8");
        let result1 = SpeedTestResult::success(server.clone(), 10.0, 0.0);
        let result2 = SpeedTestResult::success(server.clone(), 20.0, 0.0);
        let result3 = SpeedTestResult::failure(server.clone(), "timeout");

        let results = vec![result1, result2, result3];
        let summary = SpeedTester::summarize(&results);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.success, 2);
        assert_eq!(summary.timeout, 1);
        assert_eq!(summary.avg_latency, Some(15.0));
        assert_eq!(summary.min_latency, Some(10.0));
        assert_eq!(summary.max_latency, Some(20.0));
    }
}

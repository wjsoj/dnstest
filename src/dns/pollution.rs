//! DNS pollution detection.
//!
//! This module provides functionality to detect DNS pollution (also known as
//! DNS hijacking or DNS censorship). It works by comparing DNS resolution
//! results from the system DNS with results from known public DNS servers.

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use crate::dns::types::PollutionResult;
use crate::error::Result;
use std::net::IpAddr;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use trust_dns_resolver::name_server::TokioHandle;
use trust_dns_resolver::TokioAsyncResolver;

/// Google Public DNS IPv4 addresses.
const GOOGLE_DNS: &str = "8.8.8.8";

/// Cloudflare Public DNS IPv4 addresses.
const CLOUDFLARE_DNS: &str = "1.1.1.1";

/// List of known public DNS server IP addresses.
/// Used to identify legitimate DNS responses.
const PUBLIC_DNS_IPS: &[&str] = &[
    // IPv4
    "8.8.8.8",
    "8.8.4.4",
    "1.1.1.1",
    "1.0.0.1",
    "9.9.9.9",
    "208.67.222.222",
    "208.67.220.220",
    // IPv6
    "2001:4860:4860::8888",
    "2001:4860:4860::8844",
    "2606:4700:4700::1111",
    "2606:4700:4700::1001",
    "2620:fe::fe",
    "2620:fe::9",
];

/// DNS pollution checker.
///
/// Compares system DNS resolution results with public DNS servers
/// to detect potential DNS pollution or hijacking.
///
/// # Example
///
/// ```ignore
/// let checker = PollutionChecker::new()?;
/// let result = checker.check("google.com").await?;
/// if result.is_polluted {
///     println!("DNS pollution detected!");
/// }
/// ```
pub struct PollutionChecker {
    system_resolver: TokioAsyncResolver,
    public_resolver: TokioAsyncResolver,
}

impl PollutionChecker {
    /// Create a new `PollutionChecker`.
    ///
    /// Initializes both system DNS resolver and public DNS resolver
    /// (using Google and Cloudflare DNS).
    ///
    /// # Errors
    ///
    /// Returns an error if either resolver cannot be initialized.
    pub fn new() -> Result<Self> {
        // System default resolver
        let system_resolver = TokioAsyncResolver::from_system_conf(TokioHandle)
            .map_err(crate::error::Error::Resolver)?;

        // Public DNS resolver (Google DNS + Cloudflare)
        let public_config = ResolverConfig::from_parts(
            None,
            vec![],
            trust_dns_resolver::config::NameServerConfigGroup::from_ips_clear(
                &[GOOGLE_DNS.parse().unwrap(), CLOUDFLARE_DNS.parse().unwrap()],
                53,
                true,
            ),
        );
        let public_resolver = TokioAsyncResolver::tokio(public_config, ResolverOpts::default())
            .map_err(crate::error::Error::Resolver)?;

        Ok(Self {
            system_resolver,
            public_resolver,
        })
    }

    /// Check if DNS results are polluted for a domain.
    ///
    /// Compares DNS resolution from system DNS with public DNS servers
    /// to detect potential pollution.
    ///
    /// # Arguments
    ///
    /// * `domain` - The domain name to check
    ///
    /// # Returns
    ///
    /// Returns a `PollutionResult` containing the comparison details.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let checker = PollutionChecker::new()?;
    /// let result = checker.check("google.com").await?;
    /// println!("Polluted: {}", result.is_polluted);
    /// ```
    pub async fn check(&self, domain: &str) -> Result<PollutionResult> {
        // Parse domain (ensure it ends with a dot for proper resolution)
        let domain = if domain.ends_with('.') {
            domain.to_string()
        } else {
            format!("{domain}.")
        };

        // Resolve using system DNS
        let system_ips = self.resolve_with(&self.system_resolver, &domain).await?;

        // Resolve using public DNS
        let public_ips = self.resolve_with(&self.public_resolver, &domain).await?;

        // Determine if polluted
        let is_polluted = self.detect_pollution(&system_ips, &public_ips);

        let details = if is_polluted {
            format!(
                "System DNS returned: {:?}, Public DNS returned: {:?}",
                system_ips, public_ips
            )
        } else {
            format!("Both returned similar results: {:?}", public_ips)
        };

        Ok(PollutionResult {
            domain: domain.trim_end_matches('.').to_string(),
            system_ips,
            public_ips,
            is_polluted,
            details,
        })
    }

    /// Resolve domain using specified resolver.
    ///
    /// # Arguments
    ///
    /// * `resolver` - The DNS resolver to use
    /// * `domain` - The domain name to resolve
    ///
    /// # Returns
    ///
    /// Returns a vector of IP addresses.
    async fn resolve_with(
        &self,
        resolver: &TokioAsyncResolver,
        domain: &str,
    ) -> Result<Vec<IpAddr>> {
        use trust_dns_resolver::proto::rr::RecordType;

        // Try A records first (IPv4)
        let response = resolver.lookup(domain, RecordType::A).await?;
        let mut ips: Vec<IpAddr> = response
            .iter()
            .filter_map(|r| {
                if let Some(ip) = r.as_a() {
                    Some(IpAddr::V4(*ip))
                } else if let Some(ip) = r.as_aaaa() {
                    Some(IpAddr::V6(*ip))
                } else {
                    None
                }
            })
            .collect();

        // Also try AAAA records if A returned nothing
        if ips.is_empty() {
            let response = resolver.lookup(domain, RecordType::AAAA).await?;
            ips = response
                .iter()
                .filter_map(|r| {
                    if let Some(ip) = r.as_aaaa() {
                        Some(IpAddr::V6(*ip))
                    } else {
                        None
                    }
                })
                .collect();
        }

        Ok(ips)
    }

    /// Detect pollution by comparing system DNS with public DNS.
    ///
    /// Pollution is detected when:
    /// 1. System returns IP addresses that differ from public DNS results
    /// 2. System returns IP addresses that are not in public DNS results
    ///
    /// # Arguments
    ///
    /// * `system_ips` - IP addresses from system DNS
    /// * `public_ips` - IP addresses from public DNS
    ///
    /// # Returns
    ///
    /// Returns `true` if pollution is detected.
    fn detect_pollution(&self, system_ips: &[IpAddr], public_ips: &[IpAddr]) -> bool {
        if system_ips.is_empty() || public_ips.is_empty() {
            return false;
        }

        // If system returns IPs that are not in the public DNS results
        // and are not known public IPs, it might be polluted
        let public_ip_set: std::collections::HashSet<_> = public_ips.iter().collect();

        for sys_ip in system_ips {
            // Check if this IP appears in public DNS results
            if public_ip_set.contains(&sys_ip) {
                return false; // Found matching IP, not polluted
            }

            // Check if it's a known public DNS IP
            let ip_str = sys_ip.to_string();
            if PUBLIC_DNS_IPS.iter().any(|&p| p == ip_str) {
                return false;
            }
        }

        // If we get here, system returned IPs that aren't in public results
        // This is likely pollution, but we need to be careful
        // Only report as polluted if there's a clear mismatch
        !system_ips.is_empty() && !public_ips.is_empty()
    }

    /// Check multiple domains in batch.
    ///
    /// # Arguments
    ///
    /// * `domains` - List of domain names to check
    ///
    /// # Returns
    ///
    /// Returns a vector of pollution results (only successful ones).
    #[allow(dead_code)]
    pub async fn check_batch(&self, domains: &[String]) -> Vec<PollutionResult> {
        let mut results = Vec::new();
        for domain in domains {
            if let Ok(result) = self.check(domain).await {
                results.push(result);
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_google() {
        // This test requires network connection which may be unreliable in CI
        // Skip if CI environment variable is set
        if std::env::var("CI").is_ok() {
            return;
        }

        let checker = PollutionChecker::new().unwrap();
        let result = checker.check("google.com").await.unwrap();

        println!("System IPs: {:?}", result.system_ips);
        println!("Public IPs: {:?}", result.public_ips);
        println!("Polluted: {}", result.is_polluted);
    }
}

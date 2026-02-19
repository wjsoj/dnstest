//! DNS list configuration loader.
//!
//! This module provides functionality to load DNS server lists
//! from JSON files, command-line arguments, or default locations.

use crate::dns::types::{DnsList, DnsServer};
use crate::error::{Error, Result};
use std::path::Path;

/// DNS list configuration loader.
///
/// Provides various methods to load and merge DNS server lists
/// from different sources.
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load DNS list from a JSON file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let list = ConfigLoader::load_from_file("dnslist.json")?;
    /// for server in &list.servers {
    ///     println!("{}: {}", server.name, server.ip);
    /// }
    /// ```
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<DnsList> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let list: DnsList = serde_json::from_str(&content)?;
        Ok(list)
    }

    /// Load DNS list from the default location.
    ///
    /// Searches in the following order:
    /// 1. `$CONFIG_DIR/dnstest/dnslist.json`
    /// 2. `dnslist.json` in current directory
    ///
    /// # Errors
    ///
    /// Returns an error if no default file is found or cannot be parsed.
    #[allow(dead_code)]
    pub fn load_default() -> Result<DnsList> {
        Self::load_from_file(
            dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("dnstest")
                .join("dnslist.json"),
        )
        .or_else(|_| {
            // Try to load from current directory
            Self::load_from_file("dnslist.json")
        })
    }

    /// Load both IPv4 and IPv6 DNS lists from user config directory.
    ///
    /// Loads from `~/.config/dnstest/dnslist.json` and `dnslist-v6.json`.
    ///
    /// # Errors
    ///
    /// Returns an error if no DNS list files are found.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let lists = ConfigLoader::load_all()?;
    /// let merged = ConfigLoader::merge(lists);
    /// ```
    pub fn load_all() -> Result<Vec<DnsList>> {
        // Get user config directory
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dnstest");

        let mut lists = Vec::new();

        // Try to load IPv4 list from config directory
        let ipv4_path = config_dir.join("dnslist.json");
        if let Ok(list) = Self::load_from_file(&ipv4_path) {
            lists.push(list);
        }

        // Try to load IPv6 list from config directory
        let ipv6_path = config_dir.join("dnslist-v6.json");
        if let Ok(list) = Self::load_from_file(&ipv6_path) {
            lists.push(list);
        }

        if lists.is_empty() {
            return Err(Error::Config(
                "No DNS list found. Please run 'dnstest update' first.".into(),
            ));
        }

        Ok(lists)
    }

    /// Get the config directory path.
    #[must_use]
    pub fn config_dir() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dnstest")
    }

    /// Merge multiple DNS lists into one.
    ///
    /// Combines all servers from the input lists and removes duplicates
    /// based on IP address.
    ///
    /// # Arguments
    ///
    /// * `lists` - Vector of DNS lists to merge
    ///
    /// # Returns
    ///
    /// Returns a single merged DNS list with unique servers.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let lists = ConfigLoader::load_all()?;
    /// let merged = ConfigLoader::merge(lists);
    /// println!("Total servers: {}", merged.servers.len());
    /// ```
    #[must_use]
    pub fn merge(lists: Vec<DnsList>) -> DnsList {
        let mut servers = Vec::new();
        for list in lists {
            servers.extend(list.servers);
        }
        // Remove duplicates by IP
        servers.sort_by(|a, b| a.ip.cmp(&b.ip));
        servers.dedup_by(|a, b| a.ip == b.ip);
        DnsList { servers }
    }

    /// Create a custom DNS list from command-line arguments.
    ///
    /// # Arguments
    ///
    /// * `dns_servers` - Vector of strings in format "IP#Name"
    ///
    /// # Errors
    ///
    /// Returns an error if any IP address is invalid.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let args = vec!["8.8.8.8#Google".to_string(), "1.1.1.1#Cloudflare".to_string()];
    /// let list = ConfigLoader::from_args(args)?;
    /// ```
    pub fn from_args(dns_servers: Vec<String>) -> Result<DnsList> {
        let mut servers = Vec::new();
        for s in dns_servers {
            let parts: Vec<&str> = s.splitn(2, '#').collect();
            let ip = parts[0].trim().to_string();
            let name = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| ip.clone());

            // Validate IP address
            if ip.parse::<std::net::IpAddr>().is_err() {
                return Err(Error::Parse(format!("Invalid IP address: {ip}")));
            }

            servers.push(DnsServer::new(name, ip));
        }
        Ok(DnsList { servers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_server_creation() {
        let server = DnsServer::new("Test DNS", "8.8.8.8");
        assert_eq!(server.name, "Test DNS");
        assert_eq!(server.ip, "8.8.8.8");
        assert!(server.delay.is_none());
    }

    #[test]
    fn test_dns_server_ip_parse() {
        let server = DnsServer::new("Test", "8.8.8.8");
        let ip = server.ip_addr();
        assert!(ip.is_some());
        assert!(ip.unwrap().is_ipv4());

        let server_v6 = DnsServer::new("Test", "::1");
        let ip_v6 = server_v6.ip_addr();
        assert!(ip_v6.is_some());
        assert!(ip_v6.unwrap().is_ipv6());
    }

    #[test]
    fn test_dns_server_is_ipv4_ipv6() {
        let server_v4 = DnsServer::new("Test", "8.8.8.8");
        assert!(server_v4.is_ipv4());
        assert!(!server_v4.is_ipv6());

        let server_v6 = DnsServer::new("Test", "::1");
        assert!(!server_v6.is_ipv4());
        assert!(server_v6.is_ipv6());
    }

    #[test]
    fn test_dns_list() {
        let list = DnsList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);

        let servers = vec![
            DnsServer::new("Test1", "8.8.8.8"),
            DnsServer::new("Test2", "1.1.1.1"),
        ];
        let list = DnsList::from_servers(servers);
        assert!(!list.is_empty());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_config_from_args() {
        let args = vec![
            "8.8.8.8#Google".to_string(),
            "1.1.1.1#Cloudflare".to_string(),
        ];
        let list = ConfigLoader::from_args(args).unwrap();
        assert_eq!(list.servers.len(), 2);
        assert_eq!(list.servers[0].name, "Google");
        assert_eq!(list.servers[1].name, "Cloudflare");
    }

    #[test]
    fn test_config_from_args_invalid_ip() {
        let args = vec!["invalid_ip#Test".to_string()];
        let result = ConfigLoader::from_args(args);
        assert!(result.is_err());
    }
}

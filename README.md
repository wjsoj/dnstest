# dnstest

A modern DNS speed testing and pollution detection CLI tool written in Rust.

## Features

- **DNS Speed Testing**: Measure latency to DNS servers using ICMP ping
- **Pollution Detection**: Compare system DNS with public DNS to detect tampering
- **Interactive TUI**: User-friendly terminal interface
- **Multiple Formats**: Output results in table, JSON, CSV, or TSV format
- **IPv4/IPv6 Support**: Works with both address families

## Installation

### From Source

```bash
git clone https://github.com/yourname/dnstest.git
cd dnstest
cargo install --path .
```

### Prerequisites

- Rust 1.75 or later
- Root/sudo access (required for ICMP ping)

## Usage

### Interactive Mode (Default)

```bash
dnstest
```

Launches an interactive TUI with menu navigation.

### DNS Speed Test

```bash
# Test all DNS servers from default list
dnstest speed

# Sort by latency (fastest first)
dnstest speed --sort

# Use custom DNS servers
dnstest speed --dns 8.8.8.8#Google --dns 1.1.1.1#Cloudflare

# Use custom DNS list file
dnstest speed --file my-dns-list.json

# Output as JSON
dnstest speed --format json
```

### DNS Pollution Check

```bash
# Check a single domain
dnstest check google.com

# Output as JSON
dnstest check google.com --format json
```

### List DNS Servers

```bash
# List all servers
dnstest list

# IPv4 only
dnstest list --ipv4

# IPv6 only
dnstest list --ipv6

# From custom file
dnstest list --file my-dns-list.json
```

### Export DNS List

```bash
# Export to JSON
dnstest export

# Custom output path
dnstest export --output my-dns-list.json
```

## Output Formats

| Format | Description |
|--------|-------------|
| `table` | Human-readable table (default) |
| `json` | JSON array |
| `csv` | Comma-separated values |
| `tsv` | Tab-separated values |

## Configuration

### DNS List File Format

```json
{
  "list": [
    {
      "name": "Cloudflare DNS",
      "IP": "1.1.1.1",
      "delay": null,
      "status": "pending"
    },
    {
      "name": "Google Public DNS",
      "IP": "8.8.8.8",
      "delay": null,
      "status": "pending"
    }
  ]
}
```

### Default DNS List

The tool includes default DNS server lists:
- `dnslist.json` - IPv4 servers
- `dnslist-v6.json` - IPv6 servers

## Library Usage

You can also use dnstest as a library:

```rust
use dnstest::{DnsServer, SpeedTester, PollutionChecker};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test DNS speed
    let tester = SpeedTester::new()?;
    let server = DnsServer::new("Cloudflare", "1.1.1.1");
    let result = tester.test_latency(&server).await;
    
    println!("Latency: {:?} ms", result.latency_ms);
    
    // Check DNS pollution
    let checker = PollutionChecker::new()?;
    let result = checker.check("google.com").await?;
    
    println!("Polluted: {}", result.is_polluted);
    
    Ok(())
}
```

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

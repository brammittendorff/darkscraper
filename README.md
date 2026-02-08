# DarkScraper

High-performance dark web crawler and entity extractor designed for intelligence gathering across multiple anonymous networks.

## Overview

DarkScraper is a Rust-based distributed crawler that navigates and indexes content from Tor, I2P, ZeroNet, Freenet, and Lokinet. It automatically extracts and catalogs entities such as email addresses, cryptocurrency wallets, phone numbers, PGP fingerprints, and usernames, storing everything in PostgreSQL for advanced search and analysis.

## Features

- **Multi-Network Support**: Crawl across 5 anonymous networks simultaneously
  - Tor (.onion)
  - I2P (.i2p)
  - ZeroNet
  - Freenet
  - Lokinet (.loki)

- **Entity Extraction**: Automatically identify and extract:
  - Email addresses
  - Bitcoin addresses (Legacy & Bech32)
  - Ethereum addresses
  - Monero addresses
  - Phone numbers
  - PGP fingerprints
  - Usernames
  - .onion and .i2p addresses

- **High Performance**:
  - Concurrent crawling with configurable worker pools
  - Multiple proxy instances per network for load distribution
  - Bloom filter-based duplicate detection
  - Configurable depth and per-domain limits

- **Data Management**:
  - PostgreSQL storage with full-text search
  - Export to JSON
  - Grafana dashboards for monitoring
  - Advanced search by entity type or full-text query

## Architecture

The project is organized as a Rust workspace with the following crates:

- **darkscraper-core**: Core types, configuration, and error handling
- **darkscraper-networks**: Network-specific clients (Tor, I2P, ZeroNet, Freenet, Lokinet)
- **darkscraper-parser**: HTML parsing and entity extraction
- **darkscraper-storage**: PostgreSQL storage layer and migrations
- **darkscraper-frontier**: URL frontier and duplicate detection
- **darkscraper-search**: Search functionality and indexing
- **darkscraper-discovery**: Advanced discovery algorithms and metadata extraction

## Quick Start

### Prerequisites

- Docker and Docker Compose
- At least 4GB RAM
- 10GB free disk space

### Basic Setup

1. Clone the repository:
```bash
git clone <repository-url>
cd darkscraper
```

2. Start the infrastructure:
```bash
docker compose up -d
```

This launches:
- PostgreSQL database
- 3 Tor instances
- 3 I2P instances
- 3 ZeroNet instances
- 3 Freenet instances
- 3 Lokinet instances
- Grafana dashboard (http://localhost:3000)
- DarkScraper crawler

3. Monitor progress:
```bash
docker compose logs -f darkscraper
```

### Scaled Setup

For higher throughput, launch additional network instances:

```bash
# Launch with 5 instances of each network
TOR_INSTANCES=5 I2P_INSTANCES=5 ZERONET_INSTANCES=5 \
FREENET_INSTANCES=5 LOKINET_INSTANCES=5 \
docker compose --profile tor-extra --profile i2p-extra \
  --profile zeronet-extra --profile freenet-extra \
  --profile lokinet-extra up -d
```

## Configuration

Configuration is managed through `config/default.toml`:

```toml
[general]
max_depth = 10
max_pages_per_domain = 1000
max_body_size_mb = 10

[tor]
enabled = true
max_concurrency = 32
connect_timeout_seconds = 30

[extraction]
extract_emails = true
extract_crypto = true
extract_phones = true
extract_pgp = true
```

### Environment Variables

Override configuration at runtime:

| Variable | Description | Default |
|----------|-------------|---------|
| `TOR_WORKERS` | Concurrent Tor crawlers | 32 |
| `I2P_WORKERS` | Concurrent I2P crawlers | 8 |
| `ZERONET_WORKERS` | Concurrent ZeroNet crawlers | 8 |
| `FREENET_WORKERS` | Concurrent Freenet crawlers | 8 |
| `LOKINET_WORKERS` | Concurrent Lokinet crawlers | 8 |
| `TOR_ENABLED` | Enable/disable Tor crawling | true |
| `I2P_ENABLED` | Enable/disable I2P crawling | true |
| `MAX_DEPTH` | Maximum crawl depth | 10 |
| `TOR_INSTANCES` | Number of Tor proxies | 3 |
| `I2P_INSTANCES` | Number of I2P proxies | 3 |

## Usage

### CLI Commands

```bash
# Start crawling with default seeds
darkscraper crawl

# Crawl from specific seed
darkscraper crawl --seed "http://example.onion"

# Crawl from seed file or comma-separated list
darkscraper crawl --seeds "seeds.txt"

# Crawl with custom depth
darkscraper crawl --depth 5

# Search crawled data by full-text query
darkscraper search --query "bitcoin"

# Search by entity value
darkscraper search --entity "someone@example.com"

# Search by entity type
darkscraper search --entity-type "email" --limit 50

# Show crawl statistics
darkscraper status

# Export data to JSON
darkscraper export --format json --output data.json
```

### Docker Usage

```bash
# Run a one-time crawl
docker compose run --rm darkscraper crawl --seed "http://example.onion"

# Search within container
docker compose run --rm darkscraper search --query "keyword"

# View statistics
docker compose run --rm darkscraper status
```

## Monitoring

Access the Grafana dashboard at http://localhost:3000:
- Username: `admin`
- Password: `darkscraper`

Dashboards display:
- Crawl rate per network
- Entity extraction counts
- Database size and query performance
- Error rates and timeouts

## Development

### Building from Source

```bash
# Install Rust (1.93+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies (Debian/Ubuntu)
sudo apt-get install cmake pkg-config libssl-dev clang

# Build
cargo build --release

# Run
./target/release/darkscraper --help
```

### Running Tests

```bash
cargo test --workspace
```

### Project Structure

```
darkscraper/
├── src/                    # Main binary
│   ├── main.rs
│   ├── cli.rs
│   ├── crawl.rs
│   └── commands.rs
├── crates/                 # Workspace crates
│   ├── core/              # Core types & config
│   ├── networks/          # Network clients
│   ├── parser/            # HTML & entity extraction
│   ├── storage/           # Database layer
│   ├── frontier/          # URL queue & deduplication
│   ├── search/            # Search engine
│   └── discovery/         # Discovery algorithms
├── config/                # Configuration files
├── docker/                # Dockerfiles for networks
├── grafana/               # Grafana dashboards
└── docker-compose.yml
```

## Performance Tuning

### Worker Pools

Adjust worker counts based on your hardware:

```bash
# High-performance setup (16+ cores, 16GB+ RAM)
TOR_WORKERS=64 I2P_WORKERS=16 ZERONET_WORKERS=16 \
FREENET_WORKERS=16 LOKINET_WORKERS=16 \
docker compose up -d
```

### Network-Specific Tuning

- **Tor**: High concurrency works well (32-64 workers)
- **I2P**: Lower concurrency recommended (8-16 workers) due to network latency
- **ZeroNet/Freenet/Lokinet**: Moderate concurrency (8-16 workers)

### Resource Requirements

| Setup | CPU Cores | RAM | Storage |
|-------|-----------|-----|---------|
| Minimal (3 instances) | 4 | 4GB | 10GB |
| Standard (5 instances) | 8 | 8GB | 50GB |
| High-performance (5 instances) | 16+ | 16GB+ | 100GB+ |

## Security & Legal Considerations

**IMPORTANT**: This tool is intended for:
- Security research
- OSINT investigations
- Academic research
- Authorized penetration testing

Users are responsible for:
- Complying with local laws and regulations
- Obtaining proper authorization before crawling
- Respecting robots.txt and website policies
- Protecting collected data appropriately
- Not accessing illegal content

The authors assume no liability for misuse of this software.

## Troubleshooting

### I2P Won't Connect

I2P can take 5-10 minutes to bootstrap. Check logs:
```bash
docker compose logs i2p1
```

### Lokinet Failing to Start

Lokinet requires privileged mode. Ensure Docker has the necessary permissions:
```bash
docker compose logs lokinet1
```

### Database Connection Errors

Wait for PostgreSQL to be ready:
```bash
docker compose logs postgres
```

### Out of Memory

Reduce worker counts or increase Docker memory limits in Docker Desktop settings.

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [Tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [scraper](https://github.com/causal-agent/scraper) - HTML parsing
- [PostgreSQL](https://www.postgresql.org/) - Database
- [Grafana](https://grafana.com/) - Monitoring

## Contact

For questions or support, please open an issue on GitHub.

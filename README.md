# DarkScraper

High-performance dark web crawler and OSINT platform designed for intelligence gathering across multiple anonymous networks.

## Overview

DarkScraper is a Rust-based distributed crawler that navigates and indexes content from Tor, I2P, Hyphanet (formerly Freenet), and Lokinet. It automatically extracts and catalogs entities such as email addresses, cryptocurrency wallets, phone numbers, PGP fingerprints, and usernames, storing everything in PostgreSQL for advanced search and analysis.

**Note**: ZeroNet support has been disabled (Feb 2026) due to network death - zero active peers/seeders despite working infrastructure.

**Key Innovation:** Intelligent prioritization system that automatically discovers and prioritizes cryptographic addresses (base32, base64, Bitcoin addresses) over human-readable aliases, enabling deeper infrastructure mapping and correlation analysis.

## Features

- **Multi-Network Support**: Crawl across 4 anonymous networks simultaneously
  - **Tor** (.onion) - v3 onion addresses
  - **I2P** (.i2p, .b32.i2p) - Java I2P with automatic base32 discovery
  - **Hyphanet** (USK@/SSK@/CHK@) - Formerly Freenet
  - **Lokinet** (.loki) - Oxen network
  - ~~**ZeroNet** (.bit)~~ - DISABLED: Network dead (no active peers as of Feb 2026)

- **Cryptographic Address Prioritization**:
  - Automatically detects and prioritizes permanent cryptographic addresses
  - Discovers I2P base32 addresses from human-readable names
  - Maps Bitcoin addresses in ZeroNet
  - Identifies 52-char Lokinet addresses vs ONS names
  - Enables deep infrastructure correlation and tracking

- **Entity Extraction**: Automatically identify and extract:
  - Email addresses
  - Bitcoin addresses (Legacy & Bech32)
  - Ethereum addresses
  - Monero addresses
  - Phone numbers
  - PGP fingerprints
  - Usernames
  - .onion, .i2p, .b32.i2p, and .loki addresses

- **Advanced Discovery**:
  - Source mining (embedded URLs in JavaScript, comments, metadata)
  - Form spidering (search forms, hidden inputs)
  - Pattern mutation (URL structure analysis)
  - Infrastructure probing (robots.txt, sitemap.xml, common paths)
  - Correlation engine (favicon hashing, server fingerprinting)

- **High Performance**:
  - Concurrent crawling with configurable worker pools
  - Multiple proxy instances per network for load distribution
  - Priority queue with depth penalty and address type boosting
  - Bloom filter-based duplicate detection
  - Configurable depth and per-domain limits

- **Data Management**:
  - PostgreSQL storage with full-text search
  - Entity correlation and relationship mapping
  - Export to JSON
  - Grafana dashboards for real-time monitoring
  - Advanced search by entity type or full-text query

## Architecture

The project is organized as a Rust workspace with the following crates:

- **darkscraper-core**: Core types, configuration, and error handling
- **darkscraper-networks**: Network-specific clients (Tor, I2P, ZeroNet, Hyphanet, Lokinet)
- **darkscraper-parser**: HTML parsing and entity extraction
- **darkscraper-storage**: PostgreSQL storage layer and migrations
- **darkscraper-frontier**: Priority queue, URL deduplication, and cryptographic address classification
- **darkscraper-search**: Search functionality and indexing
- **darkscraper-discovery**: Advanced discovery algorithms and metadata extraction

## Quick Start

### Prerequisites

- Docker and Docker Compose
- At least 8GB RAM (recommended 16GB)
- 20GB free disk space (50GB+ for extended crawling)

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
- 3 Tor instances (SOCKS5 proxies)
- 3 Java I2P instances (HTTP proxies + router console)
- 3 Hyphanet instances
- 3 Lokinet instances (CAP_NET_ADMIN for TUN interface)
- Grafana dashboard (http://localhost:3000)
- DarkScraper crawler

3. **Wait for I2P bootstrap** (10-15 minutes for first startup):
```bash
# Check logs to monitor I2P bootstrap progress
docker compose logs i2p1 -f
```

4. Monitor crawling progress:
```bash
docker compose logs -f darkscraper
```

### Performance Scaling

Use `SCALE_LEVEL` (1-5) with `./start.sh` for automatic balanced scaling:

```bash
# Level 1: Minimal (6GB RAM, 4 cores) - ~2,000 pages/hour
SCALE_LEVEL=1 ./start.sh

# Level 2: Light (12GB RAM, 8 cores) - ~6,000 pages/hour
SCALE_LEVEL=2 ./start.sh

# Level 3: Standard [DEFAULT] (18GB RAM, 12 cores) - ~12,000 pages/hour
SCALE_LEVEL=3 ./start.sh

# Level 4: Performance (28GB RAM, 16 cores) - ~25,000 pages/hour
SCALE_LEVEL=4 ./start.sh

# Level 5: Maximum (40GB RAM, 24 cores) - ~40,000+ pages/hour
SCALE_LEVEL=5 ./start.sh
```

`start.sh` automatically generates extra instances and sets progressive timeouts.

## Network-Specific Information

### Tor
- **Bootstrap Time**: 10-20 seconds ⚡
- **Proxy Type**: SOCKS5 on port 9050
- **Address Format**: 56-character base32 `.onion` (v3 only)
- **Status**: Ready almost immediately

### I2P (Java Router)
- **Bootstrap Time**: 10-15 minutes (first start), ~3 minutes (subsequent) ⚡
- **Proxy Type**: HTTP on port 4444
- **Address Formats**:
  - Human-readable: `notbob.i2p` (addressbook)
  - Cryptographic: `[52-56 chars].b32.i2p`
- **Auto-Discovery**: Crawler automatically extracts base32 addresses from headers and HTML
- **Router Console**: Port 7657 (not exposed by default)
- **Status Check**: See `docker/i2p/README.md`

### ~~ZeroNet~~ (DISABLED)
- **Status**: Network dead as of February 2026
- **Reason**: Zero active peers/seeders despite working trackers and infrastructure
- **Note**: Code remains in codebase but network is disabled in config
- If network recovers, can be re-enabled by setting `enabled = true` in `config/default.toml`

### Hyphanet (formerly Freenet)
- **Bootstrap Time**: 1-2 minutes ⚡ (was 2-3 minutes with old build)
- **Proxy Type**: HTTP on port 8888 (FProxy)
- **Address Formats**: `USK@`, `SSK@`, `CHK@` with base64-encoded keys
- **Note**: All Hyphanet addresses are cryptographic (no human-readable aliases)
- **Timeouts**: Progressive 30s/60s/120s/180s (network is very slow, patient retry strategy)

### Lokinet
- **Bootstrap Time**: 30-60 seconds ⚡
- **Proxy Type**: SOCKS5 on port 1080
- **Address Formats**:
  - Cryptographic: 52-character `.loki` addresses
  - Human-readable: ONS names like `minecraft.loki`
- **Requirements**: `CAP_NET_ADMIN` + `/dev/net/tun` device (required for TUN interface - elevated privileges)
- **Status Check**: `dig @127.3.2.1 exit.loki`

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
request_timeout_seconds = 60

[i2p]
enabled = true
max_concurrency = 8
connect_timeout_seconds = 45
request_timeout_seconds = 90  # I2P is slower

[hyphanet]
enabled = true
max_concurrency = 8
connect_timeout_seconds = 120
request_timeout_seconds = 300  # Very slow network

[extraction]
extract_emails = true
extract_crypto = true
extract_phones = true
extract_pgp = true
```

### Environment Variables

**Simple Scaling (Recommended):**
```bash
SCALE_LEVEL=1-5  # Automatic balanced scaling (see SCALING.md)
```

**Manual Overrides (Advanced):**

| Variable | Description | Default |
|----------|-------------|---------|
| `TOR_WORKERS` | Concurrent Tor crawlers | 32 |
| `I2P_WORKERS` | Concurrent I2P crawlers | 8 |
| `HYPHANET_WORKERS` | Concurrent Hyphanet crawlers | 8 |
| `LOKINET_WORKERS` | Concurrent Lokinet crawlers | 8 |
| `TOR_INSTANCES` | Number of Tor proxies | 3 |
| `I2P_INSTANCES` | Number of I2P proxies | 3 |
| `HYPHANET_INSTANCES` | Number of Hyphanet proxies | 3 |
| `LOKINET_INSTANCES` | Number of Lokinet proxies | 3 |
| `TOR_ENABLED` | Enable/disable Tor | true |
| `I2P_ENABLED` | Enable/disable I2P | true |
| `MAX_DEPTH` | Maximum crawl depth | 10 |

## Cryptographic Address Prioritization

DarkScraper automatically prioritizes URLs based on whether they use permanent cryptographic addresses or aliasable human-readable names:

### Priority Tiers

**Tier 1 (Priority 2.0)** - Cryptographic Addresses:
- Tor: 56-char `.onion` v3 addresses
- I2P: `.b32.i2p` addresses (52 or 56+ chars)
- Hyphanet: `USK@`/`SSK@`/`CHK@` addresses
- Lokinet: 52-char `.loki` addresses

**Tier 2 (Priority 1.0)** - Human-Readable Names:
- I2P: Short `.i2p` addressbook names
- Lokinet: ONS `.loki` names

**Depth Penalty**: Priority divided by `(depth + 2)` to favor shallow URLs

### Why This Matters for OSINT

Cryptographic addresses are prioritized because they:
1. **Cannot be hijacked or changed** - Permanent identifiers
2. **Represent real infrastructure** - Actual cryptographic endpoints
3. **Enable correlation** - Track same infrastructure across networks
4. **Discover hidden sites** - Not in public addressbooks
5. **Map relationships** - See true connections between services

### Automatic Base32 Discovery (I2P)

When crawling human-readable I2P sites like `notbob.i2p`, DarkScraper automatically:
1. Checks HTTP headers for `X-I2P-DestB32`
2. Scans HTML for base32 addresses
3. Adds discovered `[hash].b32.i2p` URLs to queue with high priority
4. Builds mapping between names and cryptographic addresses

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
- Pages and links discovered
- Database size and query performance
- Error rates and timeouts
- Network-specific metrics (I2P peers, Tor circuits, etc.)

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

### Database Migrations

Migrations are in `crates/storage/migrations/`. They run automatically on startup.

To manually reset the database:
```bash
# Drop and recreate
docker compose exec postgres psql -U crawler -d darkscraper -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"

# Restart crawler (runs migrations)
docker compose restart darkscraper
```

### Project Structure

```
darkscraper/
├── src/                    # Main binary
│   ├── main.rs
│   ├── cli.rs
│   ├── crawl.rs            # Main crawler logic
│   ├── commands.rs
│   └── seeds.rs            # Default seed URLs
├── crates/                 # Workspace crates
│   ├── core/              # Core types & config
│   ├── networks/          # Network clients (Tor, I2P, etc.)
│   ├── parser/            # HTML & entity extraction
│   ├── storage/           # Database layer + migrations
│   ├── frontier/          # Priority queue & address classification
│   ├── search/            # Search engine
│   └── discovery/         # Discovery algorithms
├── config/                # Configuration files
│   ├── default.toml       # Main config
│   └── i2pd.conf          # (legacy, not used)
├── docker/                # Dockerfiles for networks
│   ├── i2p/              # Java I2P (geti2p/i2p)
│   ├── hyphanet/         # Hyphanet (formerly freenet)
│   ├── zeronet/          # ZeroNet
│   └── lokinet-socks/    # Lokinet with SOCKS proxy
├── grafana/               # Grafana dashboards
│   ├── provisioning/     # Auto-provisioning
│   └── dashboards/       # Network-specific dashboards
├── start.sh               # Simple SCALE_LEVEL startup script
└── docker-compose.yml
```

## Performance Tuning

### Quick Scaling Examples

```bash
# Start with minimal resources
SCALE_LEVEL=1 ./start.sh

# Production setup with good performance
SCALE_LEVEL=3 ./start.sh

# Maximum performance
SCALE_LEVEL=5 ./start.sh
```

`start.sh` handles instance generation and configuration automatically.

### Resource Requirements

| Setup | CPU Cores | RAM | Storage | I2P Bootstrap |
|-------|-----------|-----|---------|---------------|
| Minimal (3 instances) | 4 | 8GB | 20GB | 15 min |
| Standard (5 instances) | 8 | 16GB | 50GB | 15 min |
| High-performance (5 instances) | 16+ | 32GB+ | 100GB+ | 15 min |

## Security & Legal Considerations

**IMPORTANT**: This tool is intended for:
- Security research
- OSINT investigations
- Academic research
- Authorized penetration testing
- Intelligence gathering (with proper authorization)

Users are responsible for:
- Complying with local laws and regulations
- Obtaining proper authorization before crawling
- Respecting robots.txt and website policies
- Protecting collected data appropriately
- Not accessing illegal content
- Following ethical OSINT practices

The authors assume no liability for misuse of this software.

## Default Seeds

DarkScraper includes 56 high-quality seed URLs across active networks:

- **11 Tor seeds**: Hidden wikis, directories, search engines
- **27 I2P seeds**: notbob.i2p, identiguy.i2p, forums, directories
- **14 Hyphanet seeds**: USK indexes, wikis (updated regularly)
- **4 Lokinet seeds**: Block explorers, wikis

Seeds are in `src/seeds.rs` and optimized for maximum link discovery.

**Note**: 23 ZeroNet seeds remain in code but are not used (network disabled).

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

**Priority areas:**
- Additional network support (GNUnet, Invisible Internet Project)
- Enhanced entity extraction (new cryptocurrency formats)
- Machine learning for content classification
- Advanced correlation algorithms
- Performance optimizations

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built with:
- [Tokio](https://tokio.rs/) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [scraper](https://github.com/causal-agent/scraper) - HTML parsing
- [sqlx](https://github.com/launchbadge/sqlx) - PostgreSQL client
- [PostgreSQL](https://www.postgresql.org/) - Database
- [Grafana](https://grafana.com/) - Monitoring

Special thanks to the anonymous network communities: Tor Project, I2P (geti2p.net), Hyphanet (hyphanet.org), and Oxen (Lokinet).

## Contact

For questions or support, please open an issue on GitHub.

---

**⚠️ Reminder**: Always use this tool responsibly and ethically. OSINT work requires respect for privacy, adherence to laws, and proper authorization.

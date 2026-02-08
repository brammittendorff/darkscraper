# Java I2P Router Setup for Darkscraper

This directory contains the Docker configuration for running official Java I2P routers for darkweb OSINT crawling.

## What Changed

**Previously:** Used `i2pd` (C++ lightweight implementation)
**Now:** Using `geti2p/i2p:latest` (Official Java implementation)

### Why Java I2P?

- **Larger Network**: More active peers and better connectivity
- **Official Implementation**: Reference implementation maintained by I2P project
- **Better Compatibility**: Full protocol support and feature completeness
- **Proven Stability**: Battle-tested in production for years
- **Built-in Tools**: Web console for monitoring and configuration

## Quick Start

### 1. Build and Start I2P Routers

```bash
# Build the Java I2P images
docker compose build i2p1 i2p2 i2p3

# Start the I2P routers (this will take 10-15 minutes for first bootstrap)
docker compose up -d i2p1 i2p2 i2p3

# Watch the logs to monitor bootstrap progress
docker compose logs -f i2p1
```

### 2. Monitor Bootstrap Progress

Access the Router Console for each instance:

```bash
# Get the router console URLs (you may need to expose port 7657 in docker-compose.yml)
# Or check logs for bootstrap status
docker compose logs i2p1 | grep -i "tunnel\|peer\|reseed"
```

**Bootstrap Stages:**
1. **Initial Reseed** (2-5 minutes): Downloads network database from reseed servers
2. **Peer Discovery** (5-10 minutes): Connects to initial peers
3. **Tunnel Building** (10-15 minutes): Establishes participating tunnels
4. **Network Integration** (15-30 minutes): Full integration with stable connections

### 3. Verify I2P is Working

```bash
# Check if HTTP proxy port is listening
docker compose exec i2p1 nc -z 127.0.0.1 4444 && echo "HTTP Proxy is ready!"

# Check healthcheck status
docker compose ps i2p1 i2p2 i2p3
```

The healthcheck monitors:
- Router Console (port 7657) - Web UI accessibility
- HTTP Proxy (port 4444) - Crawling endpoint

## Configuration

### Memory Allocation

Default: **768MB per router** (JVM_XMX=768m)

Adjust in `docker-compose.yml` if needed:
```yaml
environment:
  JVM_XMX: 1024m  # Increase for better performance
```

**Memory Guidelines:**
- Minimum: 512MB (may struggle with peer count)
- Recommended: 768MB (good balance)
- High Performance: 1024MB+ (more peers, faster routing)

### Router Configuration

The `router.config` file contains optimized settings:

- **Bandwidth**: 512 KB/s inbound, 256 KB/s outbound
- **Share Percentage**: 80% (participates actively in network)
- **Tunnel Hops**: 2 hops (balance between anonymity and speed)
- **Tunnel Quantity**: 3 inbound/outbound (sufficient for crawling)
- **Max Connections**: 200 peers
- **Min Active Peers**: 10 peers

### Customization

To modify router settings:

1. Edit `router.config` in this directory
2. Rebuild containers: `docker compose build i2p1 i2p2 i2p3`
3. Restart: `docker compose up -d i2p1 i2p2 i2p3`

**Note:** First-run settings are created automatically. After initial bootstrap, you can also modify `/i2p/.i2p/router.config` directly in the volume.

## Monitoring

### Check Router Status

```bash
# View router logs
docker compose logs i2p1 --tail=100

# Check connected peers
docker compose exec i2p1 grep -i "peer" /i2p/.i2p/wrapper.log | tail -20

# Monitor network database size
docker compose exec i2p1 du -sh /i2p/.i2p/netDb
```

### Key Indicators of Healthy Router

✅ **Good Signs:**
- 30+ active peers connected
- "Tunnel building: OK" in logs
- HTTP proxy responding on port 4444
- Network status: "OK" (not "Firewalled")

⚠️ **Warning Signs:**
- Less than 10 peers after 30 minutes
- "Firewalled" status persisting
- Frequent "Tunnel build failures"
- HTTP proxy timeouts

## Troubleshooting

### Problem: Router Stuck at "Reseed in progress"

**Solution:**
```bash
# Check if reseed servers are reachable
docker compose exec i2p1 curl -I https://reseed.i2p-projekt.de/

# Try manual reseed via console (expose port 7657 first)
# Navigate to http://localhost:7657/configreseed
```

### Problem: "Firewalled" Status

**Cause:** Docker networking or host firewall blocking I2NP ports

**Solutions:**
1. Ensure UDP/TCP ports are exposed in `docker-compose.yml`
2. Check host firewall rules: `sudo iptables -L -n | grep docker`
3. Wait 30-60 minutes - status may improve as network stabilizes

**Note:** "Firewalled" status doesn't prevent crawling, but reduces routing performance.

### Problem: High Memory Usage

**Solution:**
```bash
# Reduce JVM memory allocation in docker-compose.yml
JVM_XMX: 512m

# Or reduce peer connections in router.config
router.maxConnections=100
```

### Problem: Slow Crawling / Timeouts

**Solutions:**
1. Increase request timeout in `config/default.toml`:
   ```toml
   [i2p]
   request_timeout_seconds = 120  # Increase from 90
   ```

2. Wait for full network integration (30+ minutes)

3. Check peer count:
   ```bash
   docker compose logs i2p1 | grep -i peer | tail
   ```

4. Verify tunnels are established:
   ```bash
   docker compose logs i2p1 | grep -i tunnel
   ```

### Problem: Container Keeps Restarting

**Check healthcheck failures:**
```bash
docker compose ps i2p1  # Shows health status
docker compose logs i2p1 --tail=50
```

**Common Causes:**
- Insufficient start_period (bootstrap takes 10+ minutes)
- Low memory allocation
- Port conflicts (4444, 7657 already in use)

## Performance Tuning

### For Faster Bootstrap

1. **Increase Bandwidth** in `router.config`:
   ```
   i2np.bandwidth.inboundKBytesPerSecond=1024
   i2np.bandwidth.outboundKBytesPerSecond=512
   ```

2. **Enable Floodfill** (for large deployments):
   ```
   router.floodfillParticipant=true
   ```

### For Lower Resource Usage

1. **Reduce Memory**:
   ```yaml
   JVM_XMX: 512m
   ```

2. **Limit Connections** in `router.config`:
   ```
   router.maxConnections=100
   ```

3. **Reduce Tunnels**:
   ```
   router.inboundQuantity=2
   router.outboundQuantity=2
   ```

## Network Integration Timeline

| Time | Expected State |
|------|----------------|
| 0-5 min | Reseed in progress, downloading netDb |
| 5-10 min | 5-15 peers connected, building tunnels |
| 10-15 min | 15-30 peers, tunnels established, HTTP proxy functional |
| 15-30 min | 30-50 peers, participating in network, stable crawling |
| 30-60 min | 50-100 peers, fully integrated, optimal performance |

**First crawl attempt:** Wait at least **15 minutes** after container start.

## Additional Resources

- [I2P Documentation](https://geti2p.net/en/docs)
- [Java I2P Docker Image](https://hub.docker.com/r/geti2p/i2p)
- [I2P Configuration Guide](https://geti2p.net/en/docs/api/i2pcontrol)
- [Reseed Information](https://geti2p.net/en/docs/reseed)

## HTTP Proxy Usage

The crawlers automatically use the HTTP proxies at:
- `i2p1:4444`
- `i2p2:4444`
- `i2p3:4444`

These are configured in `/config/default.toml` and load-balanced by the darkscraper application.

## Testing I2P Connectivity

```bash
# Test HTTP proxy from within darkscraper container
docker compose exec darkscraper curl -x http://i2p1:4444 http://stats.i2p/

# Expected: HTML response from I2P stats page
```

## I2P vs i2pd Comparison

| Feature | Java I2P (Current) | i2pd (Previous) |
|---------|-------------------|-----------------|
| Memory | 512-1024 MB | 64-128 MB |
| Bootstrap Time | 10-15 minutes | 3-5 minutes |
| Network Size | Larger (10k+ routers) | Smaller (2-3k routers) |
| Peer Count | 50-100 typical | 20-40 typical |
| Crawling Reliability | Higher | Moderate |
| Configuration | Web UI + Files | Files only |
| Best For | Production OSINT | Resource-constrained |

---

**Need Help?** Check container logs first: `docker compose logs i2p1 -f`

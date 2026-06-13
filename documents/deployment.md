# Deployment Guide — DigitalOcean

Production architecture, the steps to reproduce it, and the operational runbooks.
Everything below is what is actually running; pitfalls we hit along the way are
called out inline.

## Architecture

```
                    chess.socketlab.tech (DNS on DigitalOcean)
                                │ HTTPS (Let's Encrypt, terminates at LB)
                                ▼
                Regional Load Balancer (NYC3, 2 nodes, HTTP 80/443 → 3000)
                                │ health check: GET /stats
              ┌─────────────────┼─────────────────┐
              ▼                 ▼                 ▼
        Droplet (pool)    Droplet (pool)    …scale 2–8 (CPU target)
        ┌───────────┐     each runs:
        │ chess     │      - chess container (port 3000)
        │ watchtower│      - watchtower (auto-deploy)
        └───────────┘      - do-agent (pool scaling metrics)
              │                 │
              └────────┬────────┘  private VPC network (NYC3)
                       ▼
        Valkey 8 (managed, TLS)         MongoDB 8 (managed, TLS)
        live game state, pub/sub,       users, sessions, finished
        move stream, locks,             games, batched move history
        server heartbeats
```

- **All compute and databases must be in the same datacenter (NYC3).** Private
  VPC networking only works within one datacenter. We initially created the LB
  in NYC2 and droplets in NYC1 by accident — both had to be recreated.
- Game state lives in Valkey, so droplets are disposable: they can be killed,
  replaced, or scaled at any time without losing live games.

## 0. Prerequisites

- `doctl` installed and authenticated: `brew install doctl && doctl auth init`
  (DO API token with read/write from **API → Personal Access Tokens**)
- Docker Desktop
- The DO API token is also stored as the `DO_API_TOKEN` GitHub Actions secret.

## 1. Docker image

`Dockerfile` (repo root) is a 3-stage build:

1. `node:20-alpine` — builds the React frontend (Vite outputs to `static/dist/`)
2. `rust:1.88-slim` — compiles the release binary (frontend must exist first;
   it is embedded via `include_str!`)
3. `debian:bookworm-slim` — runtime, just the binary + `ca-certificates`

Pitfalls we hit (already fixed in the committed Dockerfile / Cargo.toml):

| Symptom | Cause | Fix |
|---|---|---|
| `rustc 1.85.1 is not supported by …` | deps require 1.88 | `FROM rust:1.88-slim` |
| `openssl-sys` build failure | slim image lacks headers | `apt-get install pkg-config libssl-dev` in builder stage |
| `no matching manifest for linux/amd64` on droplet | image built on Apple Silicon (arm64) | build with `--platform linux/amd64` (CI builds are amd64 natively) |
| `can't connect with TLS, the feature is not enabled` at startup | redis crate has no TLS by default; managed Valkey requires `rediss://` | `redis = { version = "1.2", features = ["tokio-rustls-comp"] }` in Cargo.toml |

Manual build + push (only needed for bootstrap; CI does this on every push to master):

```bash
doctl registry login
docker build --platform linux/amd64 \
  -t registry.digitalocean.com/chess-registry/chess:latest .
docker push registry.digitalocean.com/chess-registry/chess:latest
```

## 2. Managed databases (NYC3)

| Cluster | Engine | Plan |
|---|---|---|
| `db-mdb-nyc3-*` | MongoDB 8 | Premium AMD NVMe, 2 GB RAM, storage autoscaling at 80% |
| `db-vk-nyc3-*` | Valkey 8 | Regular SSD, 2 GB RAM |

For each cluster:

- Use the **VPC network (private)** connection URI in app config — free,
  faster, never leaves the datacenter.
- **Settings → Trusted Sources**: add the droplets (ideally by tag/pool, so
  scaled-out droplets are automatically allowed). Until this is done, all
  connections are rejected and the app panics at startup.

## 3. Load Balancer (NYC3)

- Regional / **NYC3** / HTTP / External / 2 nodes (~20k concurrent connections;
  resizable without recreation)
- Forwarding rules: `HTTP 80 → HTTP 3000`, later `HTTPS 443 → HTTP 3000`
  (after the certificate exists, step 7)
- Health check: `GET /stats` port 3000 (returns `{"games":N}`)
- Sticky sessions: **OFF** (Redis pub/sub handles cross-server delivery)
- The LB's public IP is static for its lifetime — no reserved IP exists or is
  needed for LBs. Only destroying the LB changes the IP.

## 4. Golden droplet → snapshot

The autoscale pool boots droplets from a snapshot, so we prepare one droplet
("golden") with everything installed, then image it.

On a fresh NYC3 Ubuntu droplet (or one from the pool):

```bash
apt-get update -y && apt-get install -y docker.io
systemctl enable --now docker

curl -sL https://github.com/digitalocean/doctl/releases/download/v1.119.0/doctl-1.119.0-linux-amd64.tar.gz | tar xz
mv doctl /usr/local/bin/
doctl auth init --access-token <DO_API_TOKEN>
doctl registry login
```

### chess container via systemd (per-droplet SERVER_ID)

`SERVER_ID` must be unique per droplet (the heartbeat/failover system depends
on it) and stable across reboots. It is resolved at boot from the DO metadata
service — `169.254.169.254` is a link-local address answered by the
hypervisor; it returns the identity of the droplet making the request.
A snapshot-cloned droplet therefore gets **its own** ID, not the golden
droplet's.

`/usr/local/bin/chess-start.sh`:

```bash
#!/bin/bash
SERVER_ID=$(curl -s http://169.254.169.254/metadata/v1/id)
docker rm -f chess 2>/dev/null
docker run -d --name chess --restart always -p 3000:3000 \
  -e MONGODB_URI="<private mongo uri>" \
  -e MONGODB_DB="chess" \
  -e REDIS_URL="<private valkey uri (rediss://)>" \
  -e SERVER_ID="$SERVER_ID" \
  -e GOOGLE_CLIENT_ID="<client id>" \
  -e PORT="3000" \
  registry.digitalocean.com/chess-registry/chess:latest
```

`/etc/systemd/system/chess.service`:

```ini
[Unit]
Description=Chess server container
After=docker.service network-online.target
Requires=docker.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/chess-start.sh
RemainAfterExit=true

[Install]
WantedBy=multi-user.target
```

```bash
chmod +x /usr/local/bin/chess-start.sh
systemctl daemon-reload
systemctl enable chess.service
systemctl start chess.service
curl -s localhost:3000/stats   # {"games":0}
```

### Watchtower (auto-deploy agent)

```bash
docker run -d --name watchtower --restart always \
  -e DOCKER_API_VERSION=1.44 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /root/.docker/config.json:/config.json \
  containrrr/watchtower --interval 300 --cleanup chess
```

- `DOCKER_API_VERSION=1.44` is **required**: Watchtower 1.7.1 pins an API
  version too old for current Docker daemons (`client version 1.25 is too
  old`). If Watchtower breaks again, the maintained fork is
  `nickfedor/watchtower` (drop-in replacement).
- `--interval 300` polls the registry every 5 min; `--cleanup` removes old
  images; the trailing `chess` limits it to that container.
- The registry credentials mount (`config.json`) is what lets it pull from the
  private DO registry.

### Monitoring agent

Autoscale pools **require** `do-agent` baked into the image (the pool scales on
its CPU metrics and it cannot be added later). Droplets created by a pool from
DO stock images already have it; verify with `systemctl status do-agent`. If
missing: `curl -sSL https://repos.insights.digitalocean.com/install.sh | bash`.

### Pre-snapshot checklist

```bash
docker ps                                            # chess + watchtower Up
curl -s localhost:3000/stats                         # {"games":0}
systemctl is-enabled chess.service do-agent docker   # all enabled
docker logs watchtower --tail 5                      # Scanned=1, no API errors
```

Then: **Droplet → Snapshots → Take Live Snapshot** → `chess-v1`.

## 5. Autoscale pool

Create **after** the snapshot completes (destroying a pool scrubs its droplets,
including a golden one that came from it).

- Region **NYC3**, image **My Snapshots → chess-v1**
- 1 vCPU / 2 GB droplets, min 2 / max 8, CPU target ~70–80%, cooldown 5 min
- Attach the NYC3 load balancer

New droplets boot in ~30s: Docker auto-starts both containers
(`--restart always`), `chess.service` re-creates the chess container with the
droplet's own `SERVER_ID`, Watchtower self-updates it to `:latest` within 5
minutes if the snapshot's image is stale.

## 6. CI/CD

`.github/workflows/deploy.yml`: on push to `master`, GitHub Actions builds the
image (natively amd64) and pushes `:latest` to the DO registry. Watchtower on
every droplet picks it up within 5 minutes. Required secret: `DO_API_TOKEN`
(repo → Settings → Secrets and variables → Actions).

```
git push origin master
  → Actions: build + push (~5 min)
  → Watchtower on each droplet: pull + restart (≤5 min)
  → verify rollout via /version (below)
```

### /version endpoint

`GET /version` returns `{"version":"1.0.0","server_id":"<droplet id>"}`.
Bump the `VERSION` const in `src/service/game_service.rs` with each release.
Because the LB spreads requests, repeated calls show which droplets have
rolled over:

```bash
for i in {1..6}; do curl -s https://chess.socketlab.tech/version; echo; done
```

## 7. DNS + HTTPS

Domain `socketlab.tech` is registered at get.tech; DNS is delegated to
DigitalOcean (required for DO-managed Let's Encrypt certificates).

1. get.tech → domain → Name Servers → `ns1/ns2/ns3.digitalocean.com`
2. DO **Networking → Domains** → add `socketlab.tech`
3. A record: `chess` → select the load balancer from the dropdown
4. Wait for NS propagation (`dig +short NS socketlab.tech` must show DO
   nameservers — cert creation fails with *"a non DigitalOcean Name Server was
   found"* until then; just retry later)
5. LB → Settings → Forwarding rules → `HTTPS 443 → HTTP 3000` → New
   certificate → **Let's Encrypt** → `chess.socketlab.tech` (auto-renews)
6. Enable **Redirect HTTP to HTTPS**
7. Google Cloud Console → OAuth client → add `https://chess.socketlab.tech`
   to Authorized JavaScript origins (Sign-In fails on the domain otherwise)

TLS terminates at the LB; droplets speak plain HTTP 3000 inside the VPC.

## Runbooks

### Deploy a release

```bash
# bump VERSION in src/service/game_service.rs, then:
git push origin master
# ~10 min later, confirm every server_id reports the new version:
for i in {1..8}; do curl -s https://chess.socketlab.tech/version; echo; done
```

### Change MONGODB_URI / REDIS_URL (or any baked env var)

Two parts — running fleet, then snapshot. Skipping part 2 means the next
scale-out boots a droplet with the old config and crash-loops.

1. On **each** droplet (one at a time; LB routes around the ~5s restart):
   ```bash
   vim /usr/local/bin/chess-start.sh    # update the value
   systemctl restart chess.service
   curl -s localhost:3000/stats
   ```
2. Take a new snapshot from an updated droplet (`chess-v2`), then point the
   pool at it: pool → Droplet Configuration → Edit (or
   `doctl compute droplet-autoscale update <pool-id> --droplet-image <id>`).
   Existing droplets keep running (already fixed in step 1); future
   scale-outs use the new snapshot.

If config churn becomes frequent, move the env file to a private Spaces bucket
fetched by `chess-start.sh` at boot, with GitHub Actions rendering it from
GitHub Secrets — then config changes never require snapshot rebuilds.

### Verify the stack end to end

```bash
curl -s https://chess.socketlab.tech/stats     # {"games":N}
curl -s https://chess.socketlab.tech/version   # version + which droplet answered
```

Open the site in two browsers, create + join a game, play moves — with ≥2
droplets the players often land on different servers, exercising the
cross-server Redis pub/sub path.

### A droplet looks unhealthy

```bash
ssh root@<droplet-ip>
docker ps -a                 # chess Up? watchtower Up?
docker logs chess --tail 50  # startup panics name the failing dependency:
                             #  - "failed to connect to Redis" → REDIS_URL / trusted sources / TLS
                             #  - "failed to set up MongoDB client" → MONGODB_URI / trusted sources
systemctl restart chess.service
```

Or just destroy the droplet — the pool replaces it from the snapshot, and no
game state is lost (it's all in Valkey).

### Common failure modes seen during bring-up

| Symptom | Cause → fix |
|---|---|
| App panics: Redis TLS `InvalidClientConfig` | redis crate built without TLS → `tokio-rustls-comp` feature (fixed in Cargo.toml) |
| App panics on DB connect in prod only | cluster Trusted Sources doesn't include the droplet |
| `no matching manifest for linux/amd64` | image built on Apple Silicon without `--platform linux/amd64` |
| Watchtower: `client version 1.25 is too old` | add `-e DOCKER_API_VERSION=1.44` to the watchtower container |
| Watchtower logs `Scanned=0` | the chess container isn't running — fix chess first; Watchtower only updates running containers |
| LE cert: "non DigitalOcean Name Server found" | NS delegation not propagated yet — verify with `dig +short NS`, retry |
| Private DB URI unreachable | resource in a different datacenter — everything must be in NYC3 |
| Pool won't scale | `do-agent` missing from the snapshot image |

# Deploying to the server

Every push to `main` builds the Docker image, pushes it to the GitHub
Container Registry (`ghcr.io/gbrzeczek/sheetsolver`), then deploys it
(`.github/workflows/docker.yml`, jobs `image` and `deploy`):

1. The runner joins the tailnet for the length of the job, tagged
   `tag:ci`, through a Tailscale OAuth client.
2. It connects to the server over SSH, on its tailnet name, so SSH need
   not be open to the internet.
3. It copies `deploy/docker-compose.yml` to the deploy directory and
   runs `docker compose pull && docker compose up -d` there. The pull
   uses the job's own GitHub token, which expires with the job; the
   server keeps no registry login.

The `deploy` job is skipped until the repository variable `DEPLOY_HOST`
is set, so the image is built and pushed before any of this is ready.

## One-time setup

### On the server

- Docker with the Compose plugin, and Tailscale, logged in to your
  tailnet.
- A user for deploys (e.g. `deploy`) that may run Docker (`docker`
  group) and owns the deploy directory (default `/srv/sheetsolver`).
- The external `web` network your reverse proxy uses:
  `docker network create web`, unless the proxy created it. Point the
  proxy at `http://sheetsolver:8080`.
- A key pair for the job, its public half allowed to log in as that
  user:

  ```bash
  ssh-keygen -t ed25519 -N '' -C github-deploy -f deploy_key
  # append deploy_key.pub to ~deploy/.ssh/authorized_keys on the server
  ```

### In Tailscale (admin console)

- In the access controls, let `tag:ci` exist and reach the server's SSH
  port, e.g.:

  ```json
  "tagOwners": { "tag:ci": ["autogroup:admin"] },
  "grants": [
    { "src": ["tag:ci"], "dst": ["<server's tailnet name or tag>"], "ip": ["tcp:22"] }
  ]
  ```

- Settings → OAuth clients → generate a client with the `auth_keys`
  write scope and the tag `tag:ci`. Note its id and secret. Runners join
  as ephemeral nodes and disappear after the job.

### In GitHub (repository → Settings → Secrets and variables → Actions)

| Kind | Name | Value |
|---|---|---|
| Secret | `TS_OAUTH_CLIENT_ID` | The Tailscale OAuth client's id. |
| Secret | `TS_OAUTH_SECRET` | Its secret. |
| Secret | `DEPLOY_SSH_KEY` | The private key `deploy_key`, whole. |
| Secret | `DEPLOY_KNOWN_HOSTS` | The server's host key line, from `ssh-keyscan <tailnet name>` run on a machine in the tailnet, so the job knows it talks to the real server. |
| Variable | `DEPLOY_HOST` | The server's tailnet name (MagicDNS, e.g. `myserver`, or `myserver.tailnet-name.ts.net`). Setting it turns the deploy on. |
| Variable | `DEPLOY_USER` | The deploy user, e.g. `deploy`. |
| Variable | `DEPLOY_DIR` | Optional; default `/srv/sheetsolver`. |

The image is private on GHCR at first; the job's token can pull it, so
it may stay private. To pull it by hand on the server, use
`docker login ghcr.io` with a token that can read packages.

## Cloudflare

The domain is served through Cloudflare (free plan). The server limits
each visitor to 2 jobs at a time, and reads who the visitor is from the
`CF-Connecting-IP` header Cloudflare sets (`SHEETSOLVER_CLIENT_IP_HEADER`
in `deploy/docker-compose.yml`; `docs/API.md`, *Limits*). Two settings
make that hold:

### Reachable only through Cloudflare

Anyone who finds the server's address could otherwise skip Cloudflare,
and send any `CF-Connecting-IP` to escape the limit. Either:

- **A Cloudflare Tunnel** (recommended, free): `cloudflared` on the
  server connects out to Cloudflare, so no port needs to be open to the
  internet. In the Cloudflare dashboard, Zero Trust → Networks →
  Tunnels → create a tunnel of type *Cloudflared*, and copy its token.
  Run it next to the app, on the `web` network:

  ```yaml
  # in the reverse proxy's compose file, or a compose file of its own
  services:
    cloudflared:
      image: cloudflare/cloudflared:latest
      command: tunnel --no-autoupdate run
      environment:
        TUNNEL_TOKEN: "<the tunnel's token>"
      restart: unless-stopped
      networks: [web]
  networks:
    web:
      external: true
  ```

  In the tunnel's *Public hostnames*, route the domain to the reverse
  proxy (or straight to `http://sheetsolver:8080`; Cloudflare then
  handles TLS). Then remove the domain's old `A`/`AAAA` records pointing
  at the server (the tunnel adds its own `CNAME`), and close ports 80
  and 443 in the server's firewall.
- **Or a firewall** that lets in ports 80 and 443 only from Cloudflare's
  addresses (<https://www.cloudflare.com/ips/>), kept up to date. Mind
  that Docker's published ports skip `ufw`'s rules; filter in the
  `DOCKER-USER` chain or at the host provider's firewall.

Check: `curl -sk https://<server's public IP>/openapi.json -H 'Host:
<domain>'` must fail.

### A rate-limiting rule

The free plan allows one rule, over 10 seconds. It turns away floods
before they reach the server; the server's own limit is what keeps one
visitor from taking every solver. Security → WAF → *Rate limiting
rules* → create:

- When incoming requests match: *URI Path* equals `/jobs` **and**
  *Request Method* equals `POST`.
- With the same characteristics: *IP*.
- When rate exceeds: 10 requests per 10 seconds.
- Then: *Block*, for 10 seconds.

A woodworker sends one job at a time, so this never touches them.

Leave the rest at Cloudflare's defaults: its DDoS protection is always
on. *Bot Fight Mode* is not needed and can block scripts that use the
API. Event streams (`GET /jobs/{id}/events`) pass through: Cloudflare
closes a connection idle for 100 s, and the server sends a keep-alive
every 15 s.

## By hand

The same as the job, on the server:

```bash
cd /srv/sheetsolver
docker compose pull && docker compose up -d
docker compose logs -f sheetsolver
```

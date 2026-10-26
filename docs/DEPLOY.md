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

## By hand

The same as the job, on the server:

```bash
cd /srv/sheetsolver
docker compose pull && docker compose up -d
docker compose logs -f sheetsolver
```

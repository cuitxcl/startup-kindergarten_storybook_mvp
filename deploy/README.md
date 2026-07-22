# Kindleaf Deploy Notes

This directory contains minimal deployment templates for trial environments.

## HTTPS Reverse Proxy

`Caddyfile.example` is the recommended first trial reverse proxy template:

- Caddy manages HTTPS certificates automatically.
- `/api/*` is proxied to the Loco API on `127.0.0.1:8080`.
- All other routes are proxied to the frontend service on `127.0.0.1:5173`.
- Security headers are enabled for the public site.

Before using it:

1. Replace `kindleaf.example.com` with the real trial domain.
2. Point DNS to the deployment host.
3. Set `APP_HOST=https://<real-domain>` in `.env.local`.
4. If using the Compose frontend image behind the same domain, build with `COMPOSE_VITE_API_BASE_URL=/api`.
5. Run `./scripts/check-smart.sh trial-strict`.
6. After real DeepSeek + Seedream/ARK keys are configured, run `./scripts/check-smart.sh real-required`.

Example:

```sh
COMPOSE_APP_HOST=https://kindleaf.example.com \
COMPOSE_VITE_API_BASE_URL=/api \
docker compose --profile app up --build api frontend

caddy run --config deploy/Caddyfile.example
```

This is not a full production deployment plan. Production still needs external monitoring, log shipping, automated backups, object storage, release rollback, and incident runbooks.

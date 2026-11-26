# Traefik reverse proxy for brega (TLS termination)

An alternative to the bundled Caddy service for operators already running
[Traefik](https://doc.traefik.io/traefik/). Traefik terminates HTTPS and routes
to the `brega-web` container, which serves the console and proxies the API to
`brega-server`.

## 1. A Traefik instance with an ACME resolver

Minimal static config (`traefik.yml`):

```yaml
entryPoints:
  web:
    address: ":80"
    http:
      redirections:
        entryPoint:
          to: websecure
          scheme: https
  websecure:
    address: ":443"

certificatesResolvers:
  letsencrypt:
    acme:
      email: you@example.com
      storage: /letsencrypt/acme.json
      httpChallenge:
        entryPoint: web

providers:
  docker:
    exposedByDefault: false
```

## 2. Labels on the `brega-web` service

Add these labels to the `brega-web` service in `docker-compose.yml` (and join
the Traefik network). Replace the host rule with your domain:

```yaml
  brega-web:
    # ...existing config...
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.brega.rule=Host(`brega.example.com`)"
      - "traefik.http.routers.brega.entrypoints=websecure"
      - "traefik.http.routers.brega.tls.certresolver=letsencrypt"
      - "traefik.http.services.brega.loadbalancer.server.port=80"
```

With Traefik doing TLS you do not need the bundled `caddy` service — start the
stack **without** `--profile tls` so it only runs `brega-server` and `brega-web`,
and let Traefik route to `brega-web`.

# Reverse proxy / TLS samples

`brega-server` speaks plain HTTP (`BREGA_BIND`); the `brega-web` container serves
the console and proxies the API to the server. Put one of these in front to
terminate HTTPS for a public deployment. All three route the same way — TLS in,
plain HTTP to `brega-web` — so pick whichever you already run.

| Sample                       | Best for                              | ACME (auto-TLS) |
| ---------------------------- | ------------------------------------- | --------------- |
| [`Caddyfile`](Caddyfile)     | Simplest; bundled in `docker-compose` | Yes, built in   |
| [`nginx.conf`](nginx.conf)   | Hosts already running nginx           | Via certbot     |
| [`traefik.md`](traefik.md)   | Docker/Traefik-label setups           | Via resolver    |

## Caddy (recommended)

Already wired into `docker-compose.yml`. Set `BREGA_DOMAIN` and
`BREGA_ACME_EMAIL` in `.env`, point DNS at the host, then:

```sh
docker compose --profile tls up -d
```

Caddy fetches and renews a Let's Encrypt certificate automatically and serves
`https://$BREGA_DOMAIN`.

See [`docs/self-hosting.md`](../../docs/self-hosting.md) for the full guide.

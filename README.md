# tg2fa - Vaultwarden + Telegram 2FA bridge

**tg2fa** is a small service that acts as glue between Vaultwarden and Telegram, so Telegram can be used as the second factor. It receives
login requests, sends an approval prompt to Telegram, and returns the one-time code only after the request is approved.

## Licensing

Dual-licensed under the **MIT** and **Apache License 2.0**. See [LICENSE](LICENSE) file.

## Vaultwarden modifications

**TODO**

## Intended deployment

This service is meant to run with the provided `compose.yaml`.

- `tg2fa:8080` handles **Telegram webhooks**
- `tg2fa:9090` is the **internal auth API**
- **tg2fa** itself does **not** do TLS termination
- TLS should be terminated by **Traefik**

Important:

The included compose setup follows that model: `traefik` is the public entrypoint, 
while `tg2fa`, `vaultwarden`, and `valkey` stay on the internal Docker network.

## Service configuration

| Variable                    | Required | Default                  | Meaning                                               |
|-----------------------------|----------|--------------------------|-------------------------------------------------------|
| `TELEGRAM_BOT_TOKEN`        | yes      | -                        | Telegram bot token                                    |
| `TELEGRAM_CHAT`             | yes      | -                        | Telegram user/chat id; message the bot once to get it |
| `TELEGRAM_HOST_URL`         | yes      | -                        | Public HTTPS webhook URL served through Traefik       |
| `VALKEY_SERVER`             | no       | `redis://127.0.0.1:6379` | Valkey / Redis connection URL                         |
| `CONFIG_REQUEST_EXPIRATION` | no       | `5m`                     | Pending request lifetime                              |
| `BIND_TELEGRAM`             | no       | `0.0.0.0:8080`           | Telegram webhook bind address                         |
| `BIND_AUTH`                 | no       | `0.0.0.0:9090`           | Internal auth API bind address                        |
| `GEOIP_CITY`                | no       | `/opt/geoip/city.mmdb`   | MaxMind city database path                            |
| `GEOIP_ASN`                 | no       | `/opt/geoip/asn.mmdb`    | MaxMind ASN database path                             |
| `LOGLEVEL`                  | no       | `info`                   | Log level                                             |

## Before the first start

1. Create a Telegram bot with BotFather and get the token.
2. Prepare a public HTTPS URL that points to Traefik and then to tg2fa's webhook route.

## MaxMind GeoLite2 setup (optional)

GeoIP is optional, but if you want country/city/ASN details in Telegram messages, place the MaxMind Lite databases into `.data/geoip/`.

1. Create a free MaxMind account.
2. Generate a license key.
3. Download **GeoLite2 City** and **GeoLite2 ASN**.

Copy files:

```bash
cp GeoLite2-City.mmdb .data/geoip/city.mmdb
cp GeoLite2-ASN.mmdb .data/geoip/asn.mmdb
```

Notes:

- You can skip this step, the service still works; location fields will just be empty / unavailable.

## Running with compose

The compose file includes:

- `traefik`
- `vaulwarden`
- `tg2fa`
- `valkey`

Create the compose interpolation file:

```bash
cat > .env <<'EOF'
DOMAIN=your-domain.example
EMAIL=admin@your-domain.example
EOF
```

Create runtime files and directories:

```bash
mkdir -p .data/acme .data/valkey .data/vaultwarden .data/geoip
touch .data/acme/acme.json .data/vaultwarden.env
chmod 600 .data/acme/acme.json
```

Create the **tg2fa** env file:

```bash
cat > .data/tg2fa.env <<'EOF'
TELEGRAM_BOT_TOKEN=<bot-token>
TELEGRAM_CHAT=<telegram-user-id>
EOF
```

Notes:

- `TELEGRAM_HOST_URL` is set by `compose.yaml` from `DOMAIN`
- do **not** put `TELEGRAM_HOST_URL` into `.data/tg2fa.env` unless you also change the compose file
- `.data/vaultwarden.env` is for your regular Vaultwarden settings such as `ADMIN_TOKEN`, SMTP settings, signups, and similar options

Then start the stack:

```bash
docker compose up -d --build
```

Compose behavior:

- Traefik publishes `80` and `443`
- Vaultwarden is exposed through Traefik on `https://${DOMAIN}/`
- **tg2fa** webhook is exposed through Traefik on `https://${DOMAIN}/_tg2fa/`

## Traefik and exposed ports

- **tg2fa** serves plain HTTP only; it expects Traefik to handle HTTPS
- if you keep the provided route, Telegram should call `https://example.com/_tg2fa/`
- do not publish `9090` with a `ports:` mapping
- only the Traefik entrypoint should be reachable from outside
- Traefik in the compose file is configured to terminate TLS with Let's Encrypt and store ACME data in `.data/acme/acme.json`.

## API summary

- `POST /request` - submit a login request and send it to Telegram
- `POST /validate` - validate the approved code

There is a small request example in [test.http](test.http) file.

## References

-- https://github.com/dani-garcia/vaultwarden
-- https://github.com/traefik/traefik

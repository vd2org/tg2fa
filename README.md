# tg2fa - Vaultwarden + Telegram 2FA bridge

**tg2fa** is a small service that acts as glue between Vaultwarden and Telegram, so Telegram can be used as the second factor.
It receives login requests, sends an approval prompt to the use in Telegram's chat, and validates the user response.

## Licensing

Dual-licensed under the **MIT** and **Apache License 2.0**. See [LICENSE](LICENSE) file.

## Configuration

| Environment variable        | Required | Default                  | Meaning                                         |
|-----------------------------|----------|--------------------------|-------------------------------------------------|
| `TELEGRAM_BOT_TOKEN`        | yes      | -                        | Telegram bot token                              |
| `TELEGRAM_HOST_URL`         | yes      | -                        | Public HTTPS webhook URL served through Traefik |
| `VALKEY_SERVER`             | no       | `redis://127.0.0.1:6379` | Valkey / Redis connection URL                   |
| `CONFIG_REQUEST_EXPIRATION` | no       | `5m`                     | Pending request lifetime                        |
| `BIND_TELEGRAM`             | no       | `0.0.0.0:8080`           | Telegram webhook bind address                   |
| `BIND_AUTH`                 | no       | `0.0.0.0:9090`           | Internal auth API bind address                  |
| `GEOIP_CITY`                | no       | `/opt/geoip/city.mmdb`   | MaxMind city database path                      |
| `GEOIP_ASN`                 | no       | `/opt/geoip/asn.mmdb`    | MaxMind ASN database path                       |
| `LOGLEVEL`                  | no       | `info`                   | Log level                                       |

## API summary

- `POST /request` - submit a login request and send it to Telegram
- `POST /validate` - validate the approved code

There is a small request example in the [test.http](test.http) file.

## Used ports

- `8080` - Telegram webhook bind address. Must be exposed to the internet.
- `9090` - Internal auth API bind address. Must never be exposed outside the internal network.

## Security

The endpoint used for receive webhooks from Telegram is protected by `X-Telegram-Bot-Api-Secret-Token`,
so basically no additional protection is needed.

## Vaultwarden

**TODO**

# Deployment

The **tg2fa** service intended to be deployed using `docker compose` with TLS termination done by Traefik or similar software. 
Here are examples for two cases: simple host deployment and deploying using Cloudflare Tunnel.

All examples assume that you have Docker installed and running on your server.

**NOTE**: Examples don't cover additional security-related topics, such as firewall setup, `fail2ban` and so on. 
Administrators must address these topics by themselves.

## Before the first start

Create a Telegram bot using [BotFather](https://t.me/BotFather) and get the token.

## Host deployment

This method is useful when you have a dedicated server/vps. `80` and `443` ports must be open,
and A/AAAA records of your domain should point to the server's IP.

Traefik will do TLS termination automatically. Certificates will be obtained from Let's encrypt using `tlsChallage`.

Create environment files using provided templates:

```shell
cp docker/host/env.template docker/host/.env
cp docker/host/tg2fa.env.template docker/host/tg2fa.env
cp docker/host/vaultwarden.env.template docker/host/vaultwarden.env
```

Fill templates with appropriate values.

Run the whole service using docker:

```shell
docker compose -f docker/host/compose.yaml up -d
```

Verify that all services are running:

```shell
docker compose -f docker/host/compose.yaml ps
```

You should see the following services up and running:

- `traefik`
- `tg2fa`
- `vaultwarden`
- `valkey`

Use the following command to observe logs:

```shell
docker compose -f docker/host/compose.yaml logs -f
```

Databases and other data are stored in `docker/host/.data/` directory.

To verify if your bot is working as expected, write anything to it. The bot should reply to you with your `user_id`.

## Deploying using Cloudflare Tunnel

This service is useful if you cannot or don't want to expose the service directly. 
Instead, Cloudflare Tunnel will be used to expose your service.
For example, you can set up vaultwarden on your home Raspberry Pi computer.

Please set up Cloudflare Tunnel first.

Create environment files using provided templates:

```shell
cp docker/cloudflare-tunnel/env.template docker/cloudflare-tunnel/.env
cp docker/cloudflare-tunnel/tg2fa.env.template docker/cloudflare-tunnel/tg2fa.env
cp docker/cloudflare-tunnel/vaultwarden.env.template docker/cloudflare-tunnel/vaultwarden.env
cp docker/cloudflare-tunnel/cloudflared.env.template docker/cloudflare-tunnel/cloudflared.env
```

Fill templates with appropriate values.

Run the whole service using docker:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml up -d
```

Verify that all services are running:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml ps
```

You should see the following services up and running:

- `cloudflared`
- `traefik`
- `tg2fa`
- `vaultwarden`
- `valkey`

Use the following command to observe logs:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml logs -f
```

Databases and other data are stored in `docker/cloudflare-tunnel/.data/` directory.

To verify if your bot is working as expected, write anything to it. The bot should reply to you with your `user_id`.

## MaxMind GeoLite2 setup (optional)

GeoIP is optional, but if you want country/city/ASN details in Telegram messages, 
place the MaxMind Lite databases into `.data/geoip/`.

1. Create a free MaxMind account.
2. Generate a license key.
3. Download **GeoLite2 City** and **GeoLite2 ASN**.

Copy files:

```bash
cp GeoLite2-City.mmdb .data/geoip/city.mmdb
cp GeoLite2-ASN.mmdb .data/geoip/asn.mmdb
```

# References

- https://github.com/dani-garcia/vaultwarden
- https://github.com/traefik/traefik
- https://valkey.io/docs/
- https://developers.cloudflare.com/tunnel/setup/
- https://core.telegram.org/bots/api
- https://letsencrypt.com
- https://dev.maxmind.com/geoip/geolite2-free-geolocation-data/

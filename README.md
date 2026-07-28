# tg2fa - Vaultwarden + Telegram 2fa bridge

**tg2fa** is a small service that acts as glue between Vaultwarden and Telegram, allowing Telegram to be used as the second factor.
It receives login requests, sends an approval prompt to the user in Telegram chat, and validates the user's response.

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

## Ports

- `8080` - Telegram webhook bind address. It must be exposed to the internet through Traefik or similar software.
- `9090` - Internal auth API bind address. Must never be exposed outside the internal network.

## Security

The endpoint used to receive Telegram webhooks is protected by `X-Telegram-Bot-Api-Secret-Token`,
so no additional protection is typically needed.

## Vaultwarden

This setup requires a modified Vaultwarden build. Source changes are available [here](https://github.com/vd2org/vaultwarden).

Prebuilt Docker images are available [here](https://github.com/users/vd2org/packages/container/package/vaultwardenext2fa).
The deployment examples below use this image.

```shell
docker pull ghcr.io/vd2org/vaultwardenext2fa:1.37.0-alpine
```

Configure Vaultwarden with the following settings:

| Environment variable | Type    | Default                  | Meaning                           |
|----------------------|---------|--------------------------|-----------------------------------|
| `ENABLE_EXT2FA`      | bool    | `false`                  | Enable External 2fa support       |
| `EXT2FA_URL`         | string  | `http://127.0.0.1:9090/` | URL of the 2fa server, e.g. tg2fa |

These settings can be applied either through environment variables or through the admin panel.

To allow Vaultwarden to call internal addresses such as **tg2fa**, set `HTTP_REQUEST_BLOCK_NON_GLOBAL_IPS` to `false`.

### Database setup

There is currently no GUI support for this, so you must enable **External 2fa** in the database for each user.

**Note:** If you use `sqlite`, stop the Vaultwarden instance first to avoid database corruption.

List users:

```sqlite
SELECT uuid,email,name FROM users;
```

```text
10126234-85ba-4700-bd65-89f37e607462|spock@example.com|Spock
```

Generate UUID values for the new `twofactor` rows:

```shell
python3 -c "import uuid; [print(uuid.uuid4()) for _ in range(10)]"
```

Insert External 2fa entries:

**Note:** The example below assumes that `1234567890` is your Telegram `user_id`.

```sqlite
INSERT INTO twofactor 
    (uuid, user_uuid, atype, enabled, data, last_used) 
VALUES 
    ('caf6bf5a-63dd-4dd4-8bb3-a526f749327d', '10126234-85ba-4700-bd65-89f37e607462', 900, 1, '{"id": 1234567890}', 0)
```

### Client behavior

Vaultwarden clients do not support a custom 2fa mechanism, so **External 2fa** is implemented by reusing the YubiKey flow.
This means you cannot use both **YubiKey** and **External 2fa** at the same time. If YubiKey is configured,
it will be ignored and the **External 2fa** code will be required to log in.

**TBD**: Registration API and Web Vault support.

# Deployment

The **tg2fa** service is intended to be deployed using `docker compose`, with TLS termination handled by Traefik or similar software.
Below are examples of two cases: direct host deployment and deployment through Cloudflare Tunnel.

All examples assume that you have Docker installed and running on your server.

**Note:** These examples do not cover additional security topics such as firewall setup or `fail2ban`.
Administrators must address those separately.

## Before the first start

Create a Telegram bot using [BotFather](https://t.me/BotFather) and get the token.

## Host deployment

This method is useful when you have a dedicated server or VPS. Ports `80` and `443` must be open,
and A/AAAA records of your domain should point to the server's IP.

Traefik handles TLS termination automatically. Certificates are obtained from Let's Encrypt using the `tlsChallenge` ACME challenge.

Create environment files using provided templates:

```shell
cp docker/host/env.template docker/host/.env
cp docker/host/tg2fa.env.template docker/host/tg2fa.env
cp docker/host/vaultwarden.env.template docker/host/vaultwarden.env
```

Fill templates with appropriate values.

Start the full stack with Docker:

```shell
docker compose -f docker/host/compose.yaml up -d
```

Verify that all services are running:

```shell
docker compose -f docker/host/compose.yaml ps
```

You should see the following services running:

- `traefik`
- `tg2fa`
- `vaultwarden`
- `valkey`

Use the following command to view logs:

```shell
docker compose -f docker/host/compose.yaml logs -f
```

Databases and other data are stored in the `docker/host/.data/` directory.

To verify that your bot is working as expected, send it any message. The bot should reply with your `user_id`.

## Deploying using Cloudflare Tunnel

This method is useful if you cannot or do not want to expose the service directly.
Instead, Cloudflare Tunnel is used to expose it.
For example, you can run Vaultwarden on a home Raspberry Pi.

Set up Cloudflare Tunnel first.

Create environment files using provided templates:

```shell
cp docker/cloudflare-tunnel/env.template docker/cloudflare-tunnel/.env
cp docker/cloudflare-tunnel/tg2fa.env.template docker/cloudflare-tunnel/tg2fa.env
cp docker/cloudflare-tunnel/vaultwarden.env.template docker/cloudflare-tunnel/vaultwarden.env
cp docker/cloudflare-tunnel/cloudflared.env.template docker/cloudflare-tunnel/cloudflared.env
```

Fill templates with appropriate values.

Start the full stack with Docker:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml up -d
```

Verify that all services are running:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml ps
```

You should see the following services running:

- `cloudflared`
- `traefik`
- `tg2fa`
- `vaultwarden`
- `valkey`

Use the following command to view logs:

```shell
docker compose -f docker/cloudflare-tunnel/compose.yaml logs -f
```

Databases and other data are stored in the `docker/cloudflare-tunnel/.data/` directory.

To verify that your bot is working as expected, send it any message. The bot should reply with your `user_id`.

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

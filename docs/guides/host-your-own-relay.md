# Host your own push relay

Whoosh gets "new mail" notifications on the iPhone through a small relay server. The relay is open source
(`relay/`), has no built-in address, and runs anywhere that runs a Linux program or a Docker image. You decide
where. Whoosh sends nothing to any relay until you type an address into Settings.

Nothing in this guide was run on a hosting service by the people who wrote it: the steps follow each
platform's documented flow. If one step differs on your platform, the environment table in `relay/README.md`
is the part that does not change.

The `relay/` folder builds alone: it has its own `Cargo.lock` and reads nothing outside itself, so you can copy just that
folder (or `git subtree split --prefix=relay`) to the host and build it with `cargo build --release --locked`. For Docker, the
build context is `relay/` only.

## What the relay holds

- Holds: an Apple push device token, an opaque routing id per account (a random string that says nothing
  about the account), and for tracked sent mail an opaque message token and the time of each open or click.
- Never holds: a mailbox password, an OAuth token, message text, a subject, a recipient address.
- You can read this from the database: `sqlite3 relay.sqlite3 .schema`.

## What you need

1. A machine or service that can run a program or a container and keep a small disk (SQLite file).
2. A public https address for it, for example `https://relay.example.com`. The relay speaks plain HTTP, so
   the https comes from your platform's edge or a reverse proxy.
3. An Apple Developer account for the iPhone app: its bundle id, your team id, and an APNs key (below).
   Without these the relay still runs and `GET /v1/health` says `degraded`; push waits in a queue.

## Make the APNs key

1. In your Apple Developer account, open Certificates, Identifiers and Profiles. Turn on Push Notifications
   for the app's identifier.
2. Under Keys, create a key with Apple Push Notifications service enabled. Download the `.p8` file once;
   Apple does not offer it again. Note the key id.
3. Note your team id (Membership) and the app's bundle id.
4. Keep the `.p8` out of the repository. You pass it to the relay as a file or as an environment secret.

## Option A: any Linux host (a VPS)

```
git clone <your fork or the public repository> && cd whoosh/relay
cargo build --release --locked
sudo install -m 0755 target/release/relay /usr/local/bin/relay
sudo useradd --system --home /var/lib/relay --create-home relay
sudo install -m 0600 -o relay /path/to/AuthKey_XXXXXXXXXX.p8 /var/lib/relay/apns.p8
```

`/etc/relay.env` (mode 0600; start from `relay/.env.example`):

```
RELAY_BIND=127.0.0.1:8080
RELAY_DB=/var/lib/relay/relay.sqlite3
RELAY_PUBLIC_BASE_URL=https://relay.example.com
RELAY_APNS_KEY_PATH=/var/lib/relay/apns.p8
RELAY_APNS_KEY_ID=XXXXXXXXXX
RELAY_APNS_TEAM_ID=YYYYYYYYYY
RELAY_APNS_TOPIC=com.example.whoosh
RELAY_TRACKING_KEY=<a long random string>
RELAY_BINDING_KEY=<another long random string>
```

`/etc/systemd/system/relay.service`:

```
[Unit]
Description=Whoosh relay
After=network-online.target

[Service]
User=relay
EnvironmentFile=/etc/relay.env
ExecStart=/usr/local/bin/relay
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

HTTPS with Caddy (`/etc/caddy/Caddyfile`):

```
relay.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Then `sudo systemctl enable --now relay caddy`.

## Option B: Docker anywhere

```
docker build -t whoosh-relay relay/
docker run -d --name relay --restart unless-stopped -p 127.0.0.1:8080:8080 \
  -v relay-data:/data --env-file relay/.env whoosh-relay
```

Use `RELAY_APNS_KEY` (the key text inline) in `.env` instead of a file path, or mount the key and set
`RELAY_APNS_KEY_PATH`. Put a reverse proxy (Caddy, nginx, Traefik) in front for https.

## Option C: Fly.io

From `relay/` (the Dockerfile is used as is):

```
fly launch --no-deploy --name <your-app-name>
fly volumes create relay_data --size 1
fly secrets set RELAY_APNS_KEY="$(cat AuthKey_XXXXXXXXXX.p8)" RELAY_APNS_KEY_ID=XXXXXXXXXX \
  RELAY_APNS_TEAM_ID=YYYYYYYYYY RELAY_APNS_TOPIC=com.example.whoosh \
  RELAY_TRACKING_KEY=<random> RELAY_BINDING_KEY=<random> RELAY_PUBLIC_BASE_URL=https://<your-app-name>.fly.dev
```

In `fly.toml` set the service port to 8080 and mount the volume at `/data`:

```
[http_service]
  internal_port = 8080
  force_https = true

[mounts]
  source = "relay_data"
  destination = "/data"
```

Run one machine only: the database is a single SQLite file. `fly deploy`.

## Option D: Render

Create a Web Service from the repository, runtime Docker, root directory `relay` (the Dockerfile and build context are that folder only). Add a persistent disk
mounted at `/data`. Set `PORT=8080` and the variables from the table in `relay/README.md`
(`RELAY_APNS_KEY` for the key text). Render serves https for you. Run one instance.

## Point Whoosh at it

In Whoosh, Settings, set the push relay address to `https://relay.example.com`. Leave it empty to keep push
off. Whoosh accepts plain `http` only for a relay on the same machine.

## Check it works

```
curl -s https://relay.example.com/v1/health
```

You should see `"status":"ok"`, the version, `"queue_depth":0` and `"apns":{"state":"ready"}`. `degraded`
with `"apns":{"state":"unconfigured"}` means the key variables are missing; the message names what is wrong.

## Keep it healthy

- Back up the SQLite file (`RELAY_DB`) if you want registrations to survive a lost disk; if they are lost,
  each device registers again the next time Whoosh opens.
- Keep `RELAY_TRACKING_KEY` and `RELAY_BINDING_KEY` stable. Changing them breaks tracked links already sent
  and provider bindings.
- One instance per database. To run more than one, move the store to a shared database first; that is not
  built.

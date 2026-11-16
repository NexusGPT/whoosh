# Whoosh relay

A small server that lets the Whoosh iPhone app get a push notification when new mail arrives. It holds
device tokens and opaque routing ids and nothing else: no mailbox password, no message content, no
recipient address. It also serves the sent-mail tracking pixel and redirect (opt-in per message).

The relay is host-agnostic. It has no built-in hostname, account or key. You run it where you like,
configure it with environment variables, and type its address into Whoosh (Settings, push relay address).
Without an address Whoosh sends no registration anywhere and push stays off.

- Build and run it on any Linux host, in Docker, or on Fly, Render or a VPS: [docs/guides/host-your-own-relay.md](../docs/guides/host-your-own-relay.md).
- The API and what each route stores: [docs/lanes/relay-core.md](../docs/lanes/relay-core.md). `GET /v1/health` describes the running build; `GET /` answers the same 200 and body, because Render, Fly and Nexus probe the root path.

## It builds alone

`relay/` is a complete Cargo project: its own `[workspace]`, its own `Cargo.lock`, and no file read from outside the folder. Copy
or split out just this folder (`git subtree split --prefix=relay`) and `cargo build --release --locked` works there; the Docker
build context is `relay/` only. Its numbers are in `src/constants.rs`, generated from the repository's `docs/CONSTANTS.md` by
`tools/gen-constants.py` (do not edit it by hand; a split copy keeps the file as it was at the split).
`python3 checks/relay-agnostic-check.py` (repository root) fails if a source file, a path dependency or the Dockerfile reaches outside `relay/`.
`cargo test` is the exception: some tests read fixtures and `docs/CONSTANTS.md` from the full repository and need it.

## Run it

```
cargo build --release --locked          # Rust stable; needs a C compiler and cmake
cp .env.example .env                    # fill in your own values
set -a; . ./.env; set +a
./target/release/relay                  # prints: listening on http://ADDR
curl -s http://127.0.0.1:8080/v1/health
```

Docker (context is this folder only): `docker build -t whoosh-relay . && docker run -p 8080:8080 -v relay-data:/data --env-file .env whoosh-relay`.

The relay speaks plain HTTP. Put HTTPS in front of it (your platform's edge, Caddy, nginx). The app accepts
`http` only for a relay on the same machine.

## Configuration (environment)

| Variable | Needed | Meaning |
|---|---|---|
| `RELAY_BIND` | no | Address to listen on. Unset: `127.0.0.1` on a free port; if `PORT` is set: `0.0.0.0:$PORT`. The Docker image sets `0.0.0.0:8080`. |
| `RELAY_DB` | no | SQLite file. Default `relay.sqlite3` in the working directory; the image uses `/data/relay.sqlite3`. |
| `RELAY_PUBLIC_BASE_URL` | for tracking | The public https address of this relay, put in the tracking pixel and link addresses. Default is the listen address. |
| `RELAY_APNS_KEY_PATH` | for push | Path to your APNs `.p8` key. |
| `RELAY_APNS_KEY` | for push | The same key inline (PEM; `\n` allowed for newlines), for hosts that only offer environment secrets. The path wins when both are set. |
| `RELAY_APNS_KEY_ID`, `RELAY_APNS_TEAM_ID`, `RELAY_APNS_TOPIC` | with a key | Key id, Apple team id and the iPhone app's bundle id. All three are required once a key is given. |
| `RELAY_APNS_SANDBOX_URL`, `RELAY_APNS_PRODUCTION_URL` | no | Default to Apple's APNs hosts; override only to test against a stand-in. |
| `RELAY_TRACKING_KEY` | on a hosted relay | Signs tracking links. Unset: random per start, so links die on restart (`/v1/health` shows `ephemeral`). |
| `RELAY_BINDING_KEY` | on a hosted relay | Keeps provider bindings valid across restarts. Unset: random per start. |
| `RELAY_SIGNAL_TOKEN` | no | Turns on `POST /v1/inbound/loopback` for a trusted caller (bearer token). Unset: the route does not exist. |
| `RELAY_GMAIL_JWKS_PATH`, `RELAY_GMAIL_AUDIENCE`, `RELAY_GMAIL_PUSH_ACCOUNT` | for Gmail | How the Gmail adapter checks Google's signed Pub/Sub push. |
| `RELAY_GRAPH_RENEW_MARGIN_MIN` | no | Minutes before expiry at which a Graph subscription renews; the default is in `docs/CONSTANTS.md`. |
| `RUST_LOG` | no | Log filter, for example `info`. |

Without an APNs key the relay still starts. `/v1/health` then reports `degraded` and push jobs wait in the queue.

## Tests

`cargo test -p relay -j 2` runs against a local APNs stand-in (`relay apns-stub`) on 127.0.0.1; nothing contacts Apple.
`python3 checks/relay-core-check.py` and `python3 checks/relay-agnostic-check.py` (repository root) drive and audit it.

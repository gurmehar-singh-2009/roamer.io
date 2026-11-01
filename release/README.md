# Roamer.io release

Everything needed to host Roamer.io, ready to run on any x86-64 Linux machine.

| Path | What it is |
| --- | --- |
| `roamer.sh` | Sets everything up and starts both services |
| `bin/roamer-server` | The game server (static binary, no dependencies) |
| `bin/roamer-client` | Serves the web client (static binary) |
| `web/` | The built web client |
| `config/config.toml` | Game settings (map, players, animals) |

## Start

```bash
./roamer.sh
```

That's it. On the first run it:

1. Writes `roamer.env` with the default settings.
2. Makes a certificate for the game connection (`data/`), which browsers trust through its hash; it's renewed automatically.
3. Starts the game server and the web client, and keeps them running (a crashed one is restarted).
4. Sets them to start on boot: a systemd user service when systemd is available, otherwise a `@reboot` cron entry.

Then open **http://localhost:8081**.

It needs `openssl` (for the certificate) and nothing else.

## Commands

| Command | Does |
| --- | --- |
| `./roamer.sh` | Set up and start (in the background, and on boot) |
| `./roamer.sh stop` | Stop both |
| `./roamer.sh restart` | Restart (after changing `roamer.env`) |
| `./roamer.sh status` | Running? Where to play? |
| `./roamer.sh logs` | Follow the logs |
| `./roamer.sh run` | Run in this terminal instead (Ctrl+C stops) |
| `./roamer.sh uninstall` | Stop and remove the start-on-boot entry |

Set `ROAMER_SERVICE=none` to start in the background without systemd or cron.

## Ports

Everything listens on all interfaces, IPv6 and IPv4 (`[::]:PORT`). Ports 8080 and 3000 are not used and can't be picked.

| Setting | Default | Protocol | Open it in the firewall? |
| --- | --- | --- | --- |
| `CLIENT_PORT` | 8081 | TCP | Yes: the web page |
| `GAME_PORT` | 8082 | UDP | Yes: the game (WebTransport) |
| `POW_PORT` | 8083 | TCP | No: the web client passes challenges on to it |

## Playing from other machines

Browsers only allow the game's connection on a secure page, and `http://` is only secure on `localhost`. So for other players, serve the page over HTTPS in one of two ways:

- Set `TLS_CERT` and `TLS_KEY` in `roamer.env` to a certificate for your domain (for example from Let's Encrypt), then `./roamer.sh restart`. Players open `https://your.domain:8081`.
- Or put `CLIENT_PORT` behind an HTTPS reverse proxy (Caddy, nginx, your host's proxy).

The game port needs nothing extra: its self-signed certificate is trusted through its hash. If you'd rather use a CA-signed certificate there too, set `GAME_CERT` and `GAME_KEY`.

Set `PUBLIC_HOST` only if players reach the game server at a different address than the web page.

## Files it creates

- `roamer.env`: your settings
- `data/`: the game certificate, accounts (`accounts.json`)
- `logs/`: `server.log`, `client.log`, `roamer.log`
- `web/config.json`: tells the web page which ports and certificate to use

## Rebuilding

From the repository root:

```bash
./build_release.sh
```

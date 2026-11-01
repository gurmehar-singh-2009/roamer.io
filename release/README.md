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
2. Starts the game server and the web client, and keeps them running (a crashed one is restarted).
3. Sets them to start on boot: a systemd user service when systemd is available, otherwise a `@reboot` cron entry.

Then open **http://localhost:8081**.

It needs nothing else installed.

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
| `CLIENT_PORT` | 8081 | TCP | Yes: the web page and the game connection |
| `GAME_PORT` | 8082 | TCP | No: the web client passes the game's WebSocket on to it |
| `POW_PORT` | 8083 | TCP | No: the web client passes challenges on to it |

## Playing from other machines

The page, its challenge (`/challenge`) and the game's WebSocket (`/ws`) all come from `CLIENT_PORT`, so players only need to reach that one port. Plain `http://your.server:8081` works.

For HTTPS on a domain, use either:

- An HTTPS reverse proxy in front of `CLIENT_PORT`. With Caddy that's all of it (WebSockets pass through on their own):
  ```
  your.domain {
      reverse_proxy localhost:8081
  }
  ```
- Or set `TLS_CERT` and `TLS_KEY` in `roamer.env` to a certificate for your domain (for example from Let's Encrypt), then `./roamer.sh restart`. Players open `https://your.domain:8081`.

The domain must point at this machine (an A or AAAA record) for its certificate to be issued.

## Files it creates

- `roamer.env`: your settings
- `data/`: accounts (`accounts.json`)
- `logs/`: `server.log`, `client.log`, `roamer.log`

## Rebuilding

From the repository root:

```bash
./build_release.sh
```

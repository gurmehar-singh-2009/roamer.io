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

`roamer.sh` binds both services to `[::]:PORT` (every interface, IPv6 and IPv4). Ports 8080 and 3000 are not used and can't be picked.

| Setting | Default | Binds | What |
| --- | --- | --- | --- |
| `CLIENT_PORT` | 8081 | `[::]:8081` | The web page, plus the game's WebSocket at `/ws` |
| `GAME_PORT` | 8082 | `[::]:8082` | The game server's WebSocket |

Everything is plain HTTP and WebSockets, with no certificates. The proof-of-work challenge travels inside the game's WebSocket.

## Playing from other machines

Open `http://SERVER-IP:8081`. The page and the game both come through `CLIENT_PORT`, so that's the only port players need to reach. `./roamer.sh status` lists the addresses, and says whether the page is being served.

If HTTPS is handled in front of the game by something else (a proxy that maps a domain to a port), set `GAME_URL` in `roamer.env` to where browsers should connect, for example `GAME_URL=wss://game.example.com` for a domain forwarded to `GAME_PORT`. Leave it blank to use `/ws` on the page's own address.

## Hack Club Nest

Nest's proxy handles HTTPS for you. In the Nest dashboard, under **Domains**, add a subdomain and point it at your VM's address and `CLIENT_PORT`, for example `10.60.x.x:8081` (`ip -4 addr` shows the address). Leave `GAME_URL` blank: the game connects through `/ws` on that same domain.

## Files it creates

- `roamer.env`: your settings
- `data/`: accounts (`accounts.json`)
- `logs/`: `server.log`, `client.log`, `roamer.log`

## Rebuilding

From the repository root:

```bash
./build_release.sh
```

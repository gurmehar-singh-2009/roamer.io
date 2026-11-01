#!/usr/bin/env bash
# Roamer.io: sets everything up and starts the game server and the web client.
#
#   ./roamer.sh            set up, then start both in the background (and on boot)
#   ./roamer.sh stop       stop both
#   ./roamer.sh restart    stop, then start again
#   ./roamer.sh status     are they running? where to play
#   ./roamer.sh logs       follow the logs
#   ./roamer.sh run        run both in this terminal (Ctrl+C stops them)
#   ./roamer.sh uninstall  stop and remove the start-on-boot entry
#
# Settings are in roamer.env (made on first run). Set ROAMER_SERVICE=none to
# skip systemd / start-on-boot and just run in the background.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$DIR/roamer.env"
DATA="$DIR/data"
LOGS="$DIR/logs"
PID_FILE="$DATA/roamer.pid"
UNIT=roamer.service
UNIT_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$UNIT"
CRON_TAG="# roamer.io autostart"

# Ports that must not be used.
RESERVED_PORTS=(8080 3000)

say() { printf '\033[1m%s\033[0m\n' "$*"; }
die() {
    printf '\033[31merror:\033[0m %s\n' "$*" >&2
    exit 1
}

# --------------------------------------------------------------- settings --

write_default_env() {
    cat >"$ENV_FILE" <<'EOF'
# Roamer.io settings. Run ./roamer.sh restart after changing them.
# Ports 8080 and 3000 can't be used.

# The web page (TCP). Players open http(s)://HOST:CLIENT_PORT
CLIENT_PORT=8081
# The game itself (UDP, WebTransport)
GAME_PORT=8082
# Proof-of-work challenges (TCP). The web client passes requests on to it,
# so it doesn't need to be reachable from outside.
POW_PORT=8083

# Host name or IP the game server is reached at. Blank = the one the player
# used to open the page (right for almost everyone).
PUBLIC_HOST=

# HTTPS for the web page. Browsers only allow the game's connection on a
# secure page, and http:// is only secure on localhost. To play from other
# machines, either set these to a certificate for your domain (PEM files),
# or put CLIENT_PORT behind an HTTPS reverse proxy (Caddy, nginx...).
TLS_CERT=
TLS_KEY=

# A CA-signed certificate for the game port (PEM). Blank = a self-signed one
# is made, renewed automatically, and trusted by the page through its hash.
GAME_CERT=
GAME_KEY=

# 1 = developer chat commands (/xp, /res, /tp, /event...)
ROAMER_DEV=0
EOF
}

load_env() {
    [[ -f "$ENV_FILE" ]] || {
        write_default_env
        say "Made $ENV_FILE with default settings."
    }
    # shellcheck disable=SC1090
    set -a
    source "$ENV_FILE"
    set +a
    CLIENT_PORT="${CLIENT_PORT:-8081}"
    GAME_PORT="${GAME_PORT:-8082}"
    POW_PORT="${POW_PORT:-8083}"
    ROAMER_DEV="${ROAMER_DEV:-0}"
}

check_port() {
    local name="$1" port="$2"
    [[ "$port" =~ ^[0-9]+$ ]] && ((port >= 1 && port <= 65535)) || die "$name=$port is not a port (1-65535)"
    for r in "${RESERVED_PORTS[@]}"; do
        ((port != r)) || die "$name=$port: port $r can't be used, pick another in $ENV_FILE"
    done
}

check_settings() {
    check_port CLIENT_PORT "$CLIENT_PORT"
    check_port GAME_PORT "$GAME_PORT"
    check_port POW_PORT "$POW_PORT"
    ((CLIENT_PORT != POW_PORT)) || die "CLIENT_PORT and POW_PORT must differ"
    [[ -x "$DIR/bin/roamer-server" && -x "$DIR/bin/roamer-client" ]] || die "missing binaries in $DIR/bin (run build_release.sh)"
    [[ -f "$DIR/web/index.html" ]] || die "missing the web client in $DIR/web (run build_release.sh)"
    if [[ -n "${TLS_CERT}${TLS_KEY}" ]]; then
        [[ -r "$TLS_CERT" && -r "$TLS_KEY" ]] || die "TLS_CERT / TLS_KEY must both point to readable PEM files"
    fi
    if [[ -n "${GAME_CERT}${GAME_KEY}" ]]; then
        [[ -r "$GAME_CERT" && -r "$GAME_KEY" ]] || die "GAME_CERT / GAME_KEY must both point to readable PEM files"
    fi
}

# Fails if something else already listens on our ports.
check_ports_free() {
    command -v ss >/dev/null 2>&1 || return 0
    local busy
    for p in "$CLIENT_PORT" "$POW_PORT"; do
        busy=$(ss -Htln "sport = :$p" 2>/dev/null || true)
        [[ -z "$busy" ]] || die "TCP port $p is already in use"
    done
    busy=$(ss -Huln "sport = :$GAME_PORT" 2>/dev/null || true)
    [[ -z "$busy" ]] || die "UDP port $GAME_PORT is already in use"
}

# ------------------------------------------------------------ certificate --

CERT="$DATA/game-cert.pem"
KEY="$DATA/game-key.pem"

# Self-signed game certificates must be ECDSA and valid for under 14 days to
# be trusted by hash, so they're made for 13 days and renewed with 3 left.
cert_needs_renewal() {
    [[ -f "$CERT" && -f "$KEY" ]] || return 0
    ! openssl x509 -in "$CERT" -noout -checkend $((3 * 86400)) >/dev/null 2>&1
}

make_cert() {
    command -v openssl >/dev/null 2>&1 || die "openssl is needed to make the game certificate (or set GAME_CERT / GAME_KEY)"
    local san="DNS:localhost,IP:127.0.0.1,IP:::1"
    [[ -n "${PUBLIC_HOST:-}" ]] && {
        if [[ "$PUBLIC_HOST" =~ ^[0-9.:]+$ ]]; then san="$san,IP:$PUBLIC_HOST"; else san="$san,DNS:$PUBLIC_HOST"; fi
    }
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
        -keyout "$KEY" -out "$CERT" -days 13 -subj "/CN=roamer.io" \
        -addext "subjectAltName=$san" >/dev/null 2>&1 || die "openssl couldn't make the game certificate"
    chmod 600 "$KEY"
}

# Picks the game certificate; prints the SHA-256 hash to trust (blank for a
# CA-signed one).
game_cert() {
    if [[ -n "${GAME_CERT:-}" ]]; then
        GAME_CERT_FILE="$GAME_CERT"
        GAME_KEY_FILE="$GAME_KEY"
        CERT_HASH=""
        return
    fi
    if cert_needs_renewal; then make_cert; fi
    GAME_CERT_FILE="$CERT"
    GAME_KEY_FILE="$KEY"
    CERT_HASH=$(openssl x509 -in "$CERT" -outform der | openssl dgst -sha256 -binary | base64)
}

# The web client's /config.json: where the game is and which certificate to
# trust.
write_client_config() {
    local hashes="[]" host=""
    [[ -n "$CERT_HASH" ]] && hashes="[\"$CERT_HASH\"]"
    [[ -n "${PUBLIC_HOST:-}" ]] && host="\"host\": \"$PUBLIC_HOST\", "
    printf '{ %s"gamePort": %s, "powPort": %s, "powPath": "/challenge", "certHashes": %s }\n' \
        "$host" "$GAME_PORT" "$POW_PORT" "$hashes" >"$DIR/web/config.json.tmp"
    mv "$DIR/web/config.json.tmp" "$DIR/web/config.json"
}

# -------------------------------------------------------------------- run --

SERVER_PID=""
CLIENT_PID=""

start_server() {
    (
        cd "$DATA"
        GAME_PORT="$GAME_PORT" POW_PORT="$POW_PORT" CLIENT_ORIGIN="*" \
            ROAMER_CONFIG="$DIR/config/config.toml" CERT_FILE="$GAME_CERT_FILE" KEY_FILE="$GAME_KEY_FILE" \
            LOG_FILE="$LOGS/server.log" ACCOUNTS_FILE="$DATA/accounts.json" ROAMER_DEV="$ROAMER_DEV" \
            exec "$DIR/bin/roamer-server"
    ) >>"$LOGS/server.out" 2>&1 &
    SERVER_PID=$!
}

start_client() {
    CLIENT_PORT="$CLIENT_PORT" WEB_ROOT="$DIR/web" POW_ADDR="127.0.0.1:$POW_PORT" \
        TLS_CERT="${TLS_CERT:-}" TLS_KEY="${TLS_KEY:-}" \
        "$DIR/bin/roamer-client" >>"$LOGS/client.log" 2>&1 &
    CLIENT_PID=$!
}

stop_children() {
    [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
    [[ -n "$CLIENT_PID" ]] && kill "$CLIENT_PID" 2>/dev/null || true
    wait 2>/dev/null || true
}

# Runs both in the foreground: restarts whichever stops, and renews the
# self-signed certificate (restarting the server) before it runs out.
cmd_run() {
    load_env
    check_settings
    mkdir -p "$DATA" "$LOGS"
    check_ports_free
    game_cert
    write_client_config
    trap 'stop_children; exit 0' INT TERM
    start_server
    start_client
    say "Roamer.io is running: game on UDP $GAME_PORT, web page on TCP $CLIENT_PORT (logs in $LOGS)"

    local ticks=0
    while true; do
        sleep 5
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            echo "$(date '+%F %T') server stopped, restarting" >>"$LOGS/roamer.log"
            start_server
        fi
        if ! kill -0 "$CLIENT_PID" 2>/dev/null; then
            echo "$(date '+%F %T') web client stopped, restarting" >>"$LOGS/roamer.log"
            start_client
        fi
        ticks=$((ticks + 1))
        # Hourly certificate check.
        if ((ticks % 720 == 0)) && [[ -z "${GAME_CERT:-}" ]] && cert_needs_renewal; then
            echo "$(date '+%F %T') renewing the game certificate" >>"$LOGS/roamer.log"
            game_cert
            write_client_config
            kill "$SERVER_PID" 2>/dev/null || true
            wait "$SERVER_PID" 2>/dev/null || true
            start_server
        fi
    done
}

# ----------------------------------------------------------- background --

use_systemd() {
    [[ "${ROAMER_SERVICE:-auto}" != "none" ]] || return 1
    command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1
}

install_unit() {
    mkdir -p "$(dirname "$UNIT_FILE")"
    cat >"$UNIT_FILE" <<EOF
[Unit]
Description=Roamer.io (game server and web client)
After=network-online.target

[Service]
Type=simple
WorkingDirectory=$DIR
ExecStart=$DIR/roamer.sh run
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
EOF
    systemctl --user daemon-reload
}

running_pid() {
    [[ -f "$PID_FILE" ]] || return 1
    local pid
    pid=$(cat "$PID_FILE")
    kill -0 "$pid" 2>/dev/null && echo "$pid"
}

cmd_start() {
    load_env
    check_settings
    mkdir -p "$DATA" "$LOGS"
    if use_systemd; then
        install_unit
        systemctl --user enable --now "$UNIT" >/dev/null
        # Keep running after logout and start on boot (needs permission on
        # some systems; harmless if refused).
        loginctl enable-linger "$USER" >/dev/null 2>&1 || true
        sleep 2
        systemctl --user is-active --quiet "$UNIT" || {
            journalctl --user -u "$UNIT" -n 20 --no-pager 2>/dev/null || true
            die "the service didn't start (see above, and $LOGS)"
        }
        say "Started as a systemd user service ($UNIT); it starts again on boot."
    else
        if running_pid >/dev/null; then
            say "Already running."
        else
            check_ports_free
            nohup setsid "$DIR/roamer.sh" run >>"$LOGS/roamer.log" 2>&1 </dev/null &
            echo $! >"$PID_FILE"
            sleep 2
            running_pid >/dev/null || die "it didn't start, see $LOGS/roamer.log"
            # Start on boot through cron when there's no systemd.
            if [[ "${ROAMER_SERVICE:-auto}" != "none" ]] && command -v crontab >/dev/null 2>&1; then
                (crontab -l 2>/dev/null | grep -v "$CRON_TAG"
                    echo "@reboot $DIR/roamer.sh start $CRON_TAG") | crontab - 2>/dev/null || true
            fi
            say "Started in the background."
        fi
    fi
    cmd_status
}

cmd_stop() {
    if [[ -f "$UNIT_FILE" ]] && command -v systemctl >/dev/null 2>&1; then
        systemctl --user stop "$UNIT" 2>/dev/null || true
    fi
    local pid
    if pid=$(running_pid); then
        kill -- "-$pid" 2>/dev/null || kill "$pid" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    say "Stopped."
}

cmd_uninstall() {
    cmd_stop
    if [[ -f "$UNIT_FILE" ]]; then
        systemctl --user disable "$UNIT" >/dev/null 2>&1 || true
        rm -f "$UNIT_FILE"
        systemctl --user daemon-reload 2>/dev/null || true
    fi
    if command -v crontab >/dev/null 2>&1; then
        crontab -l 2>/dev/null | grep -v "$CRON_TAG" | crontab - 2>/dev/null || true
    fi
    say "Removed the start-on-boot entry. Your settings and data are kept."
}

cmd_status() {
    load_env
    local running=no
    if [[ -f "$UNIT_FILE" ]] && systemctl --user is-active --quiet "$UNIT" 2>/dev/null; then
        running="yes (systemd user service $UNIT)"
    elif running_pid >/dev/null; then
        running="yes (pid $(cat "$PID_FILE"))"
    fi
    local scheme=http host="${PUBLIC_HOST:-localhost}"
    [[ -n "${TLS_CERT:-}" ]] && scheme=https
    say "Roamer.io"
    echo "  running:   $running"
    echo "  play at:   $scheme://$host:$CLIENT_PORT"
    echo "  ports:     web TCP $CLIENT_PORT, game UDP $GAME_PORT, challenge TCP $POW_PORT (local)"
    echo "  logs:      $LOGS"
    if [[ "$scheme" == http && "$host" != localhost && "$host" != 127.0.0.1 ]]; then
        echo "  note:      players on other machines need HTTPS (set TLS_CERT / TLS_KEY in roamer.env)"
    fi
}

cmd_logs() {
    mkdir -p "$LOGS"
    touch "$LOGS/server.log" "$LOGS/client.log" "$LOGS/roamer.log"
    tail -F "$LOGS/server.log" "$LOGS/client.log" "$LOGS/roamer.log"
}

case "${1:-start}" in
start) cmd_start ;;
stop) cmd_stop ;;
restart)
    cmd_stop
    sleep 1
    cmd_start
    ;;
status) cmd_status ;;
logs) cmd_logs ;;
run) cmd_run ;;
uninstall) cmd_uninstall ;;
*) die "unknown command '$1' (start, stop, restart, status, logs, run, uninstall)" ;;
esac

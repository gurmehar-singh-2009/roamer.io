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
# Both services listen on [::]:PORT (every interface, IPv6 and IPv4).
# Ports 8080 and 3000 can't be used.

# The web page (TCP). Players open http://HOST:CLIENT_PORT. The game's
# WebSocket is passed through at /ws on this same port.
CLIENT_PORT=8081
# The game server's WebSocket (TCP).
GAME_PORT=8082

# Where browsers connect to the game. Blank = /ws on the web page's own
# address (right for almost everyone). Set it when the game server has its
# own address, e.g. a domain whose proxy forwards to GAME_PORT:
#   GAME_URL=wss://game.example.com
GAME_URL=

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
    GAME_URL="${GAME_URL:-}"
    ROAMER_DEV="${ROAMER_DEV:-0}"
    # What each service binds to.
    CLIENT_BIND="[::]:$CLIENT_PORT"
    GAME_BIND="[::]:$GAME_PORT"
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
    ((CLIENT_PORT != GAME_PORT)) || die "CLIENT_PORT and GAME_PORT must differ"
    [[ -z "$GAME_URL" || "$GAME_URL" =~ ^wss?:// ]] || die "GAME_URL must start with ws:// or wss:// (or be blank)"
    [[ -x "$DIR/bin/roamer-server" && -x "$DIR/bin/roamer-client" ]] || die "missing binaries in $DIR/bin (run build_release.sh)"
    [[ -f "$DIR/web/index.html" ]] || die "missing the web client in $DIR/web (run build_release.sh)"
}

# Fails if something else already listens on our ports.
check_ports_free() {
    command -v ss >/dev/null 2>&1 || return 0
    local busy
    for p in "$CLIENT_PORT" "$GAME_PORT"; do
        busy=$(ss -Htln "sport = :$p" 2>/dev/null || true)
        [[ -z "$busy" ]] || die "TCP port $p is already in use"
    done
}

# Tells the web page where the game is (blank = /ws on its own address).
write_client_config() {
    printf '{ "gameUrl": "%s" }\n' "$GAME_URL" >"$DIR/web/config.json"
}

# -------------------------------------------------------------------- run --

SERVER_PID=""
CLIENT_PID=""

start_server() {
    (
        cd "$DATA"
        GAME_BIND="$GAME_BIND" ROAMER_CONFIG="$DIR/config/config.toml" \
            LOG_FILE="$LOGS/server.log" ACCOUNTS_FILE="$DATA/accounts.json" ROAMER_DEV="$ROAMER_DEV" \
            exec "$DIR/bin/roamer-server"
    ) >>"$LOGS/server.out" 2>&1 &
    SERVER_PID=$!
}

start_client() {
    CLIENT_BIND="$CLIENT_BIND" WEB_ROOT="$DIR/web" GAME_ADDR="127.0.0.1:$GAME_PORT" \
        "$DIR/bin/roamer-client" >>"$LOGS/client.log" 2>&1 &
    CLIENT_PID=$!
}

stop_children() {
    [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
    [[ -n "$CLIENT_PID" ]] && kill "$CLIENT_PID" 2>/dev/null || true
    wait 2>/dev/null || true
}

# Runs both in the foreground and restarts whichever stops.
cmd_run() {
    load_env
    check_settings
    mkdir -p "$DATA" "$LOGS"
    check_ports_free
    write_client_config
    trap 'stop_children; exit 0' INT TERM
    start_server
    start_client
    say "Roamer.io is running: web page on $CLIENT_BIND, game on $GAME_BIND (logs in $LOGS)"

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
    done
}

# Does CLIENT_PORT answer with the web page?
page_ok() {
    local page
    if command -v curl >/dev/null 2>&1; then
        page=$(curl -fsS -m 3 "http://127.0.0.1:$CLIENT_PORT/" 2>/dev/null) || return 1
    else
        exec 3<>"/dev/tcp/127.0.0.1/$CLIENT_PORT" 2>/dev/null || return 1
        printf 'GET / HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3
        page=$(timeout 3 cat <&3 2>/dev/null || true)
        exec 3>&-
    fi
    [[ "$page" == *"<html"* ]]
}

# Waits up to 15 seconds for the web page to be served.
wait_for_page() {
    for _ in $(seq 30); do
        page_ok && return 0
        sleep 0.5
    done
    tail -n 5 "$LOGS/client.log" "$LOGS/server.out" "$LOGS/roamer.log" 2>/dev/null || true
    die "port $CLIENT_PORT isn't serving the web page (see above, and $LOGS)"
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
    wait_for_page
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
    local serving=no
    page_ok && serving=yes
    say "Roamer.io"
    echo "  running:   $running"
    echo "  web page:  $serving ($CLIENT_BIND)"
    echo "  game:      $GAME_BIND, browsers connect to ${GAME_URL:-/ws on the web page}"
    echo "  play at:   http://localhost:$CLIENT_PORT"
    local ip ips
    ips=$(hostname -I 2>/dev/null || true)
    if [[ -z "$ips" ]] && command -v ip >/dev/null 2>&1; then
        ips=$(ip -o addr show scope global 2>/dev/null | awk '{split($4, a, "/"); print a[1]}')
    fi
    for ip in $ips; do
        [[ "$ip" == *:* ]] && ip="[$ip]"
        echo "             http://$ip:$CLIENT_PORT"
    done
    echo "  firewall:  open TCP $CLIENT_PORT${GAME_URL:+ (and whatever GAME_URL points at)}"
    echo "  logs:      $LOGS"
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

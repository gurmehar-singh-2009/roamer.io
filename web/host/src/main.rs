//! Serves the built Roamer.io web client (the files `vite build` makes) over
//! plain HTTP.
//!
//! Settings (environment variables):
//! - `CLIENT_BIND`: address to listen on (default `[::]:8081`, every
//!   interface, IPv6 and IPv4). `CLIENT_PORT` alone changes just the port.
//! - `WEB_ROOT`: folder with the client files (default `web`).
//! - `GAME_ADDR`: the game server's WebSocket port (default
//!   `127.0.0.1:8082`). WebSocket upgrades on `/ws` are passed through to it,
//!   so the page and the game can share one port.

use std::{
    net::IpAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Port used when `CLIENT_PORT` isn't set.
const DEFAULT_PORT: u16 = 8081;
/// Biggest request head we read.
const MAX_REQUEST: usize = 8192;

/// Default game server (WebSocket) address.
const DEFAULT_GAME_ADDR: &str = "127.0.0.1:8082";
/// Longest a client may take to send its request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared settings for every request.
struct Site {
    /// Folder with the client files.
    root: PathBuf,
    /// The game server's WebSocket port.
    game_addr: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("CLIENT_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let site = Arc::new(Site {
        root: PathBuf::from(std::env::var("WEB_ROOT").unwrap_or_else(|_| "web".into())),
        game_addr: std::env::var("GAME_ADDR").unwrap_or_else(|_| DEFAULT_GAME_ADDR.into()),
    });

    let addr = std::env::var("CLIENT_BIND").unwrap_or_else(|_| format!("[::]:{port}"));
    let listener = TcpListener::bind(&addr).await?;
    log(&format!("serving {} on http://{addr}", site.root.display()));

    loop {
        let Ok((socket, peer)) = listener.accept().await else {
            continue;
        };
        let site = site.clone();
        tokio::spawn(async move {
            let _ = handle(socket, peer.ip(), &site).await;
        });
    }
}

/// Writes a line to the log (stderr, which the start script sends to a file).
#[allow(clippy::print_stderr, reason = "this is the program's log")]
fn log(line: &str) {
    eprintln!("{line}");
}

/// Answers one request.
async fn handle<S: AsyncRead + AsyncWrite + Unpin>(
    mut socket: S,
    peer: IpAddr,
    site: &Site,
) -> std::io::Result<()> {
    let mut buf = vec![0_u8; MAX_REQUEST];
    let mut len = 0;
    // Read until the end of the request head.
    loop {
        let n = tokio::time::timeout(REQUEST_TIMEOUT, socket.read(&mut buf[len..]))
            .await
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
        if n == 0 {
            return Ok(());
        }
        len += n;
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") || len == buf.len() {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..len]);
    let mut parts = head.lines().next().unwrap_or_default().split_whitespace();
    let (method, target) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or("/"),
    );

    if method != "GET" && method != "HEAD" {
        return respond(
            &mut socket,
            "405 Method Not Allowed",
            "text/plain",
            b"method not allowed",
            method,
        )
        .await;
    }

    if target.split('?').next() == Some("/ws") {
        let head_len = head_end(&buf[..len]).unwrap_or(len);
        let mut upstream_head =
            forwarded_head(&head[..head.len().min(head_len)], peer).into_bytes();
        upstream_head.extend_from_slice(&buf[head_len..len]);
        return proxy_game(&mut socket, &site.game_addr, &upstream_head).await;
    }

    let Some(path) = resolve(&site.root, target) else {
        return respond(
            &mut socket,
            "404 Not Found",
            "text/plain",
            b"not found",
            method,
        )
        .await;
    };
    match tokio::fs::read(&path).await {
        Ok(body) => respond(&mut socket, "200 OK", content_type(&path), &body, method).await,
        Err(_) => {
            respond(
                &mut socket,
                "404 Not Found",
                "text/plain",
                b"not found",
                method,
            )
            .await
        }
    }
}

/// The file a request path points to, or None if it tries to leave the root.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let path = percent_decode(path);
    let mut file = root.to_path_buf();
    for part in Path::new(path.trim_start_matches('/')).components() {
        match part {
            Component::Normal(p) => file.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if file.is_dir() || path.ends_with('/') {
        file.push("index.html");
    }
    Some(file)
}

/// Decodes `%20`-style escapes in a URL path.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = bytes.get(i + 1..i + 3)
            && let Some(b) = std::str::from_utf8(hex)
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Content type by file extension.
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ttf" => "font/ttf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Where the request head ends (just past the blank line).
fn head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// The request head with `X-Forwarded-For` set to the player's IP, so the
/// game server sees who's connecting. A proxy on this machine (Caddy, nginx)
/// has already set it; from anyone else it could be faked, so it's replaced.
fn forwarded_head(head: &str, peer: IpAddr) -> String {
    let peer = peer.to_canonical();
    let mut forwarded = None;
    let mut lines = Vec::new();
    for line in head.split("\r\n").filter(|l| !l.is_empty()) {
        match line.split_once(':') {
            Some((name, value)) if name.trim().eq_ignore_ascii_case("x-forwarded-for") => {
                forwarded = Some(value.trim().to_string());
            }
            _ => lines.push(line),
        }
    }
    let client = match forwarded {
        Some(chain) if peer.is_loopback() => chain,
        _ => peer.to_string(),
    };
    format!(
        "{}\r\nX-Forwarded-For: {client}\r\n\r\n",
        lines.join("\r\n")
    )
}

/// Passes a WebSocket connection through to the game server: sends it the
/// request, then copies bytes both ways until either side hangs up.
async fn proxy_game<S: AsyncRead + AsyncWrite + Unpin>(
    socket: &mut S,
    game_addr: &str,
    head: &[u8],
) -> std::io::Result<()> {
    let Ok(Ok(mut upstream)) =
        tokio::time::timeout(REQUEST_TIMEOUT, TcpStream::connect(game_addr)).await
    else {
        return respond(
            socket,
            "502 Bad Gateway",
            "text/plain",
            b"game server unreachable",
            "GET",
        )
        .await;
    };
    let _ = upstream.set_nodelay(true);
    upstream.write_all(head).await?;
    tokio::io::copy_bidirectional(socket, &mut upstream).await?;
    Ok(())
}

/// Sends a response. Hashed build files are cached for long; everything
/// else (index.html, config.json) is always fetched fresh.
async fn respond<S: AsyncWrite + Unpin>(
    socket: &mut S,

    status: &str,
    kind: &str,
    body: &[u8],
    method: &str,
) -> std::io::Result<()> {
    let cache = if kind.starts_with("text/html") || kind == "application/json" {
        "no-store"
    } else {
        "public, max-age=86400"
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nConnection: \
         close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await?;
    if method != "HEAD" {
        socket.write_all(body).await?;
    }
    socket.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_root() {
        let root = Path::new("/srv/web");
        assert_eq!(
            resolve(root, "/assets/a.js?v=1"),
            Some(PathBuf::from("/srv/web/assets/a.js"))
        );
        assert_eq!(
            resolve(root, "/"),
            Some(PathBuf::from("/srv/web/index.html"))
        );
        assert_eq!(resolve(root, "/../etc/passwd"), None);
        assert_eq!(resolve(root, "/%2e%2e/etc/passwd"), None);
    }

    #[test]
    fn forwarded_for_is_only_trusted_from_this_machine() {
        let head = "GET /ws?pow=1 HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: 1.2.3.4\r\n\r\n";
        let local: IpAddr = "127.0.0.1".parse().unwrap();
        let remote: IpAddr = "::ffff:5.6.7.8".parse().unwrap();
        assert_eq!(
            forwarded_head(head, local),
            "GET /ws?pow=1 HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: 1.2.3.4\r\n\r\n"
        );
        assert_eq!(
            forwarded_head(head, remote),
            "GET /ws?pow=1 HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: 5.6.7.8\r\n\r\n"
        );
    }

    #[test]
    fn known_types() {
        assert_eq!(content_type(Path::new("a.wasm")), "application/wasm");
        assert_eq!(
            content_type(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
    }
}

//! Tiny localhost HTTP server for media playback.
//!
//! On Linux, WebKitGTK routes `<video>` loads through GStreamer, which cannot
//! service Tauri's `asset://` custom scheme (WebKit bug 146351) — videos fail
//! with MEDIA_ERR_SRC_NOT_SUPPORTED. Serving the file over `http://127.0.0.1`
//! works because `http` is always an allowed media protocol, and Range
//! requests give us normal seek behaviour.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

static TOKEN: OnceLock<String> = OnceLock::new();
static PORT: OnceLock<u16> = OnceLock::new();

fn random_token() -> String {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut buf = [0u8; 16];
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            if f.read_exact(&mut buf).is_ok() {
                return buf.iter().map(|b| format!("{b:02x}")).collect();
            }
        }
    }
    format!(
        "{:x}{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    )
}

/// Spawn the media server on a dedicated thread + tokio runtime.
/// Safe to call more than once; only the first call binds a socket.
pub fn start() {
    if PORT.get().is_some() {
        return;
    }
    let token = TOKEN.get_or_init(random_token).clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind media server");
    listener.set_nonblocking(true).expect("set nonblocking");
    let port = listener.local_addr().expect("local addr").port();
    let _ = PORT.set(port);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("media server runtime");
        rt.block_on(async move {
            let listener = TcpListener::from_std(listener).expect("tokio listener");
            while let Ok((stream, _)) = listener.accept().await {
                let token = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, &token).await {
                        // The webview aborts in-flight Range requests on every
                        // seek/pause — a reset socket is normal, not an error.
                        if !is_client_disconnect(&e) {
                            eprintln!("[media] connection error: {e}");
                        }
                    }
                });
            }
        });
    });
}

/// Client aborted the transfer (seek/pause/close) — expected, skip logging.
fn is_client_disconnect(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        matches!(
            c.downcast_ref::<std::io::Error>().map(|io| io.kind()),
            Some(std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe)
        )
    })
}

/// Build the playback URL for a local file path, if the server is running.
pub fn media_url(path: &str) -> Option<String> {
    let port = *PORT.get()?;
    let token = TOKEN.get()?;
    Some(format!(
        "http://127.0.0.1:{port}/media/{token}/{}",
        url_encode(path)
    ))
}

#[tauri::command]
pub fn get_media_url(path: String) -> Result<String, String> {
    media_url(&path).ok_or_else(|| "Media server chưa khởi động".to_string())
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "flac" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        403 => "Forbidden",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        _ => "Error",
    }
}

async fn write_headers(
    stream: &mut TcpStream,
    status: u16,
    extra_headers: &str,
    content_length: u64,
) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n{}\r\n",
        status,
        reason(status),
        content_length,
        extra_headers
    );
    stream.write_all(head.as_bytes()).await?;
    Ok(())
}

async fn handle_conn(mut stream: TcpStream, token: &str) -> Result<()> {
    // Read until end of headers (compact GET from WebKit; cap at 64KB).
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 65536 {
            break;
        }
    }

    let req = String::from_utf8_lossy(&buf);
    let mut lines = req.lines();
    let mut parts = lines.next().unwrap_or_default().split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();

    let mut range: Option<(u64, Option<u64>)> = None;
    for line in lines {
        if line.trim().is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("range:") {
            let spec = v.trim().trim_start_matches("bytes=");
            let mut it = spec.splitn(2, '-');
            if let (Some(Ok(s)), e) = (
                it.next().map(|s| s.trim().parse::<u64>()),
                it.next().and_then(|s| s.trim().parse::<u64>().ok()),
            ) {
                range = Some((s, e));
            }
        }
    }

    let prefix = format!("/media/{token}/");
    let encoded = target.strip_prefix(&prefix);
    if method != "GET" && method != "HEAD" || encoded.is_none() {
        write_headers(&mut stream, 403, "", 0).await?;
        return Ok(());
    }

    let path = PathBuf::from(url_decode(encoded.unwrap()));
    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) if m.is_file() => m,
        _ => {
            write_headers(&mut stream, 404, "", 0).await?;
            return Ok(());
        }
    };

    let mut file = tokio::fs::File::open(&path)
        .await
        .context("open media file")?;
    let len = meta.len();

    let (status, start, end) = match range {
        Some((s, e)) if len > 0 && s < len => {
            let e = e.unwrap_or(len - 1).min(len - 1);
            if e < s {
                write_headers(&mut stream, 416, "", 0).await?;
                return Ok(());
            }
            (206, s, e)
        }
        Some(_) => {
            write_headers(&mut stream, 416, "", 0).await?;
            return Ok(());
        }
        None => (200, 0, len.saturating_sub(1)),
    };

    let body_len = if len == 0 { 0 } else { end - start + 1 };
    let content_range = if status == 206 {
        format!("Content-Range: bytes {start}-{end}/{len}\r\n")
    } else {
        String::new()
    };
    let extra = format!("Content-Type: {}\r\n{}", mime_for(&path), content_range);
    write_headers(&mut stream, status, &extra, body_len).await?;

    if method == "HEAD" || body_len == 0 {
        return Ok(());
    }

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut limited = file.take(body_len);
    tokio::io::copy(&mut limited, &mut stream).await?;
    Ok(())
}

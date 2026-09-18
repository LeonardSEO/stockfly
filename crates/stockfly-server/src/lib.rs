//! Minimal localhost static server for the StockFly browser app. Sets
//! cross-origin isolation headers on every response, since the browser's
//! multithreaded WASM/SharedArrayBuffer path requires COOP/COEP to be
//! present (a secure context, i.e. localhost or https, plus these two
//! headers) -- without them the browser silently disables
//! SharedArrayBuffer instead of erroring, which is why this is enforced
//! server-side rather than left to be discovered per-deployment.

use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use tiny_http::{Header, Response, Server};

pub struct ServerConfig {
    pub web_dir: PathBuf,
    pub model_dir: PathBuf,
    pub bind_addr: String,
}

fn coop_coep_headers() -> Vec<Header> {
    vec![
        Header::from_bytes(&b"Cross-Origin-Opener-Policy"[..], &b"same-origin"[..]).unwrap(),
        Header::from_bytes(&b"Cross-Origin-Embedder-Policy"[..], &b"require-corp"[..]).unwrap(),
    ]
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("bin" | "sfckpt") => "application/octet-stream",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// True for immutable model/checkpoint blobs, which are content-addressed
/// by their compiled/trained hash and safe to cache for a year.
fn is_long_cache_asset(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("wasm" | "bin" | "sfckpt")
    )
}

/// Resolves a URL path against `root`, rejecting any path that would climb
/// outside of it (`..` components) -- the server exists to serve exactly
/// `web_dir`/`model_dir` and nothing else on the filesystem.
fn resolve_safe_path(root: &Path, url_path: &str) -> Option<PathBuf> {
    let trimmed = url_path.trim_start_matches('/');
    let candidate = if trimmed.is_empty() { "index.html" } else { trimmed };
    let mut resolved = root.to_path_buf();
    for component in Path::new(candidate).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            _ => return None, // ParentDir, RootDir, Prefix: reject
        }
    }
    Some(resolved)
}

pub fn handle_request(config: &ServerConfig, url: &str) -> Response<Cursor<Vec<u8>>> {
    let url_path = url.split('?').next().unwrap_or(url);

    if url_path == "/health" {
        return respond_json(200, r#"{"status":"ok"}"#);
    }

    if url_path == "/models" {
        let names: Vec<String> = fs::read_dir(&config.model_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("sfckpt"))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        let body = serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string());
        return respond_json(200, &body);
    }

    let Some(path) = resolve_safe_path(&config.web_dir, url_path) else {
        return respond_plain(403, "forbidden path");
    };

    match fs::read(&path) {
        Ok(bytes) => {
            let mut response = Response::from_data(bytes)
                .with_status_code(200)
                .with_header(Header::from_bytes(&b"Content-Type"[..], content_type_for(&path).as_bytes()).unwrap());
            for h in coop_coep_headers() {
                response = response.with_header(h);
            }
            if is_long_cache_asset(&path) {
                response = response.with_header(
                    Header::from_bytes(&b"Cache-Control"[..], &b"public, max-age=31536000, immutable"[..]).unwrap(),
                );
            }
            response
        }
        Err(_) => respond_plain(404, "not found"),
    }
}

fn respond_plain(status: u16, body: &str) -> Response<Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body).with_status_code(status);
    for h in coop_coep_headers() {
        response = response.with_header(h);
    }
    response
}

fn respond_json(status: u16, body: &str) -> Response<Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap());
    for h in coop_coep_headers() {
        response = response.with_header(h);
    }
    response
}

pub fn run(config: ServerConfig) -> std::io::Result<()> {
    let server = Server::http(&config.bind_addr).map_err(std::io::Error::other)?;
    println!("StockFly server listening on http://{}", config.bind_addr);
    for request in server.incoming_requests() {
        let response = handle_request(&config, request.url());
        let _ = request.respond(response);
    }
    Ok(())
}

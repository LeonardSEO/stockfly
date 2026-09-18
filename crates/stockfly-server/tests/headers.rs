use std::fs;
use std::thread;
use std::time::Duration;

use stockfly_server::{run, ServerConfig};

fn free_port() -> u16 {
    // Bind to port 0 to let the OS pick a free one, then drop the listener.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn start_test_server() -> (String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("index.html"), "<html>stockfly</html>").unwrap();
    fs::create_dir_all(tmp.path().join("models")).unwrap();
    fs::write(tmp.path().join("models").join("stockfly-bio-full.sfckpt"), b"{}").unwrap();

    let port = free_port();
    let bind_addr = format!("127.0.0.1:{port}");
    let config = ServerConfig {
        web_dir: tmp.path().to_path_buf(),
        model_dir: tmp.path().join("models"),
        bind_addr: bind_addr.clone(),
    };

    thread::spawn(move || {
        let _ = run(config);
    });
    // Give the server a moment to bind before the test issues requests.
    thread::sleep(Duration::from_millis(100));

    (format!("http://{bind_addr}"), tmp)
}

#[test]
fn responses_carry_cross_origin_isolation_headers() {
    let (base, _tmp) = start_test_server();

    let response = ureq::get(&base).call().unwrap();
    assert_eq!(response.headers().get("Cross-Origin-Opener-Policy").unwrap(), "same-origin");
    assert_eq!(response.headers().get("Cross-Origin-Embedder-Policy").unwrap(), "require-corp");
}

#[test]
fn model_blobs_get_immutable_long_lived_cache_control() {
    let (base, _tmp) = start_test_server();

    let response = ureq::get(&format!("{base}/index.html")).call().unwrap();
    assert!(response.headers().get("Cache-Control").is_none(), "index.html should not be long-cached");

    // Exercise a .wasm-style path served straight from web_dir to verify
    // the cache-control rule, since /models only lists filenames.
    let (base2, tmp2) = start_test_server();
    fs::write(tmp2.path().join("model.wasm"), b"fake wasm bytes").unwrap();
    let response2 = ureq::get(&format!("{base2}/model.wasm")).call().unwrap();
    assert_eq!(
        response2.headers().get("Cache-Control").unwrap(),
        "public, max-age=31536000, immutable"
    );
}

#[test]
fn health_endpoint_reports_ok() {
    let (base, _tmp) = start_test_server();
    let mut response = ureq::get(&format!("{base}/health")).call().unwrap();
    let text = response.body_mut().read_to_string().unwrap();
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["status"], "ok");
}

#[test]
fn models_endpoint_lists_only_local_sfckpt_filenames() {
    let (base, _tmp) = start_test_server();
    let mut response = ureq::get(&format!("{base}/models")).call().unwrap();
    let text = response.body_mut().read_to_string().unwrap();
    let body: Vec<String> = serde_json::from_str(&text).unwrap();
    assert_eq!(body, vec!["stockfly-bio-full.sfckpt".to_string()]);
}

#[test]
fn path_traversal_outside_web_dir_is_rejected() {
    let (base, _tmp) = start_test_server();
    let response = ureq::get(&format!("{base}/../../../../etc/passwd")).call();
    // Either the HTTP client normalizes this away or the server rejects
    // it; in no case should it succeed in returning /etc/passwd contents.
    if let Ok(mut r) = response {
        let body = r.body_mut().read_to_string().unwrap_or_default();
        assert!(!body.contains("root:"), "path traversal must not read outside web_dir");
    }
}

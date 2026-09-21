use std::path::PathBuf;

use stockfly_server::{run, ServerConfig};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:5187";

fn browser_url(bind_addr: &str) -> String {
    let display_addr = bind_addr
        .strip_prefix("127.0.0.1:")
        .map(|port| format!("localhost:{port}"))
        .unwrap_or_else(|| bind_addr.to_string());
    format!("http://{display_addr}/#play")
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "fetch-models") {
        let mut root = PathBuf::from(".");
        let mut tag = None;
        let mut fixture = None;
        let mut i = 1;
        while i < args.len() {
            let value = args
                .get(i + 1)
                .ok_or_else(|| std::io::Error::other("missing fetch-models option value"))?;
            match args[i].as_str() {
                "--root" => root = value.into(),
                "--tag" => tag = Some(value.as_str()),
                "--fixture-base" => fixture = Some(value.as_str()),
                other => {
                    return Err(std::io::Error::other(format!(
                        "unknown fetch-models option: {other}"
                    )))
                }
            }
            i += 2;
        }
        return stockfly_server::install::fetch_models(&root, tag, fixture)
            .map_err(|error| std::io::Error::other(error.to_string()));
    }
    let mut data_dir = PathBuf::from("data");
    let mut web_dir = PathBuf::from("apps/web/dist");
    let mut model_dir = PathBuf::from("data/checkpoints");
    let mut bind_addr = DEFAULT_BIND_ADDR.to_string();
    let mut open = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--data-dir" => {
                i += 1;
                data_dir = args.get(i).map(PathBuf::from).unwrap_or(data_dir);
            }
            "--web-dir" => {
                i += 1;
                web_dir = args.get(i).map(PathBuf::from).unwrap_or(web_dir);
            }
            "--model-dir" => {
                i += 1;
                model_dir = args.get(i).map(PathBuf::from).unwrap_or(model_dir);
            }
            "--bind" => {
                i += 1;
                bind_addr = args.get(i).cloned().unwrap_or(bind_addr);
            }
            "--open" => {
                open = true;
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let url = browser_url(&bind_addr);
    if open {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    println!("{url}");

    run(ServerConfig {
        web_dir,
        model_dir,
        data_dir,
        bind_addr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_browser_url_opens_local_play() {
        assert_eq!(
            browser_url(DEFAULT_BIND_ADDR),
            "http://localhost:5187/#play"
        );
    }

    #[test]
    fn custom_loopback_port_keeps_localhost_play_url() {
        assert_eq!(browser_url("127.0.0.1:9000"), "http://localhost:9000/#play");
    }
}

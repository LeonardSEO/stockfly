//! Release integrity installer. Hashes identify content; they are not signatures.
use std::{fs, io::{self, Read, Write}, path::{Component, Path, PathBuf}};
use serde_json::Value;
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const REPOSITORY: &str = "LeonardSEO/stockfly";

fn checked_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let allowed = ["data/compiled/malecns-v1/", "data/checkpoints/", "data/browser-models/", "data/browser/", "data/chess-maps/", "data/release-evidence/"];
    if !allowed.iter().any(|prefix| relative.starts_with(prefix)) || relative.contains('\\') || relative.contains(':') || relative.contains('%') {
        return Err(format!("unapproved artifact path: {relative}").into());
    }
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(name) => path.push(name),
            _ => return Err(format!("unsafe artifact path: {relative}").into()),
        }
        if fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(format!("installation path contains a symlink: {}", path.display()).into());
        }
    }
    Ok(path)
}
fn digest(path: &Path) -> Result<(String, u64)> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        hash.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((format!("{:x}", hash.finalize()), size))
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().filter(|s| !s.is_empty()).ok_or_else(|| format!("missing {key}").into())
}
fn sha(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }

/// CLI only: a local fixture URL is deliberately not exposed as an HTTP route.
pub fn fetch_models(root: &Path, tag: Option<&str>, fixture_base: Option<&str>) -> Result<()> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let (base, expected_tag) = if let Some(base) = fixture_base {
        let authority = base.strip_prefix("http://127.0.0.1:").ok_or("fixture URL must use http://127.0.0.1:PORT")?;
        if authority.parse::<u16>().is_err() { return Err("invalid fixture port".into()); }
        (base.to_owned(), tag.map(str::to_owned))
    } else {
        let endpoint = match tag {
            Some(tag) if !tag.is_empty() && tag.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) => format!("tags/{tag}"),
            Some(_) => return Err("tag must contain only letters, digits, dot, underscore or hyphen".into()),
            None => "?per_page=100".to_string(),
        };
        let mut response = ureq::get(format!("https://api.github.com/repos/{REPOSITORY}/releases{}", if endpoint.starts_with('?') { endpoint.clone() } else { format!("/{endpoint}") }))
            .header("User-Agent", "StockFly-model-installer").call()?;
        let release: Value = serde_json::from_str(&response.body_mut().read_to_string()?)?;
        // GitHub's /latest excludes prereleases. Experimental model releases are
        // prereleases, so default to the newest published release, including them.
        let release = if tag.is_none() {
            release.as_array().ok_or("invalid release listing")?.iter()
                .filter(|release| release["draft"] == false)
                .max_by_key(|release| release["published_at"].as_str().unwrap_or(""))
                .ok_or("no published model release exists; use locally prepared artifacts")?
        } else { &release };
        let resolved = string(release, "tag_name")?;
        if !resolved.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) { return Err("unsafe release tag".into()); }
        (format!("https://github.com/{REPOSITORY}/releases/download/{resolved}"), Some(resolved.to_owned()))
    };
    let mut response = ureq::get(format!("{base}/release-manifest.json")).call()?;
    let manifest_text = response.body_mut().read_to_string()?;
    let manifest: Value = serde_json::from_str(&manifest_text)?;
    if manifest["formatVersion"] != 1 || manifest["releaseStatus"] != "experimental" || manifest["causalAuditStatus"] != "failed" || manifest["liteStatus"] != "blocked" {
        return Err("only explicitly experimental, failed-causal-gate manifests are supported by this release".into());
    }
    if expected_tag.as_deref().is_some_and(|tag| manifest["tag"].as_str() != Some(tag)) { return Err("manifest tag differs from requested release".into()); }
    for key in ["maleCns", "stockfish"] { string(&manifest["attribution"], key)?; }
    let files = manifest["files"].as_array().filter(|files| !files.is_empty()).ok_or("manifest has no files")?;
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for file in files {
        let path = string(file, "path")?;
        checked_path(&root, path)?;
        if !seen.insert(path) { return Err("duplicate artifact path".into()); }
        let asset = string(file, "asset")?;
        if !asset.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) || asset == "." || asset == ".." { return Err("unsafe release asset name".into()); }
        if !sha(string(file, "sha256")?) || !sha(string(file, "graphNeuronsSha256")?) { return Err("invalid SHA-256".into()); }
        if !matches!(string(file, "modelKind")?, "shared" | "bio-full" | "max-full") { return Err("unsupported model kind".into()); }
        string(file, "trainingPreset")?;
        total = total.checked_add(file["size"].as_u64().ok_or("invalid file size")?).ok_or("download size overflow")?;
    }
    println!("Release {}: {total} bytes (matching files are skipped). Experimental models; causal gate FAILED.", manifest["tag"]);
    let stage = tempfile::Builder::new().prefix(".stockfly-download-").tempdir_in(&root)?;
    let mut replacements = Vec::new();
    for (i, file) in files.iter().enumerate() {
        let relative = string(file, "path")?;
        let target = checked_path(&root, relative)?;
        let expected = (string(file, "sha256")?.to_owned(), file["size"].as_u64().unwrap());
        if target.is_file() && digest(&target)? == expected { println!("Verified existing {relative}"); continue; }
        let temporary = stage.path().join(i.to_string());
        let mut output = fs::File::create(&temporary)?;
        let mut response = ureq::get(format!("{base}/{}", string(file, "asset")?)).call()?;
        // Bound each transfer to its declared size plus one byte, detecting oversized downloads.
        io::copy(&mut response.body_mut().as_reader().take(expected.1.saturating_add(1)), &mut output)?;
        output.flush()?;
        if digest(&temporary)? != expected { return Err(format!("SHA-256 or size mismatch: {relative}").into()); }
        replacements.push((relative.to_owned(), temporary));
    }
    // Do not install any bytes until every download has passed verification. Publish catalog last.
    replacements.sort_by_key(|(path, _)| path.ends_with("catalog.json"));
    for (relative, temporary) in replacements {
        let target = checked_path(&root, &relative)?;
        fs::create_dir_all(target.parent().unwrap())?;
        checked_path(&root, &relative)?;
        // Persist uses platform atomic replacement, including existing files on Windows.
        let mut file = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
        io::copy(&mut fs::File::open(temporary)?, &mut file)?;
        file.persist(&target)?;
        println!("Installed {relative}");
    }
    println!("Download complete. Start StockFly, or retry model loading in the browser.");
    Ok(())
}

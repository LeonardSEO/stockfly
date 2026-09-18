use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use stockfly_connectome::format::CompiledManifest;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputHashes {
    pub graph_sha256: String,
    pub graph_files: BTreeMap<String, String>,
    pub manifest_sha256: String,
    pub model_sha256: String,
    pub suite_sha256: String,
    pub sensory_map_sha256: String,
    pub output_map_sha256: String,
    pub metadata_sha256: String,
    pub binary_sha256: String,
}

pub fn sha256_file(path: impl AsRef<Path>) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verified_graph_files(graph: &Path) -> Result<BTreeMap<String, String>> {
    let manifest: CompiledManifest =
        serde_json::from_slice(&fs::read(graph.join("manifest.json"))?)?;
    let mut expected = BTreeMap::from([
        ("neurons.bin".to_string(), manifest.neurons_sha256),
        ("offsets.bin".to_string(), manifest.offsets_sha256),
    ]);
    for block in manifest.edge_blocks {
        expected.insert(
            format!("edge_src_blocks/{:04}.bin", block.index),
            block.src_sha256,
        );
        expected.insert(
            format!("edge_weight_blocks/{:04}.bin", block.index),
            block.weight_sha256,
        );
    }
    let mut actual = BTreeMap::new();
    for (name, expected_digest) in expected {
        let digest = sha256_file(graph.join(&name))?;
        if digest != expected_digest {
            return Err(format!("compiled graph hash mismatch: {name}").into());
        }
        actual.insert(name, digest);
    }
    Ok(actual)
}

/// Hashes the actual audit inputs without buffering full graph blocks. The
/// graph aggregate matches the Python wrapper exactly: SHA-256 of the compact,
/// key-sorted JSON object mapping graph-relative paths to their actual digests.
pub fn input_hashes(
    graph: impl AsRef<Path>,
    model: impl AsRef<Path>,
    suite: impl AsRef<Path>,
    chess: impl AsRef<Path>,
    metadata: impl AsRef<Path>,
    binary: impl AsRef<Path>,
) -> Result<InputHashes> {
    let graph = graph.as_ref();
    let graph_files = verified_graph_files(graph)?;
    let graph_sha256 = {
        let canonical = serde_json::to_vec(&graph_files)?;
        format!("{:x}", Sha256::digest(canonical))
    };
    let chess: PathBuf = chess.as_ref().into();
    Ok(InputHashes {
        graph_sha256,
        graph_files,
        manifest_sha256: sha256_file(graph.join("manifest.json"))?,
        model_sha256: sha256_file(model)?,
        suite_sha256: sha256_file(suite)?,
        sensory_map_sha256: sha256_file(chess.join("sensory-map.json"))?,
        output_map_sha256: sha256_file(chess.join("output-map.json"))?,
        metadata_sha256: sha256_file(metadata)?,
        binary_sha256: sha256_file(binary)?,
    })
}

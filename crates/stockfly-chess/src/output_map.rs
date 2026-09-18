use std::collections::HashMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const PROMOTION_ORDER: [&str; 4] = ["queen", "rook", "bishop", "knight"];

#[derive(Debug, Deserialize)]
pub struct OutputMap {
    pub format_version: u32,
    pub seed: u64,
    pub source_population: String,
    pub from_groups: Vec<Vec<u32>>,
    pub to_groups: Vec<Vec<u32>>,
    pub promotion_groups: HashMap<String, Vec<u32>>,
    #[serde(default)]
    canonical_json: Option<String>,
}

#[derive(Debug)]
pub enum OutputMapError {
    Json(serde_json::Error),
    Invariant(String),
}

impl std::fmt::Display for OutputMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputMapError::Json(e) => write!(f, "output map parse error: {e}"),
            OutputMapError::Invariant(s) => write!(f, "output map invariant violated: {s}"),
        }
    }
}
impl std::error::Error for OutputMapError {}
impl From<serde_json::Error> for OutputMapError {
    fn from(e: serde_json::Error) -> Self {
        OutputMapError::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, OutputMapError>;

impl OutputMap {
    pub fn load_str(json: &str) -> Result<Self> {
        let mut map: OutputMap = serde_json::from_str(json)?;

        if map.from_groups.len() != 64 {
            return Err(OutputMapError::Invariant(format!(
                "expected 64 from_groups, got {}",
                map.from_groups.len()
            )));
        }
        if map.to_groups.len() != 64 {
            return Err(OutputMapError::Invariant(format!(
                "expected 64 to_groups, got {}",
                map.to_groups.len()
            )));
        }
        for name in PROMOTION_ORDER {
            if !map.promotion_groups.contains_key(name) {
                return Err(OutputMapError::Invariant(format!("missing promotion group '{name}'")));
            }
        }

        let mut seen = std::collections::HashSet::new();
        let all_groups = map
            .from_groups
            .iter()
            .chain(map.to_groups.iter())
            .chain(map.promotion_groups.values());
        for group in all_groups {
            if group.is_empty() {
                return Err(OutputMapError::Invariant("a group is empty".into()));
            }
            for &neuron in group {
                if !seen.insert(neuron) {
                    return Err(OutputMapError::Invariant(format!(
                        "neuron {neuron} appears in more than one output group"
                    )));
                }
            }
        }

        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        map.canonical_json = Some(format!("{:x}", hasher.finalize()));
        Ok(map)
    }

    pub fn sha256(&self) -> &str {
        self.canonical_json.as_deref().unwrap_or("")
    }

    pub fn promotion_group(&self, name: &str) -> Option<&[u32]> {
        self.promotion_groups.get(name).map(|v| v.as_slice())
    }
}

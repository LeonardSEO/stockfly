use serde::{Deserialize, Serialize};
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, FrameSummary, Stimulus};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Deserialize)]
pub struct NeuronMetadata {
    pub body_id: u64,
    pub transmitter: String,
    pub region: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuditMetadata {
    pub format_version: u32,
    pub graph_neurons_sha256: String,
    pub source: String,
    pub neurons: Vec<NeuronMetadata>,
}

impl AuditMetadata {
    pub fn validate(&self, graph: &Connectome) -> Result<()> {
        if self.format_version != 1
            || self.graph_neurons_sha256 != graph.manifest.neurons_sha256
            || self.neurons.len() != graph.neurons.len()
            || self
                .neurons
                .iter()
                .zip(&graph.neurons)
                .any(|(metadata, neuron)| metadata.body_id != neuron.body_id)
        {
            return Err(
                "audit neuron metadata does not match compiled graph identity/order".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AblationStats {
    pub annotation_field: &'static str,
    pub annotation_value: String,
    pub silenced_neurons: usize,
}

pub struct RegionAblation {
    mask: Vec<bool>,
    stats: AblationStats,
}

impl RegionAblation {
    pub fn new(graph: &Connectome, metadata: &AuditMetadata, region: &str) -> Result<Self> {
        metadata.validate(graph)?;
        let mask: Vec<bool> = metadata
            .neurons
            .iter()
            .map(|neuron| neuron.region == region)
            .collect();
        let silenced_neurons = mask.iter().filter(|&&value| value).count();
        if silenced_neurons == 0 {
            return Err(format!("ablation region {region:?} has no neurons in this graph").into());
        }
        Ok(Self {
            mask,
            stats: AblationStats {
                annotation_field: "MaleCNS body-annotations superclass",
                annotation_value: region.to_owned(),
                silenced_neurons,
            },
        })
    }

    pub fn step(&self, simulator: &mut CpuSimulator<'_>, stimulus: &Stimulus) -> FrameSummary {
        simulator.step_with_silenced(stimulus, &self.mask)
    }

    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    pub fn stats(&self) -> &AblationStats {
        &self.stats
    }
}

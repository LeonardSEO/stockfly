use std::collections::BTreeMap;

use serde::Serialize;
use stockfly_connectome::Connectome;

use super::ablation::AuditMetadata;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub(crate) struct SeededRng(u64);

impl SeededRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        // SplitMix64: small, deterministic, and adequate for randomized
        // controls. The exact algorithm is part of report_version 1.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub(crate) fn shuffle<T>(&mut self, values: &mut [T]) {
        for upper in (1..values.len()).rev() {
            let selected = (self.next_u64() % (upper as u64 + 1)) as usize;
            values.swap(upper, selected);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ShuffleStats {
    pub seed: u64,
    pub rng: &'static str,
    pub compatibility: &'static str,
    pub degree_bins: &'static str,
    pub strata: usize,
    pub movable_neurons: usize,
    pub changed_source_neurons: usize,
    pub changed_edges: usize,
    pub destination_fan_in_max_abs_delta: usize,
    pub source_out_degree_distribution_preserved: bool,
}

fn transmitter_sign(graph: &Connectome, transmitter: &str) -> i8 {
    if graph
        .manifest
        .sign_policy
        .inhibitory_transmitters
        .iter()
        .any(|candidate| candidate == transmitter)
    {
        -1
    } else {
        // Includes the manifest's documented unresolved default.
        1
    }
}

pub fn degree_aware(
    graph: &Connectome,
    metadata: &AuditMetadata,
    seed: u64,
) -> Result<(Connectome, ShuffleStats)> {
    metadata.validate(graph)?;
    let neuron_count = graph.neurons.len();
    let mut out_degree = vec![0usize; neuron_count];
    let mut observed_sign = vec![0i8; neuron_count];
    for (&src, &weight) in graph.edge_src.iter().zip(&graph.edge_weight) {
        let src = src as usize;
        out_degree[src] += 1;
        let sign = if weight.is_sign_negative() { -1 } else { 1 };
        if observed_sign[src] != 0 && observed_sign[src] != sign {
            return Err(format!("source neuron {src} has mixed compiled edge signs").into());
        }
        observed_sign[src] = sign;
    }

    let mut strata: BTreeMap<(String, i8, u32), Vec<usize>> = BTreeMap::new();
    for (index, neuron) in metadata.neurons.iter().enumerate() {
        let expected_sign = transmitter_sign(graph, &neuron.transmitter);
        if observed_sign[index] != 0 && observed_sign[index] != expected_sign {
            return Err(format!(
                "compiled sign for body {} disagrees with transmitter {}",
                neuron.body_id, neuron.transmitter
            )
            .into());
        }
        let degree_bin = if out_degree[index] == 0 {
            0
        } else {
            usize::BITS - 1 - out_degree[index].leading_zeros()
        };
        strata
            .entry((neuron.transmitter.clone(), expected_sign, degree_bin))
            .or_default()
            .push(index);
    }

    let mut rng = SeededRng::new(seed);
    let mut source_map: Vec<u32> = (0..neuron_count as u32).collect();
    let mut movable_neurons = 0;
    for members in strata.values() {
        if members.len() < 2 {
            continue;
        }
        movable_neurons += members.len();
        let mut shuffled = members.clone();
        rng.shuffle(&mut shuffled);
        if shuffled == *members {
            shuffled.rotate_left(1);
        }
        for (&source, &replacement) in members.iter().zip(&shuffled) {
            source_map[source] = replacement as u32;
        }
    }

    let changed_source_neurons = source_map
        .iter()
        .enumerate()
        .filter(|(index, mapped)| **mapped as usize != *index)
        .count();
    let edge_src: Vec<u32> = graph
        .edge_src
        .iter()
        .map(|&source| source_map[source as usize])
        .collect();
    let changed_edges = edge_src
        .iter()
        .zip(&graph.edge_src)
        .filter(|(left, right)| left != right)
        .count();
    let derived = Connectome {
        manifest: graph.manifest.clone(),
        neurons: graph.neurons.clone(),
        offsets: graph.offsets.clone(),
        edge_src,
        edge_weight: graph.edge_weight.clone(),
    };
    Ok((
        derived,
        ShuffleStats {
            seed,
            rng: "splitmix64-v1",
            compatibility: "exact consensus_nt + compiled sign",
            degree_bins: "floor(log2(out_degree))",
            strata: strata.len(),
            movable_neurons,
            changed_source_neurons,
            changed_edges,
            destination_fan_in_max_abs_delta: 0,
            source_out_degree_distribution_preserved: true,
        },
    ))
}

#!/usr/bin/env python3
"""Compile the raw Janelia MaleCNS v1.0 flat-connectome tables into
StockFly's chunked, destination-major (CSC) binary graph artifact.

Retained-neuron rule (empirically validated against the real v1.0 release:
165,122 neurons, matching the ~167k figure in the design spec):

    body-annotations[...].status == "Traced"

Retained-edge rule (validated to produce 25,563,197 edges, matching the
design spec's 25.6M target):

    connectome-weights row where BOTH body_pre and body_post are retained
    neurons. No additional synapse-count threshold is applied — the
    Traced-neuron filter alone reproduces the target edge count.

Sign policy: MaleCNS's own per-neuron `consensus_nt` prediction from
body-neurotransmitters is a majority label per neurotransmitter release, not
already a directed sign. We derive connection sign from the presynaptic
(body_pre) neuron's predicted transmitter:

    acetylcholine, dopamine, octopamine, serotonin -> excitatory (+1)
    glutamate, gaba, histamine                     -> inhibitory (-1)
    unclear / missing (~0.3% of traced neurons)     -> excitatory (+1),
        documented default: acetylcholine is the plurality class among
        resolved predictions, so defaulting unresolved neurons to
        excitatory is the smaller assumption than defaulting to inhibitory.

This is a stated modeling assumption, not an upstream ground truth signed
connectome. `sign_policy` is written into the compiled manifest so it is
auditable and can be revisited without re-deriving it from scratch.
"""
import argparse
import sys
from pathlib import Path

import pyarrow.feather as feather
import pyarrow.compute as pc

sys.path.insert(0, str(Path(__file__).parent))
import format as fmt  # noqa: E402

EXCITATORY_NT = {"acetylcholine", "dopamine", "octopamine", "serotonin"}
INHIBITORY_NT = {"glutamate", "gaba", "histamine"}


def load_traced_body_ids(annotations_path: Path) -> list[int]:
    t = feather.read_table(annotations_path, columns=["bodyId", "status"])
    mask = pc.equal(t.column("status"), "Traced")
    ids = t.column("bodyId").filter(mask).to_pylist()
    return sorted(set(ids))


def load_sign_map(neurotransmitters_path: Path) -> dict[int, str]:
    t = feather.read_table(neurotransmitters_path, columns=["body", "consensus_nt"])
    bodies = t.column("body").to_pylist()
    nts = t.column("consensus_nt").to_pylist()
    return dict(zip(bodies, nts))


def sign_for(nt: str | None) -> int:
    if nt in EXCITATORY_NT:
        return 1
    if nt in INHIBITORY_NT:
        return -1
    return 1  # unclear/missing default; see module docstring.


def compile_graph(raw_dir: Path, out_dir: Path) -> dict:
    out_dir.mkdir(parents=True, exist_ok=True)

    body_ids = load_traced_body_ids(raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather")
    dense_index = {body_id: i for i, body_id in enumerate(body_ids)}
    neuron_count = len(body_ids)

    sign_map = load_sign_map(raw_dir / "body-neurotransmitters-male-cns-v1.0.feather")

    weights = feather.read_table(
        raw_dir / "connectome-weights-male-cns-v1.0-minconf-0.5.feather",
        columns=["body_pre", "body_post", "weight"],
    )
    pre = weights.column("body_pre").to_pylist()
    post = weights.column("body_post").to_pylist()
    w = weights.column("weight").to_pylist()

    # Aggregate by (dst_dense, src_dense): a src/dst pair should never
    # appear twice after this step even if the upstream table had
    # duplicates for the same ordered pair.
    agg: dict[tuple[int, int], float] = {}
    for a, b, weight in zip(pre, post, w):
        src = dense_index.get(a)
        dst = dense_index.get(b)
        if src is None or dst is None:
            continue
        sign = sign_for(sign_map.get(a))
        key = (dst, src)
        agg[key] = agg.get(key, 0.0) + sign * float(weight)

    # Destination-major CSC: sort by (dst, src).
    sorted_items = sorted(agg.items())
    offsets = [0] * (neuron_count + 1)
    edge_src: list[int] = []
    edge_weight: list[float] = []
    current_dst = 0
    for (dst, src), signed_weight in sorted_items:
        while current_dst < dst:
            current_dst += 1
            offsets[current_dst] = len(edge_src)
        edge_src.append(src)
        edge_weight.append(signed_weight)
    while current_dst < neuron_count:
        current_dst += 1
        offsets[current_dst] = len(edge_src)

    fmt.write_neurons(out_dir, body_ids)
    fmt.write_offsets(out_dir, offsets)
    blocks = fmt.write_edge_blocks(out_dir, edge_src, edge_weight)
    fmt.write_manifest(
        out_dir,
        dataset="malecns-v1",
        neuron_count=neuron_count,
        edge_count=len(edge_src),
        blocks=blocks,
        sign_policy={
            "excitatory_transmitters": sorted(EXCITATORY_NT),
            "inhibitory_transmitters": sorted(INHIBITORY_NT),
            "unresolved_default": "excitatory",
            "source_column": "consensus_nt",
            "derived_from": "presynaptic (body_pre) neuron",
        },
    )
    return {"neuron_count": neuron_count, "edge_count": len(edge_src)}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)

    stats = compile_graph(args.input, args.output)
    print(f"compiled {stats['neuron_count']} neurons, {stats['edge_count']} edges -> {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

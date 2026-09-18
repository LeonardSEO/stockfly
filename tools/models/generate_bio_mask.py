#!/usr/bin/env python3
"""Generate the StockFly Bio plasticity eligibility mask: one byte per
compiled edge (1 = eligible, 0 = not), in the same destination-major order
as the compiled edge arrays.

Eligibility rule: an edge is Bio-eligible if either its source or
destination neuron belongs to the real MaleCNS mushroom-body / dopaminergic
learning circuit classes -- 'Kenyon_Cell' (mushroom body intrinsic neurons),
'DAN' (dopaminergic neurons), or 'MBON' (mushroom body output neurons).
This is the actual anatomical substrate for associative learning in the
insect brain, not an arbitrary neuron subset (design spec: "primarily
mushroom-body / dopamine-modulated learning circuits").
"""
import argparse
import struct
import sys
from pathlib import Path

import pyarrow.feather as feather
import pyarrow.compute as pc

BIO_CLASSES = {"Kenyon_Cell", "DAN", "MBON"}


def load_dense_index(compiled_dir: Path) -> list[int]:
    raw = (compiled_dir / "neurons.bin").read_bytes()
    return list(struct.unpack(f"<{len(raw)//8}Q", raw))


def load_bio_class_by_body(raw_dir: Path) -> dict[int, bool]:
    t = feather.read_table(
        raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather",
        columns=["bodyId", "status", "class"],
    )
    mask = pc.equal(t.column("status"), "Traced")
    t = t.filter(mask)
    ids = t.column("bodyId").to_pylist()
    classes = t.column("class").to_pylist()
    return {body_id: (cls in BIO_CLASSES) for body_id, cls in zip(ids, classes)}


def load_edges(compiled_dir: Path) -> list[int]:
    """Returns the flat, concatenated edge_src array (dense source indices)
    in the same order as the edge_weight blocks; caller derives dst per
    edge from offsets.bin."""
    src_dir = compiled_dir / "edge_src_blocks"
    edge_src: list[int] = []
    for block_path in sorted(src_dir.glob("*.bin")):
        raw = block_path.read_bytes()
        edge_src.extend(struct.unpack(f"<{len(raw)//4}I", raw))
    return edge_src


def dst_per_edge(compiled_dir: Path, edge_count: int) -> list[int]:
    offsets_raw = (compiled_dir / "offsets.bin").read_bytes()
    offsets = struct.unpack(f"<{len(offsets_raw)//8}Q", offsets_raw)
    dst = [0] * edge_count
    for neuron in range(len(offsets) - 1):
        for e in range(offsets[neuron], offsets[neuron + 1]):
            dst[e] = neuron
    return dst


def generate_bio_mask(raw_dir: Path, compiled_dir: Path) -> dict:
    dense_order = load_dense_index(compiled_dir)
    is_bio_by_body = load_bio_class_by_body(raw_dir)
    is_bio_by_dense = [is_bio_by_body.get(body_id, False) for body_id in dense_order]

    edge_src = load_edges(compiled_dir)
    edge_dst = dst_per_edge(compiled_dir, len(edge_src))

    mask = bytearray(len(edge_src))
    eligible_count = 0
    for i, (src, dst) in enumerate(zip(edge_src, edge_dst)):
        if is_bio_by_dense[src] or is_bio_by_dense[dst]:
            mask[i] = 1
            eligible_count += 1

    (compiled_dir / "bio_mask.bin").write_bytes(bytes(mask))
    return {"edge_count": len(edge_src), "bio_eligible_count": eligible_count}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw", type=Path, required=True)
    parser.add_argument("--compiled", type=Path, required=True)
    args = parser.parse_args(argv)

    stats = generate_bio_mask(args.raw, args.compiled)
    frac = stats["bio_eligible_count"] / stats["edge_count"] * 100
    print(f"bio-eligible edges: {stats['bio_eligible_count']}/{stats['edge_count']} ({frac:.2f}%)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Compile browser brain-rendering geometry, keyed by dense neuron index.

Weekend-MVP scope: both LOD0 and LOD1 are single soma-centroid points per
neuron (from the annotation table's `somaLocation`), NOT full downsampled
skeletons. This keeps the artifact tiny and well within the memory budget
while establishing the real index/format contract that a later, richer
skeleton-based LOD1 compiler can fill in without changing the reader.
A neuron missing a `somaLocation` (~15% of traced neurons, mostly small or
partially-reconstructed fragments) gets zero vertices in both LODs; it still
participates in simulation, it just isn't drawn.

Vertex layout (16 bytes, little-endian): u32 dense_index, f32 x, f32 y, f32 z
Index layout (16 bytes/neuron, little-endian):
    u32 lod0_offset, u32 lod0_count, u32 lod1_offset, u32 lod1_count
(offset/count address vertex *positions*, i.e. multiply by 16 for byte offset)
"""
import argparse
import json
import struct
import sys
from pathlib import Path

import pyarrow.feather as feather
import pyarrow.compute as pc

DEFAULT_LOD0_BUDGET_BYTES = 64 * 1024 * 1024
DEFAULT_LOD1_BUDGET_BYTES = 256 * 1024 * 1024
VERTEX_BYTES = 16
INDEX_ENTRY_BYTES = 16


def load_dense_order(compiled_neurons_path: Path) -> list[int]:
    raw = compiled_neurons_path.read_bytes()
    count = len(raw) // 8
    return list(struct.unpack(f"<{count}Q", raw))


def load_soma_locations(annotations_path: Path) -> dict[int, tuple[float, float, float]]:
    t = feather.read_table(annotations_path, columns=["bodyId", "somaLocation"])
    ids = t.column("bodyId").to_pylist()
    locs = t.column("somaLocation").to_pylist()
    out = {}
    for body_id, loc in zip(ids, locs):
        if loc is not None and len(loc) == 3:
            out[body_id] = (float(loc[0]), float(loc[1]), float(loc[2]))
    return out


def compile_geometry(
    raw_dir: Path,
    compiled_dir: Path,
    lod0_budget: int = DEFAULT_LOD0_BUDGET_BYTES,
    lod1_budget: int = DEFAULT_LOD1_BUDGET_BYTES,
) -> dict:
    dense_order = load_dense_order(compiled_dir / "neurons.bin")
    soma = load_soma_locations(raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather")

    vertices: list[tuple[int, float, float, float]] = []
    index_entries: list[tuple[int, int, int, int]] = []

    for dense_index, body_id in enumerate(dense_order):
        loc = soma.get(body_id)
        if loc is None:
            index_entries.append((0, 0, 0, 0))
            continue
        offset = len(vertices)
        vertices.append((dense_index, *loc))
        # LOD0 and LOD1 share the same single point for this weekend-MVP compiler.
        index_entries.append((offset, 1, offset, 1))

    lod0_bytes = len(vertices) * VERTEX_BYTES
    lod1_bytes = lod0_bytes  # identical content in this MVP compiler
    if lod0_bytes > lod0_budget:
        raise ValueError(
            f"LOD0 geometry would be {lod0_bytes} bytes, exceeding the {lod0_budget}-byte "
            "default budget for 16 GB systems. Pass --lod0-budget-bytes to override."
        )
    if lod1_bytes > lod1_budget:
        raise ValueError(
            f"LOD1 geometry would be {lod1_bytes} bytes, exceeding the {lod1_budget}-byte "
            "default budget for 16 GB systems. Pass --lod1-budget-bytes to override."
        )

    vertex_bytes = b"".join(struct.pack("<Ifff", di, x, y, z) for di, x, y, z in vertices)
    index_bytes = b"".join(struct.pack("<IIII", *entry) for entry in index_entries)

    (compiled_dir / "geometry_lod0.bin").write_bytes(vertex_bytes)
    (compiled_dir / "geometry_lod1.bin").write_bytes(vertex_bytes)
    (compiled_dir / "geometry_index.bin").write_bytes(index_bytes)

    manifest_path = compiled_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["geometry"] = {
        "vertex_bytes": VERTEX_BYTES,
        "index_entry_bytes": INDEX_ENTRY_BYTES,
        "neurons_with_geometry": len(vertices),
        "neurons_total": len(dense_order),
        "lod0_bytes": lod0_bytes,
        "lod1_bytes": lod1_bytes,
        "note": "LOD0/LOD1 are identical soma-centroid points in this weekend-MVP compiler; full skeleton LOD1 is deferred.",
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")

    return manifest["geometry"]


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True, help="raw MaleCNS dir (for annotations)")
    parser.add_argument("--compiled", type=Path, required=True, help="compiled graph dir (for neurons.bin + manifest.json)")
    parser.add_argument("--lod0-budget-bytes", type=int, default=DEFAULT_LOD0_BUDGET_BYTES)
    parser.add_argument("--lod1-budget-bytes", type=int, default=DEFAULT_LOD1_BUDGET_BYTES)
    args = parser.parse_args(argv)

    stats = compile_geometry(args.input, args.compiled, args.lod0_budget_bytes, args.lod1_budget_bytes)
    print(f"compiled geometry for {stats['neurons_with_geometry']}/{stats['neurons_total']} neurons -> {args.compiled}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

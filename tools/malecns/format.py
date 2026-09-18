"""Binary writer for the compiled StockFly connectome artifact.

Layout (all integers little-endian, matching Rust's native x86_64/aarch64
byte order so the Rust reader can read blocks with zero-copy `bytemuck`):

  manifest.json           — see write_manifest()
  neurons.bin              — u64 original body ID per dense index, ordered
                              ascending by body ID (dense index == array pos)
  offsets.bin               — u64[neuron_count + 1] CSC offsets into the
                              concatenated edge arrays (destination-major:
                              offsets[j]..offsets[j+1] is neuron j's incoming
                              edges)
  edge_src_blocks/NNNN.bin  — u32 source dense index, chunked to <= 64 MiB
  edge_weight_blocks/NNNN.bin — f32 signed magnitude (sign baked in),
                              chunked identically to edge_src_blocks

Chunking is purely for the 64 MiB browser storage-buffer limit; blocks
concatenate back into one logical CSC edge array in file order.
"""
import hashlib
import json
import struct
from pathlib import Path

MAX_BLOCK_BYTES = 64 * 1024 * 1024
EDGES_PER_BLOCK = MAX_BLOCK_BYTES // 4  # 4 bytes per u32 src index (the tighter of the two arrays)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        while True:
            chunk = fh.read(8 * 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def write_neurons(out_dir: Path, body_ids: list[int]) -> None:
    with open(out_dir / "neurons.bin", "wb") as fh:
        fh.write(struct.pack(f"<{len(body_ids)}Q", *body_ids))


def write_offsets(out_dir: Path, offsets: list[int]) -> None:
    with open(out_dir / "offsets.bin", "wb") as fh:
        fh.write(struct.pack(f"<{len(offsets)}Q", *offsets))


def write_edge_blocks(out_dir: Path, edge_src: list[int], edge_weight: list[float]) -> list[dict]:
    assert len(edge_src) == len(edge_weight)
    src_dir = out_dir / "edge_src_blocks"
    weight_dir = out_dir / "edge_weight_blocks"
    src_dir.mkdir(parents=True, exist_ok=True)
    weight_dir.mkdir(parents=True, exist_ok=True)

    blocks = []
    n = len(edge_src)
    block_index = 0
    for start in range(0, max(n, 1), EDGES_PER_BLOCK):
        end = min(start + EDGES_PER_BLOCK, n)
        count = end - start
        src_path = src_dir / f"{block_index:04d}.bin"
        weight_path = weight_dir / f"{block_index:04d}.bin"
        with open(src_path, "wb") as fh:
            fh.write(struct.pack(f"<{count}I", *edge_src[start:end]))
        with open(weight_path, "wb") as fh:
            fh.write(struct.pack(f"<{count}f", *edge_weight[start:end]))
        blocks.append({
            "index": block_index,
            "edge_start": start,
            "edge_count": count,
            "src_sha256": sha256_file(src_path),
            "weight_sha256": sha256_file(weight_path),
        })
        block_index += 1
        if n == 0:
            break
    return blocks


def write_manifest(out_dir: Path, *, dataset: str, neuron_count: int, edge_count: int, blocks: list[dict], sign_policy: dict) -> None:
    manifest = {
        "format_version": 1,
        "dataset": dataset,
        "neuron_count": neuron_count,
        "edge_count": edge_count,
        "neurons_sha256": sha256_file(out_dir / "neurons.bin"),
        "offsets_sha256": sha256_file(out_dir / "offsets.bin"),
        "edge_blocks": blocks,
        "sign_policy": sign_policy,
    }
    with open(out_dir / "manifest.json", "w") as fh:
        json.dump(manifest, fh, indent=2, sort_keys=True)
        fh.write("\n")

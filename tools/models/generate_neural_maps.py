#!/usr/bin/env python3
"""Generate the fixed, versioned sensory-map.json and output-map.json
resources from the real compiled MaleCNS graph and raw annotations.

Both maps are project *source*, not trained artifacts: they are generated
once, committed, and any change requires a format-version bump (see the
design spec's "Chess output encoding" and "Chess sensory encoding"
sections). Regenerating with the same inputs must be byte-for-byte
deterministic.

Population choices (documented, not hidden):

- Board/visual input: MaleCNS `superclass == 'ol_sensory'` (optic-lobe
  sensory neurons -- i.e. real photoreceptor-class visual input neurons),
  4,114 available in the traced v1.0 graph. 8 neurons are assigned per
  board square (64 * 8 = 512), sorted by dense index for reproducibility.
- Context channels (side to move, 4 castling rights, 8 en-passant files):
  MaleCNS `superclass == 'cb_sensory'` (central-brain sensory neurons,
  distinct from the visual population above), 1 neuron per binary/one-hot
  context signal, 13 neurons total.
- Output (from/to/promotion): MaleCNS `superclass == 'descending_neuron'`
  (the real anatomical motor-output pathway from brain to nerve cord),
  1,314 available. Deterministically shuffled with a fixed seed and
  partitioned into 64 from-groups + 64 to-groups + 4 promotion groups
  (132 groups), balanced as evenly as 1,314 neurons allow.

Deterministic seed: 0x53544F434B464C59 ("STOCKFLY" in ASCII hex-packed).
"""
import argparse
import json
import random
import struct
import sys
from pathlib import Path

import pyarrow.feather as feather
import pyarrow.compute as pc

SEED = 0x53544F434B464C59
PIECE_IDENTITIES = [
    "P", "N", "B", "R", "Q", "K",  # white
    "p", "n", "b", "r", "q", "k",  # black
]
NEURONS_PER_SQUARE = 8
PROMOTION_PIECES = ["queen", "rook", "bishop", "knight"]


def load_dense_index(compiled_dir: Path) -> dict[int, int]:
    raw = (compiled_dir / "neurons.bin").read_bytes()
    body_ids = struct.unpack(f"<{len(raw)//8}Q", raw)
    return {body_id: i for i, body_id in enumerate(body_ids)}


def load_superclass_bodies(raw_dir: Path, superclass: str) -> list[int]:
    t = feather.read_table(
        raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather",
        columns=["bodyId", "status", "superclass"],
    )
    mask = pc.and_(pc.equal(t.column("status"), "Traced"), pc.equal(t.column("superclass"), superclass))
    return sorted(t.column("bodyId").filter(mask).to_pylist())


def fixed_piece_embedding(piece_index: int, dims: int = NEURONS_PER_SQUARE) -> list[float]:
    """A fixed, reproducible bipolar spectral code per piece identity:
    deterministic from `piece_index` and the global SEED, independent of
    which neurons end up assigned to a square."""
    rng = random.Random(SEED ^ (piece_index * 0x9E3779B1))
    return [1.0 if rng.random() < 0.5 else -1.0 for _ in range(dims)]


def build_sensory_map(raw_dir: Path, compiled_dir: Path) -> dict:
    dense_index = load_dense_index(compiled_dir)

    ol_sensory = load_superclass_bodies(raw_dir, "ol_sensory")
    cb_sensory = load_superclass_bodies(raw_dir, "cb_sensory")

    needed_board = 64 * NEURONS_PER_SQUARE
    if len(ol_sensory) < needed_board:
        raise ValueError(f"only {len(ol_sensory)} ol_sensory neurons available, need {needed_board}")
    needed_context = 1 + 4 + 8  # side-to-move + castling rights + en-passant file one-hot
    if len(cb_sensory) < needed_context:
        raise ValueError(f"only {len(cb_sensory)} cb_sensory neurons available, need {needed_context}")

    squares = []
    for sq in range(64):
        bodies = ol_sensory[sq * NEURONS_PER_SQUARE:(sq + 1) * NEURONS_PER_SQUARE]
        squares.append([dense_index[b] for b in bodies])

    piece_embeddings = {piece: fixed_piece_embedding(i) for i, piece in enumerate(PIECE_IDENTITIES)}

    context_bodies = cb_sensory[:needed_context]
    context = {
        "side_to_move": dense_index[context_bodies[0]],
        "castling_rights": [dense_index[b] for b in context_bodies[1:5]],  # [K, Q, k, q]
        "en_passant_file": [dense_index[b] for b in context_bodies[5:13]],  # a..h one-hot
    }

    return {
        "format_version": 1,
        "seed": SEED,
        "neurons_per_square": NEURONS_PER_SQUARE,
        "board_populations": squares,
        "piece_embeddings": piece_embeddings,
        "context": context,
        "source_populations": {
            "board": "superclass == 'ol_sensory'",
            "context": "superclass == 'cb_sensory'",
        },
    }


def build_output_map(raw_dir: Path, compiled_dir: Path) -> dict:
    dense_index = load_dense_index(compiled_dir)
    descending = load_superclass_bodies(raw_dir, "descending_neuron")

    needed_groups = 64 + 64 + 4
    if len(descending) < needed_groups:
        raise ValueError(f"only {len(descending)} descending_neuron available, need at least {needed_groups} groups")

    rng = random.Random(SEED)
    shuffled = descending[:]
    rng.shuffle(shuffled)

    # Balanced partition into 132 groups, each non-empty.
    groups: list[list[int]] = [[] for _ in range(needed_groups)]
    for i, body in enumerate(shuffled):
        groups[i % needed_groups].append(body)

    from_groups = groups[0:64]
    to_groups = groups[64:128]
    promotion_groups = groups[128:132]

    return {
        "format_version": 1,
        "seed": SEED,
        "source_population": "superclass == 'descending_neuron'",
        "from_groups": [[dense_index[b] for b in g] for g in from_groups],
        "to_groups": [[dense_index[b] for b in g] for g in to_groups],
        "promotion_groups": {name: [dense_index[b] for b in g] for name, g in zip(PROMOTION_PIECES, promotion_groups)},
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw", type=Path, required=True)
    parser.add_argument("--compiled", type=Path, required=True)
    parser.add_argument("--out-sensory", type=Path, required=True)
    parser.add_argument("--out-output", type=Path, required=True)
    args = parser.parse_args(argv)

    sensory_map = build_sensory_map(args.raw, args.compiled)
    args.out_sensory.write_text(json.dumps(sensory_map, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.out_sensory}")

    output_map = build_output_map(args.raw, args.compiled)
    args.out_output.write_text(json.dumps(output_map, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.out_output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

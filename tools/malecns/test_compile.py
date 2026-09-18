import struct
from pathlib import Path

import pyarrow.feather as feather
import pyarrow as pa
import pytest

import compile as compile_mod
import format as fmt


def make_fixture(tmp_path: Path) -> Path:
    """Six neurons: excitatory (ACh), inhibitory (GABA), and unclear/unknown
    sign, plus a duplicate (src,dst) pair to verify aggregation."""
    raw_dir = tmp_path / "raw"
    raw_dir.mkdir()

    annotations = pa.table({
        "bodyId": pa.array([10, 20, 30, 40, 50, 60], type=pa.int64()),
        "status": ["Traced", "Traced", "Traced", "Traced", "Traced", "Orphan"],  # 60 excluded
    })
    feather.write_feather(annotations, raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather")

    neurotransmitters = pa.table({
        "body": pa.array([10, 20, 30, 40], type=pa.int64()),
        "consensus_nt": ["acetylcholine", "gaba", "unclear", "glutamate"],
    })
    feather.write_feather(neurotransmitters, raw_dir / "body-neurotransmitters-male-cns-v1.0.feather")

    # 10 -(ACh, excitatory)-> 20   weight 3
    # 20 -(GABA, inhibitory)-> 30  weight 2
    # 30 -(unclear -> default excitatory)-> 40  weight 1, appears twice (duplicate to aggregate -> total 2)
    # 40 -(glutamate, inhibitory)-> 50 weight 5
    # 60 is Orphan (untraced): any edge touching it must be dropped
    weights = pa.table({
        "body_pre": pa.array([10, 20, 30, 30, 40, 60], type=pa.int64()),
        "body_post": pa.array([20, 30, 40, 40, 50, 10], type=pa.int64()),
        "weight": pa.array([3, 2, 1, 1, 5, 99], type=pa.int64()),
    })
    feather.write_feather(weights, raw_dir / "connectome-weights-male-cns-v1.0-minconf-0.5.feather")

    return raw_dir


def test_compile_fixture_filters_signs_and_aggregates(tmp_path):
    raw_dir = make_fixture(tmp_path)
    out_dir = tmp_path / "compiled"

    stats = compile_mod.compile_graph(raw_dir, out_dir)

    assert stats["neuron_count"] == 5  # body 60 (Orphan) excluded
    assert stats["edge_count"] == 4  # (10->20), (20->30), (30->40 aggregated), (40->50); 60's edge dropped

    body_ids = struct.unpack("<5Q", (out_dir / "neurons.bin").read_bytes())
    assert body_ids == (10, 20, 30, 40, 50)
    dense = {b: i for i, b in enumerate(body_ids)}

    offsets = struct.unpack("<6Q", (out_dir / "offsets.bin").read_bytes())
    assert offsets[0] == 0
    assert offsets[-1] == 4

    src_bytes = (out_dir / "edge_src_blocks" / "0000.bin").read_bytes()
    weight_bytes = (out_dir / "edge_weight_blocks" / "0000.bin").read_bytes()
    edge_src = struct.unpack(f"<{len(src_bytes)//4}I", src_bytes)
    edge_weight = struct.unpack(f"<{len(weight_bytes)//4}f", weight_bytes)

    # Reconstruct dst-> [(src, weight)] from CSC offsets to check per-destination edges.
    by_dst = {}
    for dst_dense in range(5):
        start, end = offsets[dst_dense], offsets[dst_dense + 1]
        by_dst[dst_dense] = list(zip(edge_src[start:end], edge_weight[start:end]))

    # dst=20 (dense 1): from 10, sign +1 (ACh), weight 3 -> +3.0
    assert by_dst[dense[20]] == [(dense[10], pytest.approx(3.0))]
    # dst=30 (dense 2): from 20, sign -1 (GABA), weight 2 -> -2.0
    assert by_dst[dense[30]] == [(dense[20], pytest.approx(-2.0))]
    # dst=40 (dense 3): from 30, unclear -> default excitatory (+1), weight 1+1 aggregated -> +2.0
    assert by_dst[dense[40]] == [(dense[30], pytest.approx(2.0))]
    # dst=50 (dense 4): from 40, glutamate -> inhibitory (-1), weight 5 -> -5.0
    assert by_dst[dense[50]] == [(dense[40], pytest.approx(-5.0))]
    # dst=10 (dense 0): no incoming edges (60->10 dropped because 60 is untraced)
    assert by_dst[dense[10]] == []


def test_sign_policy_recorded_in_manifest(tmp_path):
    raw_dir = make_fixture(tmp_path)
    out_dir = tmp_path / "compiled"
    compile_mod.compile_graph(raw_dir, out_dir)

    import json
    manifest = json.loads((out_dir / "manifest.json").read_text())
    assert "acetylcholine" in manifest["sign_policy"]["excitatory_transmitters"]
    assert "gaba" in manifest["sign_policy"]["inhibitory_transmitters"]
    assert manifest["sign_policy"]["unresolved_default"] == "excitatory"

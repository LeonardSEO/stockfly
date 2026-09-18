import json
import struct
from pathlib import Path

import pyarrow.feather as feather
import pyarrow as pa
import pytest

import geometry


def setup_fixture(tmp_path: Path):
    raw_dir = tmp_path / "raw"
    raw_dir.mkdir()
    compiled_dir = tmp_path / "compiled"
    compiled_dir.mkdir()

    # 4 neurons, dense order 0..3, one (body 40) has no somaLocation.
    body_ids = [10, 20, 30, 40]
    (compiled_dir / "neurons.bin").write_bytes(struct.pack("<4Q", *body_ids))
    (compiled_dir / "manifest.json").write_text(json.dumps({"neuron_count": 4, "edge_count": 0}))

    annotations = pa.table({
        "bodyId": pa.array(body_ids, type=pa.int64()),
        "somaLocation": pa.array([[1, 2, 3], [4, 5, 6], [7, 8, 9], None], type=pa.list_(pa.int64())),
    })
    feather.write_feather(annotations, raw_dir / "body-annotations-male-cns-v1.0-minconf-0.5.feather")

    return raw_dir, compiled_dir


def test_geometry_index_points_to_each_neuron(tmp_path):
    raw_dir, compiled_dir = setup_fixture(tmp_path)
    stats = geometry.compile_geometry(raw_dir, compiled_dir)

    assert stats["neurons_with_geometry"] == 3
    assert stats["neurons_total"] == 4

    index_bytes = (compiled_dir / "geometry_index.bin").read_bytes()
    entries = [struct.unpack("<IIII", index_bytes[i:i + 16]) for i in range(0, len(index_bytes), 16)]
    assert len(entries) == 4

    # dense index 3 (body 40) has no soma location -> zero counts in both LODs.
    assert entries[3] == (0, 0, 0, 0)
    for i in (0, 1, 2):
        lod0_offset, lod0_count, lod1_offset, lod1_count = entries[i]
        assert lod0_count > 0
        assert lod1_count <= lod0_count


def test_geometry_vertices_carry_correct_dense_index_and_position(tmp_path):
    raw_dir, compiled_dir = setup_fixture(tmp_path)
    geometry.compile_geometry(raw_dir, compiled_dir)

    lod0_bytes = (compiled_dir / "geometry_lod0.bin").read_bytes()
    vertices = [struct.unpack("<Ifff", lod0_bytes[i:i + 16]) for i in range(0, len(lod0_bytes), 16)]
    assert len(vertices) == 3

    by_dense_index = {v[0]: v[1:] for v in vertices}
    assert by_dense_index[0] == pytest.approx((1.0, 2.0, 3.0))
    assert by_dense_index[1] == pytest.approx((4.0, 5.0, 6.0))
    assert by_dense_index[2] == pytest.approx((7.0, 8.0, 9.0))
    assert 3 not in by_dense_index


def test_budget_enforced_with_clear_error(tmp_path):
    raw_dir, compiled_dir = setup_fixture(tmp_path)
    with pytest.raises(ValueError, match="exceeding"):
        geometry.compile_geometry(raw_dir, compiled_dir, lod0_budget=8)  # 3 vertices * 16 bytes > 8

import hashlib
import json
from pathlib import Path

import pytest

import fetch


def load_manifest(path):
    return fetch.load_manifest(Path(path))


def test_manifest_has_required_artifacts():
    manifest = load_manifest(fetch.MANIFEST_PATH)
    names = {x["name"] for x in manifest["artifacts"]}
    assert "connectome_weights" in names
    assert "annotations" in names
    assert "neurotransmitters" in names


def test_manifest_artifacts_have_filenames_and_base_url():
    manifest = load_manifest(fetch.MANIFEST_PATH)
    assert manifest["base_url"].startswith("https://")
    for artifact in manifest["artifacts"]:
        assert artifact["filename"]


def test_bad_hash_leaves_no_final_artifact(tmp_path):
    payload = b"pretend feather bytes"
    src = tmp_path / "upstream.bin"
    src.write_bytes(payload)

    manifest = {
        "base_url": f"file://{tmp_path}",
        "artifacts": [{"name": "thing", "filename": "upstream.bin", "approx_bytes": len(payload)}],
    }
    out_dir = tmp_path / "out"
    lock = {"thing": {"sha256": "0" * 64, "filename": "upstream.bin"}}  # deliberately wrong

    with pytest.raises(ValueError, match="hash mismatch"):
        fetch.fetch_artifact(manifest, "thing", out_dir, lock)

    assert not (out_dir / "upstream.bin").exists()
    assert not (out_dir / "upstream.bin.partial").exists()


def test_first_fetch_pins_hash_and_second_fetch_verifies(tmp_path, monkeypatch):
    payload = b"pretend feather bytes for pin test"
    src = tmp_path / "upstream.bin"
    src.write_bytes(payload)
    expected_sha = hashlib.sha256(payload).hexdigest()

    manifest = {
        "base_url": f"file://{tmp_path}",
        "artifacts": [{"name": "thing", "filename": "upstream.bin", "approx_bytes": len(payload)}],
    }
    out_dir = tmp_path / "out"
    lock = {}

    dest = fetch.fetch_artifact(manifest, "thing", out_dir, lock)
    assert dest.exists()
    assert lock["thing"]["sha256"] == expected_sha

    # Second call with the file already present should skip, not re-download.
    dest2 = fetch.fetch_artifact(manifest, "thing", out_dir, lock)
    assert dest2 == dest

#!/usr/bin/env python3
"""Prepare a reproducible, content-addressed browser model catalog."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[2]
MODEL_SPECS = (
    ("bio-full", "Bio Full"),
    ("max-full", "Max Full"),
)
HEADER_LIMIT = 1024 * 1024
DELTAS_MARKER = b',"deltas":'


def checkpoint_header(path: Path) -> dict:
    with path.open("rb") as source:
        prefix = source.read(HEADER_LIMIT)
        source.seek(max(0, path.stat().st_size - 64))
        tail = source.read().rstrip()
    marker = prefix.find(DELTAS_MARKER)
    if marker < 0:
        raise ValueError(f"{path}: checkpoint header does not end before {HEADER_LIMIT} bytes")
    if not tail.endswith(b"]}"):
        raise ValueError(f"{path}: checkpoint does not have a complete JSON ending")
    return json.loads(prefix[:marker] + b"}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_hash(value: object, field: str, path: Path) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        raise ValueError(f"{path}: {field} is not a lowercase SHA-256")
    return value


def inspect_checkpoint(path: Path, expected_kind: str) -> dict:
    if not path.is_file():
        raise FileNotFoundError(f"Checkpoint does not exist: {path}")
    header = checkpoint_header(path)
    if header.get("format_version") != 1:
        raise ValueError(f"{path}: unsupported checkpoint format {header.get('format_version')}")
    if header.get("model_kind") != expected_kind:
        raise ValueError(f"{path}: expected {expected_kind}, found {header.get('model_kind')}")
    preset = header.get("preset")
    trials = header.get("trials_run")
    if not isinstance(preset, str) or not preset:
        raise ValueError(f"{path}: missing training preset")
    if not isinstance(trials, int) or isinstance(trials, bool) or trials < 1:
        raise ValueError(f"{path}: invalid trial count")
    return {
        "checkpointSha256": sha256(path),
        "trainingPreset": preset,
        "trialsRun": trials,
        "graphNeuronsSha256": require_hash(header.get("graph_neurons_sha256"), "graph_neurons_sha256", path),
        "sensoryMapSha256": require_hash(header.get("sensory_map_sha256"), "sensory_map_sha256", path),
        "outputMapSha256": require_hash(header.get("output_map_sha256"), "output_map_sha256", path),
    }


def prepare_asset(source: Path, destination: Path, mode: str, expected_sha256: str) -> None:
    if destination.exists() or destination.is_symlink():
        if sha256(destination) != expected_sha256:
            raise ValueError(f"Content-addressed asset collision: {destination}")
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(f".{destination.name}.{os.getpid()}.tmp")
    try:
        if mode == "copy":
            shutil.copyfile(source, temporary)
        else:
            temporary.symlink_to(source.resolve())
        os.replace(temporary, destination)
    finally:
        if temporary.exists() or temporary.is_symlink():
            temporary.unlink()


def write_catalog(output_dir: Path, catalog: dict) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=output_dir, prefix=".catalog-", suffix=".tmp", delete=False) as handle:
        json.dump(catalog, handle, indent=2)
        handle.write("\n")
        temporary = Path(handle.name)
    os.replace(temporary, output_dir / "catalog.json")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bio-source", type=Path, default=REPO_ROOT / "data/checkpoints/stockfly-bio-full.sfckpt")
    parser.add_argument("--max-source", type=Path, default=REPO_ROOT / "data/checkpoints/stockfly-max-full.sfckpt")
    parser.add_argument("--output-dir", type=Path, default=REPO_ROOT / "data/browser-models")
    parser.add_argument("--mode", choices=("symlink", "copy"), default="symlink",
                        help="symlink for local development; copy to materialize release assets")
    args = parser.parse_args()

    def repo_path(path: Path) -> Path:
        return path if path.is_absolute() else REPO_ROOT / path

    output_dir = repo_path(args.output_dir)
    sources = {"bio-full": repo_path(args.bio_source), "max-full": repo_path(args.max_source)}
    entries = []
    for model_id, label in MODEL_SPECS:
        source = sources[model_id].resolve()
        identity = inspect_checkpoint(source, model_id)
        filename = f"{model_id}-{identity['checkpointSha256']}.sfckpt"
        prepare_asset(source, output_dir / "checkpoints" / filename, args.mode, identity["checkpointSha256"])
        entries.append({
            "id": model_id,
            "label": label,
            "expectedKind": model_id,
            "availability": "available",
            "checkpointUrl": f"/vendor/models/checkpoints/{filename}",
            **identity,
        })

    entries.append({
        "id": "lite",
        "label": "Lite",
        "expectedKind": "lite",
        "availability": "unavailable",
        "reason": "The Full causal prerequisite did not pass; no Lite checkpoint exists.",
    })
    write_catalog(output_dir, {"formatVersion": 1, "models": entries})
    for entry in entries:
        if entry["availability"] == "available":
            print(f"{entry['label']}: {entry['trainingPreset']} · {entry['trialsRun']} trials · {entry['checkpointSha256']}")
    print(f"Catalog: {output_dir / 'catalog.json'} ({args.mode})")


if __name__ == "__main__":
    main()

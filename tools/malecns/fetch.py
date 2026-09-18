#!/usr/bin/env python3
"""Reproducible, hash-verified fetcher for the official Janelia MaleCNS v1.0
dataset. Downloads only missing artifacts, streams SHA-256 while writing to
a `.partial` file, and atomically renames only after the hash matches a
locally pinned "known good" value (there is no upstream-published checksum
for these files, so the first verified download pins the hash for future
runs — see `manifest.lock.json`).

Raw MaleCNS files are never committed to the repository (see .gitignore).
"""
import argparse
import hashlib
import json
import sys
from pathlib import Path
from urllib.request import urlopen

HERE = Path(__file__).parent
MANIFEST_PATH = HERE / "manifest.json"
LOCK_PATH = HERE / "manifest.lock.json"
DEFAULT_OUT_DIR = HERE.parent.parent / "data" / "raw" / "malecns-v1"

CHUNK_SIZE = 8 * 1024 * 1024


def load_manifest(path: Path = MANIFEST_PATH) -> dict:
    with open(path) as fh:
        return json.load(fh)


def load_lock(path: Path = LOCK_PATH) -> dict:
    if not path.exists():
        return {}
    with open(path) as fh:
        return json.load(fh)


def save_lock(lock: dict, path: Path = LOCK_PATH) -> None:
    with open(path, "w") as fh:
        json.dump(lock, fh, indent=2, sort_keys=True)
        fh.write("\n")


def artifact_by_name(manifest: dict, name: str) -> dict:
    for artifact in manifest["artifacts"]:
        if artifact["name"] == name:
            return artifact
    raise KeyError(f"unknown artifact: {name}")


def stream_download(url: str, dest_partial: Path) -> str:
    """Stream `url` into `dest_partial`, returning the SHA-256 hex digest."""
    digest = hashlib.sha256()
    dest_partial.parent.mkdir(parents=True, exist_ok=True)
    with urlopen(url) as response, open(dest_partial, "wb") as out:
        while True:
            chunk = response.read(CHUNK_SIZE)
            if not chunk:
                break
            digest.update(chunk)
            out.write(chunk)
    return digest.hexdigest()


def fetch_artifact(manifest: dict, name: str, out_dir: Path, lock: dict, force: bool = False) -> Path:
    artifact = artifact_by_name(manifest, name)
    filename = artifact["filename"]
    dest = out_dir / filename
    dest_partial = out_dir / f"{filename}.partial"

    if dest.exists() and not force:
        print(f"[skip] {name}: {dest} already present")
        return dest

    url = f"{manifest['base_url']}/{filename}"
    print(f"[fetch] {name}: {url}")
    actual_sha256 = stream_download(url, dest_partial)

    pinned = lock.get(name)
    if pinned is None:
        print(f"[pin] {name}: no pinned hash yet, trusting this first download ({actual_sha256})")
        lock[name] = {"sha256": actual_sha256, "filename": filename}
    elif pinned["sha256"] != actual_sha256:
        dest_partial.unlink(missing_ok=True)
        raise ValueError(
            f"hash mismatch for {name}: expected {pinned['sha256']}, got {actual_sha256}. "
            "Refusing to finalize a changed upstream asset."
        )

    dest_partial.rename(dest)
    print(f"[ok] {name}: verified sha256={actual_sha256}")
    return dest


def cmd_list(manifest: dict) -> None:
    total = sum(a.get("approx_bytes", 0) for a in manifest["artifacts"])
    print(f"{len(manifest['artifacts'])} artifacts, ~{total / 1e9:.2f} GB total:")
    for artifact in manifest["artifacts"]:
        size_mb = artifact.get("approx_bytes", 0) / 1e6
        print(f"  {artifact['name']:<20} {artifact['filename']:<60} ~{size_mb:.1f} MB")


def cmd_verify(manifest: dict, out_dir: Path, lock: dict) -> bool:
    ok = True
    for artifact in manifest["artifacts"]:
        name = artifact["name"]
        dest = out_dir / artifact["filename"]
        pinned = lock.get(name)
        if not dest.exists():
            print(f"[missing] {name}: {dest}")
            ok = False
            continue
        if pinned is None:
            print(f"[unverified] {name}: present but no pinned hash recorded")
            continue
        digest = hashlib.sha256()
        with open(dest, "rb") as fh:
            while True:
                chunk = fh.read(CHUNK_SIZE)
                if not chunk:
                    break
                digest.update(chunk)
        actual = digest.hexdigest()
        if actual == pinned["sha256"]:
            print(f"[ok] {name}: {actual}")
        else:
            print(f"[FAIL] {name}: expected {pinned['sha256']}, got {actual}")
            ok = False
    return ok


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=MANIFEST_PATH)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument("--list", action="store_true", help="list artifacts and sizes, then exit")
    parser.add_argument("--verify", action="store_true", help="verify already-downloaded artifacts against the pinned hash lock, then exit")
    parser.add_argument("--fetch", action="append", default=[], help="artifact name to fetch (repeatable); omit to fetch all")
    parser.add_argument("--force", action="store_true", help="re-download even if the file already exists")
    args = parser.parse_args(argv)

    manifest = load_manifest(args.manifest)

    if args.list:
        cmd_list(manifest)
        return 0

    lock = load_lock()

    if args.verify:
        ok = cmd_verify(manifest, args.out, lock)
        return 0 if ok else 1

    names = args.fetch or [a["name"] for a in manifest["artifacts"]]
    for name in names:
        fetch_artifact(manifest, name, args.out, lock, force=args.force)
    save_lock(lock)
    return 0


if __name__ == "__main__":
    sys.exit(main())

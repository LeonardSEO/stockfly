#!/usr/bin/env python3
"""Fetch or copy the pinned Stockfish 19 Lite browser assets with hash checks."""
import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Optional
from urllib.request import urlopen

VERSION = "v19.0.0"
BASE = f"https://github.com/nmrugg/stockfish.js/releases/download/{VERSION}"
REPO_ROOT = Path(__file__).resolve().parents[2]
FILES = {
    "stockfish-js-v19.0.0-source.tar.gz": (
        "https://codeload.github.com/nmrugg/stockfish.js/tar.gz/refs/tags/v19.0.0",
        "183b576fa9c8610a9be48c6e5b97506502985e64a771e769a16dbecf2f125d54",
    ),
    "stockfish-19-lite-single.js": (
        f"{BASE}/stockfish-19-lite-single.js",
        "d3344124ab067fb0b90ee77873bb8e9fbf5fc01bc525fe714b0f942581e889e6",
    ),
    "stockfish-19-lite-single.wasm": (
        f"{BASE}/stockfish-19-lite-single.wasm",
        "57ac2d72312aba346760e3f173f687a8c211208e97a87268436f7f0e10bb5387",
    ),
    "Copying.txt": (
        f"https://raw.githubusercontent.com/nmrugg/stockfish.js/{VERSION}/Copying.txt",
        "0b383d5a63da644f628d99c33976ea6487ed89aaa59f0b3257992deac1171e6b",
    ),
    "AUTHORS": (
        f"https://raw.githubusercontent.com/nmrugg/stockfish.js/{VERSION}/AUTHORS",
        "f81e7d74c350626766382ec67fa262c38cda00cb8b501790f95cdebb526006c1",
    ),
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def atomic_write(destination: Path, data: bytes) -> None:
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=f".{destination.name}.", suffix=".tmp", delete=False) as handle:
            temporary = Path(handle.name)
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def materialize(out: Path, source: Optional[Path], offline: bool) -> None:
    out.mkdir(parents=True, exist_ok=True)
    sources = []
    for name, (url, expected) in FILES.items():
        destination = out / name
        local = source / name if source else None
        if destination.exists() and digest(destination.read_bytes()) == expected:
            data = destination.read_bytes()
            origin = "verified cache"
        elif local and local.exists():
            data = local.read_bytes()
            origin = str(local)
        elif offline:
            raise FileNotFoundError(f"Offline source is missing {name}")
        else:
            with urlopen(url) as response:
                data = response.read()
            origin = url
        actual = digest(data)
        if actual != expected:
            raise ValueError(f"SHA-256 mismatch for {name}: expected {expected}, got {actual}")
        atomic_write(destination, data)
        sources.append({"file": name, "url": url, "sha256": expected})
        print(f"{name}: {origin} ({actual})")

    notice = (
        "Stockfish.js 19.0.0 Lite single-thread build\n"
        "Source: https://github.com/nmrugg/stockfish.js/tree/v19.0.0\n"
        "Corresponding source and build instructions: stockfish-js-v19.0.0-source.tar.gz (build.js and README.md).\n"
        "License: GNU General Public License; see Copying.txt in this directory.\n"
        "The StockFly project is not affiliated with the Stockfish project.\n"
    )
    atomic_write(out / "NOTICE.txt", notice.encode("utf-8"))
    atomic_write(out / "sources.json", (json.dumps({"version": VERSION, "files": sources}, indent=2) + "\n").encode("utf-8"))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=REPO_ROOT / "data/vendor/stockfish-19-lite")
    parser.add_argument("--source-dir", type=Path, help="Use an existing offline asset directory before network fetch")
    parser.add_argument("--offline", action="store_true", help="Fail instead of fetching missing source files")
    args = parser.parse_args()
    materialize(args.out, args.source_dir, args.offline)

#!/usr/bin/env python3
"""Fetch or copy the pinned Stockfish 19 Lite browser assets with hash checks."""
import argparse
import hashlib
import json
from pathlib import Path
from typing import Optional
from urllib.request import urlopen

VERSION = "v19.0.0"
BASE = f"https://github.com/nmrugg/stockfish.js/releases/download/{VERSION}"
REPO_ROOT = Path(__file__).resolve().parents[2]
FILES = {
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
        destination.write_bytes(data)
        sources.append({"file": name, "url": url, "sha256": expected})
        print(f"{name}: {origin} ({actual})")

    notice = (
        "Stockfish.js 19.0.0 Lite single-thread build\n"
        "Source: https://github.com/nmrugg/stockfish.js/tree/v19.0.0\n"
        "License: GNU General Public License; see Copying.txt in this directory.\n"
        "The StockFly project is not affiliated with the Stockfish project.\n"
    )
    (out / "NOTICE.txt").write_text(notice)
    (out / "sources.json").write_text(json.dumps({"version": VERSION, "files": sources}, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=REPO_ROOT / "data/vendor/stockfish-19-lite")
    parser.add_argument("--source-dir", type=Path, help="Use an existing offline asset directory before network fetch")
    parser.add_argument("--offline", action="store_true", help="Fail instead of fetching missing source files")
    args = parser.parse_args()
    materialize(args.out, args.source_dir, args.offline)

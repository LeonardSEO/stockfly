# Third-Party Notices

StockFly builds on two external works. Neither is vendored as raw data in
this repository; both are fetched by scripts at build/train time and their
terms are preserved below.

## Janelia MaleCNS v1.0 connectome

- **Source:** Janelia Research Campus, Male Adult Nerve Cord/Brain connectome
  ("MaleCNS") v1.0.
- **License:** CC-BY (attribution required).
- **What we do with it:** `tools/malecns/fetch.py` downloads the official
  connectivity, annotation, neurotransmitter, and geometry files listed in
  `tools/malecns/manifest.json` directly from the upstream source. Raw files
  are never committed to this repository (`data/raw/` is git-ignored).
  `tools/malecns/compile.py` derives a compact CSC graph and geometry
  artifact used at runtime; these derived artifacts preserve original body
  IDs and are distributed as GitHub Release assets with this same
  attribution notice embedded in their manifest.
- **Attribution:** Please cite the MaleCNS connectome and Janelia Research
  Campus / FlyEM project when redistributing or publishing results derived
  from this data.

## Stockfish 19 (Lite build)

- **Source:** Stockfish chess engine, https://stockfishchess.org
- **License:** GNU General Public License v3.0 (GPLv3).
- **What we do with it:** Stockfish 19 Lite (single-threaded JS/WASM build)
  is used strictly as (a) a training-time move teacher that generates local
  curriculum examples, and (b) an optional opponent in "Stockfish vs
  StockFly" exhibition mode. `tools/browser/fetch_stockfish.py` prepares the pinned Stockfish.js
  v19.0.0 browser assets and verifies their SHA-256 values. Portable bundles
  include the corresponding upstream source archive, its README/build.js
  build instructions, Copying.txt, AUTHORS and source URLs/hashes. The local
  StockFly source archive and lockfiles accompany the runtime as well. Stockfish binaries are not committed to this repository.
- **Isolation:** Stockfish's evaluations, search output, and candidate moves
  are never passed into the StockFly neural simulator at inference time. See
  `docs/superpowers/specs/2026-09-18-stockfly-design.md` for the enforced
  boundary and the causal audit that verifies it.
- **Distribution consequence:** because Stockfish is GPLv3, the StockFly
  application that bundles/links against it is distributed under
  GPL-3.0-or-later (see `LICENSE`).

## Other dependencies

Standard OSS dependencies (Rust crates, npm packages) retain their own
licenses as declared in `Cargo.toml` / `package.json` and their respective
lockfiles. Their dependency-specific license and notice obligations still apply; this
notice is not a legal certification that every distribution obligation is met.

## Browser chess pieces

The local SVG chess pieces in `apps/web/public/pieces` are the Cburnett set
by Colin M. L. Burnett, licensed GPL-2.0-or-later and redistributed here
under GPL-3.0-or-later. They come from `lichess-org/lila` at revision
`eff61677731721776ccc7b2b1b39c313c8f143eb`, `public/piece/cburnett`.
The directory preserves upstream `COPYING.md`, exact source URLs and SHA256
hashes in `sources.json`, and the applicable GPL-3.0 license text.
StockFly is not affiliated with Chess.com or Lichess.

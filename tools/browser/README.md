# Browser soma metadata

After compiling the graph and geometry, export the source annotations into a separate derived directory:

```sh
.venv/bin/python tools/browser/export_metadata.py
```

`apps/web/public/vendor/brain` links to `data/browser`; Vite includes that directory in its production output. The exporter only reads `data/raw` and `data/compiled`. It preserves the exact compiled neuron ordering and graph hash. IDs are read directly from `neurons.bin` as uint64 strings in the browser, without a JavaScript Number round trip. Annotation columns are type, class, superclass, and somaNeuromere. A superclass is not an anatomical region. An absent neuromere is explicitly unannotated.

LOD0 is the existing real soma point cloud, not skeletons or inferred positions. Neurons without recorded soma geometry still participate in the full simulation. The visible, rendered and simulated counts are separate in the interface.

Live inference pauses at actual CPU/GPU state boundaries and transfers at most 25 quantized frames per second (up to 16 samples per inference). Frame `tMs` is simulation time; `elapsedMs` measures wall time including sampling/pacing. Quantization maps dimensionless rates 0–20 to uint8; exact top-neuron values and final decision readouts remain full precision. `frameMode: "full"` opts into full-rate frames. GPU runtime failure restarts the position on CPU; backend and restarted step counters identify that change. Frame `regionRates` contains exact full-population averages by source somaNeuromere (including neurons without recorded geometry); unannotated cells stay explicit. If metadata is unavailable, region summaries are empty and the brain panel reports the metadata error. No activity is interpolated or fabricated.

The main thread paints the final sample before applying a move. Worker requests mutate one engine serially; generation and trace IDs invalidate outdated frames, decisions and errors after new game/side change. Ordinary game reset preserves the loaded checkpoint.

Focused checks:

```sh
node --experimental-strip-types --test apps/web/src/brain/activation.test.ts apps/web/src/chess/board.test.ts
npx tsc --noEmit --target ES2022 --module ESNext --moduleResolution bundler --lib ES2022,DOM --skipLibCheck apps/web/src/main.ts apps/web/src/engine/stockfly.worker.ts
```

Build WASM first after changing the binding; then `npm run build --workspace apps/web`. The application reports missing geometry separately from engine load failures. Missing, corrupt, or incompatible prepared checkpoints stop model loading rather than silently selecting another model.

## Stockfish exhibition engine

The Stockfish vs Fly mode uses the official Stockfish.js 19.0.0 Lite single-thread build. Populate the ignored local cache from the pinned release and verify every SHA-256 before building:

```sh
npm run assets:stockfish --workspace apps/web
```

For an offline preparation, pass an existing asset directory to the tool:

```sh
python3 tools/browser/fetch_stockfish.py --source-dir /path/to/stockfish-assets --offline
```

The generated `data/vendor/stockfish-19-lite` directory contains the exact source URLs, hashes, attribution notice, and upstream GPL text. `apps/web/public/vendor/stockfish` is a tracked symlink to that cache. Evaluation scores and principal variations remain inside the isolated Stockfish worker; the application receives only a validated best move.

## Trained model catalog

Prepare the two approved Full checkpoints for local browser development:

```bash
npm run assets:models --workspace apps/web
```

The command reads checkpoint metadata and bytes from the source artifacts, verifies the expected model kind, and writes an ignored `data/browser-models/catalog.json`. Checkpoint assets use content-addressed names, so promoting a completed training run changes the catalog without overwriting an older checkpoint or editing product code:

```bash
npm run assets:models --workspace apps/web -- \
  --max-source data/training-runs/max-quick-2026-09-18/stockfly-max-full.sfckpt
```

Run promotion only after the trainer has completed the checkpoint. Local development uses symlinks in its own cache and never writes shared checkpoint sources. Release preparation can materialize the same layout with `--mode copy --output-dir <staging-directory>`.

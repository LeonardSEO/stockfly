# Task 1 report: remove StockFly Lite

## Completed scope

- Removed the StockFly Lite variant from the Rust model enum, browser catalog parser and generator, selector UI, truth badges, trace validation, release manifest status/schema, installer checks, package message, README, and focused tests.
- Regenerated the ignored local browser catalog from Bio Full Standard seed 45 and Max Full Standard seed 43. Its two entries are `bio-full` and `max-full`.
- Preserved Stockfish 19 Lite as the external teacher/opponent, including its worker, vendor assets, attribution, and historical results.
- Added cancellation notices to the historical StockFly Lite plan and design specification without rewriting their original evidence.

## Validation

- `node --experimental-strip-types --test apps/web/src/engine/modelCatalog.test.ts apps/web/src/traces/MoveTrace.test.ts` — 12 passed.
- `python3 test_prepare_models.py` from `tools/browser` — 1 passed.
- `npm --workspace @stockfly/web run build` — passed.
- `cargo test -p stockfly-types` — 2 passed.
- `cargo test -p stockfly-server` — 7 integration tests passed.
- `node --test tools/models/test-publish-release.mjs` — 2 passed.
- `cargo build -p stockfly-server && node --test tools/models/test-fetch-release.mjs` — 11 passed.

The Vite build retained its existing bundle-size warning; it completed successfully.

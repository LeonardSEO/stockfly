# Task 5 report: Windows portable runtime validation

## Result

Implemented the Windows runtime/browser smoke gate without changing product or release documentation. The Windows matrix entry now extracts the ZIP it just built, starts `stockfly-server.exe` from the extracted `stockfly` directory on an ephemeral loopback port, validates the app, health, empty-model, and absent-catalog routes, and opens the app in installed headless Chromium through the Chrome DevTools Protocol.

The browser gate requires the no-model `Unavailable` state and visible model-download instructions, including the Windows `stockfly-server.exe fetch-models` command and the statement that no training is required. It fails on page exceptions, console errors, failed requests, or unexpected browser/HTTP errors. In download-first mode it records and permits only the five same-origin 404 routes the app intentionally requests while models are absent: the model catalog, graph manifest, graph neuron IDs, graph LOD0 geometry, and brain metadata. Every other missing asset remains fatal.

The packager now carries the tracked favicon referenced by the built HTML, and the smoke verifies `/favicon.svg` directly. The smoke writes versioned JSON evidence containing the source commit, archive and executable SHA-256 hashes, platform, loopback server configuration, route results and headers, Chromium identity, visible no-model state, expected no-model errors, and unexpected error arrays. Server/browser spawn errors, argument failures, bounded HTTP failures, and disconnected or stalled DevTools sessions flow through the evidence writer. Evidence is written to a temporary sibling and atomically renamed before the process exits nonzero. The upload step runs with `always()` so a failed Windows smoke still preserves its JSON evidence when packaging produced the artifact. The macOS matrix entry remains packaging-only and retains its ZIP upload.

## Files

- `.github/workflows/release.yml`
- `scripts/smoke-portable.mjs`
- `scripts/smoke-portable.test.mjs`
- `scripts/package-local.py`
- `tests/fixtures/portable-no-model-browser.json`
- `.superpowers/sdd/2026-09-19-standard-validation-release/task-5-report.md`

## Local validation

- `node --test scripts/smoke-portable.test.mjs` — passed 10/10 tests, including exact no-model route classification, bounded HTTP/CDP operations, disconnected and stalled fake CDP peers, parse-failure evidence, and a subprocess `ENOENT` spawn failure with atomic JSON evidence.
- `node --check scripts/smoke-portable.mjs` — passed.
- `node --check scripts/smoke-portable.test.mjs` — passed.
- Node 22 global WebSocket availability check — passed.
- `git diff --check` for Task 5 files — passed.
- Parsed `.github/workflows/release.yml` with local PyYAML — passed.

An isolated no-model macOS ZIP fixture containing the packaged app, executable, chess maps, and tracked favicon passed the complete runtime/browser smoke in local Chrome 153. The server routes and visible no-model instructions passed; page, console, unexpected browser-log, failed-request, and unexpected HTTP arrays were empty; the five expected no-model 404s were retained in JSON. This is cross-platform integration evidence, not native Windows proof or local inference evidence.

## Remaining verification boundary

Per the task instruction, the workflow was not dispatched and nothing was pushed. Native Windows extraction, execution, HTTP, Chromium, and uploaded artifact evidence remain unverified until the committed branch is pushed and the workflow is dispatched. The smoke does not claim local model inference; it validates only the download-first no-model state.

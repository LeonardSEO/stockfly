# Task 5 report: Windows portable runtime validation

## Result

Implemented the Windows runtime/browser smoke gate without changing product or release documentation. The Windows matrix entry now extracts the ZIP it just built, starts `stockfly-server.exe` from the extracted `stockfly` directory on an ephemeral loopback port, validates the app, health, empty-model, and absent-catalog routes, and opens the app in installed headless Chromium through the Chrome DevTools Protocol.

The browser gate requires the no-model `Unavailable` state and visible model-download instructions, including the Windows `stockfly-server.exe fetch-models` command and the statement that no training is required. It fails on page exceptions, console errors, unexpected browser log errors, failed requests, or unexpected HTTP errors. The expected missing catalog response is retained as explicit no-model evidence and is the only allowed 404.

The smoke writes versioned JSON evidence containing the source commit, archive and executable SHA-256 hashes, platform, loopback server configuration, route results and headers, Chromium identity, visible no-model state, and error arrays. The upload step runs with `always()` so a failed Windows smoke still preserves its JSON evidence when packaging produced the artifact. The macOS matrix entry remains packaging-only and retains its ZIP upload.

## Files

- `.github/workflows/release.yml`
- `scripts/smoke-portable.mjs`
- `scripts/smoke-portable.test.mjs`
- `.superpowers/sdd/2026-09-19-standard-validation-release/task-5-report.md`

## Local validation

- `node --test scripts/smoke-portable.test.mjs` — passed 5/5 tests.
- `node --check scripts/smoke-portable.mjs` — passed.
- `node --check scripts/smoke-portable.test.mjs` — passed.
- Node 22 global WebSocket availability check — passed.
- `git diff --check` for Task 5 files — passed.
- Parsed `.github/workflows/release.yml` with local PyYAML — passed.

An existing local macOS ZIP was also passed to the smoke as a negative integration check. It was a model-bearing artifact rather than the required `--without-models` artifact, and the smoke correctly rejected its installed catalog before browser launch. This is not Windows runtime evidence.

## Remaining verification boundary

Per the task instruction, the workflow was not dispatched and nothing was pushed. Native Windows extraction, execution, HTTP, Chromium, and uploaded artifact evidence remain unverified until the committed branch is pushed and the workflow is dispatched. The smoke does not claim local model inference; it validates only the download-first no-model state.

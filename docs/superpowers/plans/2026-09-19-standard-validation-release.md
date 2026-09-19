# StockFly Standard Validation and Canonical Release Plan

**Goal:** Validate the selected Bio Standard seed 45 and Max Standard seed 43 checkpoints, measure their low-end playing strength, remove StockFly Lite from the product, close the remaining parity and Windows validation gaps, and publish an evidence-bound canonical experimental release.

**Selected checkpoints:**

- Bio Full Standard seed 45: `data/training-runs/bio-standard-seed45-2026-09-18/stockfly-bio-full-standard-seed45.sfckpt`
- Max Full Standard seed 43: `data/training-runs/max-standard-seed43-2026-09-18/stockfly-max-full-standard-seed43.sfckpt`

**Global constraints:**

- Keep Stockfish 19 Lite: it is the external teacher/opponent. Remove only the proposed StockFly Lite model.
- Bind every result to checkpoint, graph, maps, executable, configuration, backend and source hashes.
- Never call training-stream agreement held-out accuracy or Elo.
- Never claim an absolute, human, FIDE or Chess.com Elo. Report score and local Elo difference against the exact fixed-node opponent condition.
- Preserve failed and negative evidence. A failed scientific gate must remain failed in the release and UI copy.
- Use 16 settling steps for canonical inference, causal audit and ladder measurements.
- Do not overwrite existing reports or checkpoints.
- Keep the live local server available throughout work except for a brief final packaged-runtime smoke if required.

## Task 1: Remove StockFly Lite completely

- Remove `lite` from the StockFly model kind, browser catalog, selector, truth badges, trace validation, release schema/status, package messaging, tests and current documentation.
- Preserve every reference to **Stockfish 19 Lite**.
- Mark the historical StockFly Lite design/plan as cancelled by the user instead of rewriting historical implementation evidence.
- Regenerate the local browser catalog with exactly Bio Full and Max Full.
- Add or update focused tests proving only the two Full model identities are accepted.
- Verify TypeScript tests/build, relevant Rust tests and release manifest fixtures.

## Task 2: Run and publish frozen causal audits for the selected Standard checkpoints

- Freeze a release-built `stockfly-train` evaluator and record its hash.
- Run the existing 175-position causal suite at 16 steps for each selected checkpoint with intact, weight reset, degree-aware shuffle, descending-neuron ablation, output permutation and brain bypass.
- Before interpreting results, use this acceptance rule: intact top-1 must exceed every control; intact top-3 must not be below any control; and the paired one-sided exact McNemar test against the strongest top-1 control must pass at `p < 0.05`. Report the exact discordant counts and p-value. This is a project gate, not a universal neuroscience standard.
- Produce immutable raw JSON plus tracked summary JSON/Markdown with all identities, limitations and PASS/FAIL.
- Do not promote a checkpoint scientifically when either requirement fails.

## Task 3: Run the low-node paired playing-strength ladder

- Use the actual selected Standard checkpoints, persistent Apple M4 Metal inference and 16 settling steps.
- Measure ten deterministic color-swapped opening pairs at Stockfish 19 Lite budgets `1,5,10,25,50` nodes per move for each selected model.
- Retain terminal W/D/L, exclusions, timeouts, ply caps, paired score intervals and local Elo differences/bounds per exact budget.
- Compare the 50-node row with the prior quick-checkpoint 50-node result. Do not infer a standard-vs-quick improvement at budgets without matched quick data.
- Produce immutable raw reports plus tracked summary JSON/Markdown. Report no finite point Elo for endpoint scores.

## Task 4: Establish broad full-graph CPU/GPU numerical parity evidence

- Add a reproducible full-graph parity harness covering both selected checkpoints and a fixed, diverse FEN suite across intermediate and final steps.
- Compare full activation state, legal-move policy rates and selected move at every recorded step.
- Define and record explicit f32-aware activation and policy tolerances before measuring.
- Diagnose and fix implementation defects revealed by the harness without hiding discrepancies through loose tolerances.
- Publish machine/backend/config identities and per-position maxima. Mark parity PASS only if every case meets its declared tolerance and move identity.
- Add focused synthetic regression tests and run the real Apple M4 Metal measurement.

## Task 5: Validate the Windows runtime and browser path

- Extend the existing Windows GitHub Actions job to build the portable runtime, extract it, start `stockfly-server.exe`, verify critical HTTP routes and load the app in headless Chromium with no console/page errors.
- Verify the no-model state and model-download instruction without claiming local model inference when release assets are not installed.
- Upload machine-readable smoke evidence with the Windows artifact.
- Run local static/unit validation, then dispatch the authorized workflow after the branch is pushed and verify the Windows job result and artifacts.

## Task 6: Integrate evidence and publish the canonical experimental release

- Make the selected Standard checkpoints the canonical two-model catalog for the package.
- Update README, result summaries, package evidence and release manifest with the actual Task 2-5 outcomes.
- Scientific and strength claims must remain conditional on measured evidence. A failed causal gate produces an experimental causal-failed release, never a validated-strength claim.
- Build and smoke-test the macOS arm64 portable bundle with the exact selected catalog.
- Use the verified Windows workflow artifact when its smoke passes.
- Tag the final commit and publish a GitHub prerelease with manifest, evidence, selected model assets and verified runtime artifacts.
- Verify the remote branch, tag, release state, asset names, counts, sizes and digests server-side.

## Task 7: Final review and integration

- Run task-level reviews after each implementation task and resolve findings.
- Run a broad final review across the complete branch.
- Run the proportional full validation suite once after all fixes.
- Merge/push only the reviewed final state and leave the development server on the final application.

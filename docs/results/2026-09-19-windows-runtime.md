# Windows portable runtime smoke — 19 September 2026

**PASS for the download-first, no-model runtime/browser path.** [GitHub Actions run 35434707879](https://github.com/LeonardSEO/stockfly/actions/runs/35434707879), attempt 1, built source commit `d29039f37a24076acbf2d79776349059a2d5deba`. The extracted x64 server served the application, favicon, health and empty-model routes with isolation headers. Headless Chrome 152 showed `Unavailable` and the Windows download command with no-training instructions. Unexpected page, console, browser-log, failed-request and HTTP error arrays are empty. The five intentional missing-model routes are retained as expected 404s.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `stockfly-windows-x64.zip` | 4,223,666 | `e90cbadc6f6f3eb8a5a7ebf761544dd6b9bde167a174cd9689922c774baf967a` |
| `stockfly-windows-x64-smoke.json` | 4,724 | `ffcf3fb5b07615114581e2f9b7fa3eebdaf72e4310b6d97be1abb70b17cae65e` |

The [unmodified smoke JSON](2026-09-19-windows-runtime.json) records the executable identity, timestamps, route responses and browser checks. The downloaded archive size and digest were verified locally against the passing smoke report. This workflow commit precedes the v0.3.0 documentation/evidence integration; the Windows ZIP is retained as tested, with its original bundled documentation. It contains no models. This evidence does not establish Windows model inference, DX12 parity, browser WebGPU parity, signing or notarization.

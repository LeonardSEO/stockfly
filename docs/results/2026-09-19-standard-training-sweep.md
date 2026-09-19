# Standard training candidate sweep — 19 September 2026

Ten independent wall-clock-bounded candidates were trained concurrently on the Apple M4: five Bio Full and five Max Full. Each run used the `standard` preset (16 settling steps), a 30-minute untrained baseline cap, and a two-hour learning cap. The five deterministic curriculum orderings contain the same 24,584 locally generated Stockfish 19-labelled positions.

These are **candidate checkpoints**. Online teacher agreement is order-dependent training-stream telemetry, not held-out accuracy, causal evidence, or Elo. The active browser/release catalog remains on the previously audited quick checkpoints until a candidate passes fresh held-out causal and match evaluation.

| Model | Seed | Baseline trials / top-1 | Trained trials / top-1 | Change | Delta edges | SHA-256 |
|---|---:|---:|---:|---:|---:|---|
| Bio | 42 | 2,352 / 6.46% | 6,903 / 5.27% | -1.19 pp | 1,148,735 | `2c8c8af058866d129452952e1ea43ee6b13abe57facd2d7f0db0b0dc24433723` |
| Bio | 43 | 2,187 / 10.11% | 6,925 / 9.76% | -0.35 pp | 1,153,039 | `6ab727d8133fcec67013738bb109e10612aa8bac6929870ae79f695951c9a699` |
| Bio | 44 | 2,181 / 9.40% | 6,918 / 9.87% | +0.47 pp | 1,148,131 | `5187403340c08e6950367e084cb5074a072516fc77a657c3329dcd1f5b3602ed` |
| Bio | 45 | 2,180 / 9.82% | 6,898 / 10.50% | +0.68 pp | 1,150,563 | `5e2094743fe960f307d4a39647afbd05682e47867f3a4e771a24f9634d01b3c5` |
| Bio | 46 | 2,168 / 9.50% | 6,953 / 10.23% | +0.73 pp | 1,150,740 | `868300721b6cd7f383523d2cba58edd7b0d5f897be7c38b9e80aa6d6ce7772d0` |
| Max | 42 | 2,360 / 6.44% | 5,566 / 3.92% | -2.52 pp | 8,075,729 | `fcc2845cd92eabcd794d8c6c296f78e7a34584558937a3cda2b2b7b76e3a9bc4` |
| Max | 43 | 2,183 / 10.08% | 5,649 / 9.93% | -0.15 pp | 8,550,866 | `a10de3f743ba9b0a2f590ba2a19fae794c0da615facd54e1b88d1595cd1c0bd3` |
| Max | 44 | 2,172 / 9.44% | 5,658 / 9.35% | -0.09 pp | 8,393,536 | `e67f271aa6c1117ac70140881bee1099d391ecd055ebdb991450ebce73d7d2bb` |
| Max | 45 | 2,177 / 9.83% | 5,651 / 9.11% | -0.72 pp | 8,290,173 | `3d1a38facbf1d74a9c965db6b548b03370c0b3b12503c9806a282959003bf480` |
| Max | 46 | 2,164 / 9.52% | 5,661 / 9.63% | +0.11 pp | 8,311,452 | `b08cfe84e6a25ca740a903fe128885459c54f3438b824514d8adc901e8d4f3ff` |

Bio seed 45 has the highest Bio online score; Bio seed 46 has the largest same-order improvement. Max seed 43 has the highest Max online score, while Max seed 46 is the only Max ordering with a positive same-order change. None of those observations selects a scientifically accepted model.

The trainer's plasticity implementation is currently CPU-reference-only. Ten concurrent processes used the ten CPU cores; Metal/wgpu remains the inference and benchmark backend. Heavy concurrent contention reduced the number of examples each process could complete within its wall-clock deadline, which is why the full 24,584-position curriculum was not exhausted.

Shared identities:

- Graph neurons: `cb1bd8c967056afe8e1ae667df9f0d99c59cac365d1660ed61b142caca5c6405`
- Sensory map: `fb5ba4e541b13a34a50973a0760de2b056ed4ac7a2d81425b199c095cdf569b5`
- Output map: `cd6af3b0961c9e096938a9f1802ec32c00d6afb6c9a0125cf72162c64448e5b2`
- Base curriculum: `14f8614c11702c7b31275324de5642259baa11c02f2c6726482a7d2e9dcfb991`


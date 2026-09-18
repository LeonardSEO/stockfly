#!/usr/bin/env python3
"""Run all native causal controls with hash-verified immutable inputs."""
import argparse
import json
from pathlib import Path
import subprocess

from weight_reset import input_hashes, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--suite', type=Path, default=Path('tests/fixtures/chess-suite.json'))
    parser.add_argument('--graph', type=Path, default=Path('data/compiled/malecns-v1'))
    parser.add_argument('--chess-dir', type=Path, default=Path('crates/stockfly-chess/resources'))
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/release/stockfly-train'))
    parser.add_argument('--settle-steps', type=int, default=16)
    parser.add_argument('--shuffle-seed', type=int, default=0x53485546464C4501)
    parser.add_argument('--permutation-seed', type=int, default=0x5045524D55544501)
    parser.add_argument('--bypass-seed', type=int, default=0x4259504153530001)
    parser.add_argument('--ablate-region', default='descending_neuron')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.settle_steps < 1:
        parser.error('--settle-steps must be positive')

    before = input_hashes(
        args.graph, args.model, args.suite, args.chess_dir, args.binary
    )
    before['metadata_sha256'] = sha256(args.metadata)
    command = [
        str(args.binary.resolve()), 'causal-audit',
        '--model', str(args.model), '--suite', str(args.suite),
        '--graph', str(args.graph), '--chess-dir', str(args.chess_dir),
        '--metadata', str(args.metadata), '--settle-steps', str(args.settle_steps),
        '--shuffle-seed', str(args.shuffle_seed),
        '--permutation-seed', str(args.permutation_seed),
        '--bypass-seed', str(args.bypass_seed),
        '--ablate-region', args.ablate_region,
    ]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    report = json.loads(result.stdout)
    after = input_hashes(
        args.graph, args.model, args.suite, args.chess_dir, args.binary
    )
    after['metadata_sha256'] = sha256(args.metadata)
    if before != after:
        raise ValueError('audit inputs changed during evaluation; refusing to publish report')
    report['inputs'] = before
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open('x') as target:
        json.dump(report, target, indent=2)
        target.write('\n')
    print(json.dumps(report['summary'], indent=2))
    print(f'Report: {args.out}')


if __name__ == '__main__':
    main()

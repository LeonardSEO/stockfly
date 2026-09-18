#!/usr/bin/env python3
"""Hash-verified local weight-reset audit; uses only Python's standard library.

Build stockfly-train first. Does not invoke a teacher, train, or modify models.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def input_hashes(graph, model, suite, chess, binary):
    manifest = json.loads((graph / 'manifest.json').read_text())
    files = {'neurons.bin': manifest['neurons_sha256'],
             'offsets.bin': manifest['offsets_sha256']}
    for block in manifest['edge_blocks']:
        for prefix, field in [('edge_src_blocks', 'src_sha256'),
                              ('edge_weight_blocks', 'weight_sha256')]:
            files[f"{prefix}/{block['index']:04}.bin"] = block[field]
    hashes = {}
    for name, expected in files.items():
        actual = sha256(graph / name)
        if actual != expected:
            raise ValueError(f'compiled graph hash mismatch: {name}')
        hashes[name] = actual
    graph_hash = hashlib.sha256(json.dumps(hashes, sort_keys=True,
                                          separators=(',', ':')).encode()).hexdigest()
    return {'graph_sha256': graph_hash, 'graph_files': hashes,
            'manifest_sha256': sha256(graph / 'manifest.json'),
            'model_sha256': sha256(model), 'suite_sha256': sha256(suite),
            'sensory_map_sha256': sha256(chess / 'sensory-map.json'),
            'output_map_sha256': sha256(chess / 'output-map.json'),
            'binary_sha256': sha256(binary)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--suite', type=Path, default=Path('tests/fixtures/chess-suite.json'))
    parser.add_argument('--graph', type=Path, default=Path('data/compiled/malecns-v1'))
    parser.add_argument('--chess-dir', type=Path, default=Path('crates/stockfly-chess/resources'))
    parser.add_argument('--binary', type=Path, default=Path('target/release/stockfly-train'))
    parser.add_argument('--settle-steps', type=int, default=16)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.settle_steps < 1:
        parser.error('--settle-steps must be positive')
    before = input_hashes(args.graph, args.model, args.suite, args.chess_dir, args.binary)
    command = [str(args.binary.resolve()), 'audit-reset', '--model', str(args.model),
               '--suite', str(args.suite), '--graph', str(args.graph),
               '--chess-dir', str(args.chess_dir), '--settle-steps', str(args.settle_steps)]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    report = json.loads(result.stdout)
    after = input_hashes(args.graph, args.model, args.suite, args.chess_dir, args.binary)
    if before != after:
        raise ValueError('audit inputs changed during evaluation; refusing to publish report')
    report['inputs'] = before
    report['limitations'] = [
        'Weight reset tests learned-weight sensitivity, not whole-connectome causality.',
        'Teacher agreement is not Elo or proof of improved playing strength.',
        'Version-1 checkpoints identify neuron IDs and maps, but do not bind edge hashes or calibration; actual audit inputs are hashed here.',
        'Section labels are inherited from the suite, not independently verified tactical categories.',
    ]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    # An audit should never overwrite an existing report or any input file.
    with args.out.open('x') as target:
        json.dump(report, target, indent=2)
        target.write('\n')
    print(json.dumps(report['summary'], indent=2))
    print(f'Report: {args.out}')


if __name__ == '__main__':
    main()

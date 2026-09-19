#!/usr/bin/env python3
"""Publish immutable parity summaries, retaining baseline failures and source identities."""
import argparse
import copy
import hashlib
import json
from pathlib import Path


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paired(left, right):
    if len(left) != len(right):
        raise ValueError("measurement shape changed")
    return zip(left, right)


def summarize(path):
    report = json.loads(path.read_text())
    result = copy.deepcopy(report)
    result['raw_report'] = {'path': str(path), 'sha256': sha256(path)}
    for model in result['models']:
        for position in model['positions']:
            for step in position['steps'] + [position['batched_final']]:
                # Actual UCI/CPU/GPU triples remain in the identified immutable raw file.
                del step['legal_scores_uci_cpu_gpu']
    return result


def publish(raw_dir, output):
    outputs = [output.with_suffix('.json'), output.with_suffix('.md')]
    if any(path.exists() for path in outputs):
        raise ValueError('refusing to overwrite an existing published measurement')
    baseline = summarize(raw_dir / 'baseline.json')
    diagnostic = summarize(raw_dir / 'diagnostic.json')
    final = summarize(raw_dir / 'final.json')
    if len({r['protocol_sha256'] for r in [baseline, diagnostic, final]}) != 1:
        raise ValueError('protocol changed between measurements')
    gpu_matches = 0
    gpu_comparisons = 0
    for old, new in paired(baseline['models'], final['models']):
        for field in ['graph_sha256', 'model_sha256', 'sensory_map_sha256', 'output_map_sha256']:
            if old['identity'][field] != new['identity'][field]:
                raise ValueError(f'measurement input changed: {field}')
        for before, after in paired(old['positions'], new['positions']):
            if before['fen'] != after['fen']:
                raise ValueError('position changed between measurements')
            for left, right in paired(before['steps'] + [before['batched_final']], after['steps'] + [after['batched_final']]):
                gpu_comparisons += 1
                gpu_matches += left['gpu_state_sha256'] == right['gpu_state_sha256']
    diagnosis = {model['id']: [
        {'step': step['step'], **step['rounding_diagnosis']}
        for step in model['positions'][0]['steps']
    ] for model in diagnostic['models']}
    report = {
        'schema_version': 1, 'status': final['status'],
        'baseline': baseline, 'final': final,
        'diagnostic': {key: diagnostic[key] for key in [
            'status', 'raw_report', 'source_sha256', 'embedded_source_files', 'started_at', 'completed_at'
        ]},
        'rounding_diagnosis_initial_position_all_neurons': diagnosis,
        'metal_before_after': {'identical_state_hashes': gpu_matches, 'compared_state_hashes': gpu_comparisons},
        'interpretation': 'Explicit FMA fixes CPU/GPU rounding inconsistency. Historical CPU measurements use pre-fix rounding and require fresh audits for post-fix claims. GPU hash equivalence applies only to these fixed fixtures.',
    }
    report['diagnostic']['binary_sha256'] = diagnostic['models'][0]['identity']['binary_sha256']
    with outputs[0].open('x') as handle:
        json.dump(report, handle, indent=2)
        handle.write('\n')
    old_failures = sum(not p['pass'] for m in baseline['models'] for p in m['positions'])
    positions = [p for m in final['models'] for p in m['positions']]
    steps = [s for p in positions for s in p['steps']]
    batched = [p['batched_final'] for p in positions]
    maxima = {field: max(p['max_abs'][field] for p in positions) for field in ['membrane', 'rates', 'populations', 'legal_policy']}
    identical = sum(s['cpu_state_sha256'] == s['gpu_state_sha256'] for s in steps + batched)
    passed = sum(p['pass'] for p in positions)
    moved = sum(s['move_identity'] for s in steps)
    batched_moves = sum(s['move_identity'] for s in batched)
    final_sentence = (f"**{final['status']} after the explicit-FMA correction.** {passed}/{len(positions)} model/FEN cases pass at every step from 1 through 16. Maximum membrane / activation / population / legal-policy differences are {maxima['membrane']:g} / {maxima['rates']:g} / {maxima['populations']:g} / {maxima['legal_policy']:g}. CPU/GPU full-state hashes are identical in {identical}/{len(steps + batched)} observations. Selected moves match in {moved}/{len(steps)} intermediate/final steps and {batched_moves}/{len(batched)} additional batched-final checks. GPU incremental/batched state hashes match in {sum(p['gpu_batched_state_bitwise_identical'] for p in positions)}/{len(positions)} positions.")
    lines = [
        '# Full-graph CPU/GPU parity — 19 September 2026', '',
        final_sentence, '',
        f"The pre-fix baseline remains **{baseline['status']}**: {old_failures}/24 positions exceeded the declared activation tolerance, despite zero move mismatches. Maximum rate error was {max(p['max_abs']['rates'] for m in baseline['models'] for p in m['positions']):.9f}; maximum legal-policy error was {max(p['max_abs']['legal_policy'] for m in baseline['models'] for p in m['positions']):.9f}. No tolerance was changed after either result.", '',
        '## Protocol and diagnosis', '',
        'The acceptance rule, fixed before the first run in [the protocol](../../crates/stockfly-train/resources/fullgraph-parity.json), is `abs(cpu-gpu) <= 1e-4 + 2e-5 * max(abs(cpu), abs(gpu))` for both activation and policy, with finite values and exact selected UCI identity required. The relative term is about 168 f32 epsilon across high-fan-in reductions and sixteen recurrent updates; the absolute floor covers near-zero cancellation. Rates cap at 20, limiting permitted rate error to 0.0005; membranes are unbounded. A position fails if any neuron, policy rate, legal score, move or batched-final check fails. Legal scores are joined by UCI rather than ranking position.', '',
        'The Metal compiler contracted multiplication plus addition into FMA, while the CPU used separate f32 operations. A diagnostic replay from each GPU previous state found that fused gather plus fused membrane integration matched every membrane bit for both models on all sixteen initial-position steps; separate arithmetic did not. The correction uses explicit Rust `mul_add` and WGSL `fma` for both operations, preserving sensory addition order. Focused cancellation tests proved the old CPU returned zero where fused arithmetic returns `2^-46`; both CPU and Metal now preserve that result.', '',
        f"All {gpu_matches}/{gpu_comparisons} pre/post-fix Metal state hashes match, including batched finals. {('Metal behavior is unchanged on this suite' if gpu_matches == gpu_comparisons else 'Metal behavior changed on this suite')}, while CPU rounding changes. Historical pre-fix CPU causal results cannot represent the corrected executable; fresh CPU audits are required for post-fix release claims. Native Metal parity does not establish browser WebGPU or Windows DX12 equivalence.", '',
        'The twelve cases include both sides to move, initial boards, tactical/castling positions, en passant, all promotion choices, check evasion, sparse pawn/rook endgames and a closed middlegame. Every position is nonterminal and starts at zero state. Promotion-white, check-evasion, pawn-endgame and closed-middlegame remain numerically equal even before the fix; those easier cases are retained. This fixture is numerical evidence, not playing strength, held-out accuracy or a guarantee for every possible FEN.', '',
        '## Per-position maxima', '',
        'Final maxima cover all sixteen steps and the separate batched-final run. Full per-step comparisons, failures, CPU/GPU state hashes and selected moves are retained in [the tracked JSON](2026-09-19-fullgraph-parity.json); identified raw files also retain every actual legal-move score.', '',
        '| Model | Position | Baseline | Baseline max rate error | Baseline max policy error | Final membrane / rate / policy error | Final |',
        '|---|---|---|---:|---:|---|---|',
    ]
    for old, new in paired(baseline['models'], final['models']):
        for before, after in paired(old['positions'], new['positions']):
            maximum = after['max_abs']
            lines.append(f"| {new['id']} | {after['id']} | {'PASS' if before['pass'] else 'FAIL'} | {before['max_abs']['rates']:.9f} | {before['max_abs']['legal_policy']:.9f} | {maximum['membrane']:g} / {maximum['rates']:g} / {maximum['legal_policy']:g} | {'PASS' if after['pass'] else 'FAIL'} |")
    identity = final['models'][0]['identity']
    lines += ['', '## Machine and identities', '',
        f"Measured {final['started_at']}–{final['completed_at']} on `{final['machine']['cpu']}`, `{final['machine']['uname']}`; OS: {final['machine']['os'].replace(chr(10), '; ')}. Backend: `{final['models'][0]['backend']['name']}` / `{final['models'][0]['backend']['adapter']}`. Full graph: {final['graph']['neurons']:,} neurons and {final['graph']['edges']:,} edges. Release build: Rust 1.96.1, aarch64-apple-darwin, LLVM 22.1.8, isolated `target/fullgraph-parity`.", '',
        f"Configuration: `{json.dumps(final['configuration'], sort_keys=True)}`. The graph, maps and checkpoint identities were validated before measurement. Source digests are embedded at compile time; the run hashes the actual executable. Source files, graph blocks and each model identity are listed in the JSON.", '',
        '| Input | SHA-256 |', '|---|---|',
    ]
    for label, value in [
        ('Bio Standard seed 45', final['models'][0]['identity']['model_sha256']),
        ('Max Standard seed 43', final['models'][1]['identity']['model_sha256']),
        ('Graph aggregate', identity['graph_sha256']), ('Sensory map', identity['sensory_map_sha256']),
        ('Output map', identity['output_map_sha256']), ('Protocol', final['protocol_sha256']),
        ('Configuration', final['configuration_sha256']), ('Final executable', identity['binary_sha256']),
        ('Embedded source aggregate', final['source_sha256']),
        ('Baseline executable', baseline['models'][0]['identity']['binary_sha256']),
    ]:
        lines.append(f'| {label} | `{value}` |')
    lines += ['', '## Reproduction and validation', '',
        'See [tools/parity/README.md](../../tools/parity/README.md) for commands. Raw baseline, diagnostic and final reports plus frozen executables are kept separately under `data/reports/standard-validation-2026-09-19/fullgraph-parity/`; their report/executable digests are retained in the tracked JSON. Output creation is exclusive: reruns require a new filename.', '',
        'Validation performed: three comparator/protocol tests; two CPU cancellation regressions; real Metal cancellation for gather and membrane integration; two existing CPU fixture tests; the expanded learned-weight Metal fixture covering every intermediate membrane/rate and reset/batched state; targeted causal-control and weight-reset tests; optimized full-graph measurement. The old cancellation tests failed before the fix and passed afterward. The fixed full-graph result is stated above; no tolerance was changed.', '',
    ]
    with outputs[1].open('x') as handle:
        handle.write('\n'.join(lines))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('raw_dir', type=Path)
    parser.add_argument('output_basename', type=Path)
    args = parser.parse_args()
    publish(args.raw_dir, args.output_basename)

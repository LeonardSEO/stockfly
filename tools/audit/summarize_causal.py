#!/usr/bin/env python3
"""Summarize frozen causal reports using the preregistered Standard project gate."""
import argparse
from fractions import Fraction
import hashlib
import json
from math import comb
from pathlib import Path

# Fixed tie order; all equally strongest controls are reported and tested.
CONDITIONS = {
    'intact': 'intact',
    'weight_reset': 'reset',
    'shuffled_graph': 'shuffled',
    'region_ablation': 'ablated',
    'output_permutation': 'output_permuted',
    'brain_bypass': 'brain_bypass',
}
GATE = {
    'description': 'Project gate; not a universal neuroscience standard.',
    'top1': 'Intact strictly exceeds every control.',
    'top3': 'Intact is not below any control.',
    'paired_test': 'One-sided exact McNemar: P[Binomial(b+c, 0.5) >= b] < 0.05.',
    'b': 'Intact correct and control incorrect.',
    'c': 'Intact incorrect and control correct.',
    'ties': 'Test every control tied for highest control top-1; all must pass.',
    'alpha': 0.05,
}


def exact_mcnemar(intact_only, control_only):
    """Exact upper tail under equal discordant-pair probabilities (no mid-p)."""
    if intact_only < 0 or control_only < 0:
        raise ValueError('discordant counts must be nonnegative')
    n = intact_only + control_only
    return Fraction(sum(comb(n, k) for k in range(intact_only, n + 1)), 2 ** n)


def evaluate_gate(report):
    trials = report['trials']
    if not trials:
        raise ValueError('empty report')
    counts = {}
    for condition, prefix in CONDITIONS.items():
        top3 = [row[f'{prefix}_top3'] for row in trials]
        if any(type(value) is not bool for value in top3):
            raise ValueError('top-3 flags must be booleans')
        counts[condition] = {
            'top1': sum(row[f'{prefix}_move'] == row['bestmove'] for row in trials),
            'top3': sum(top3),
        }
        if counts[condition]['top1'] > counts[condition]['top3']:
            raise ValueError('top-1 exceeds top-3')
        if any(report['summary'][condition][metric] != counts[condition][metric]
               for metric in ('top1', 'top3')):
            raise ValueError(f'summary disagrees with trials: {condition}')
    if report['summary']['positions'] != len(trials):
        raise ValueError('position count disagrees with trials')
    controls = list(CONDITIONS)[1:]
    strongest_score = max(counts[name]['top1'] for name in controls)
    strongest = [name for name in controls if counts[name]['top1'] == strongest_score]
    tests = []
    for name in strongest:
        prefix = CONDITIONS[name]
        pairs = [(row['intact_move'] == row['bestmove'],
                  row[f'{prefix}_move'] == row['bestmove']) for row in trials]
        b = sum(a and not c for a, c in pairs)
        c = sum(c and not a for a, c in pairs)
        p = exact_mcnemar(b, c)
        tests.append({
            'control': name, 'intact_only_correct': b, 'control_only_correct': c,
            'both_correct': sum(a and c for a, c in pairs),
            'both_incorrect': sum(not a and not c for a, c in pairs),
            'discordant_total': b + c, 'p_value': float(p),
            'p_exact_numerator': p.numerator, 'p_exact_denominator': p.denominator,
            'pass': p < Fraction(1, 20),
        })
    checks = {
        'intact_top1_exceeds_every_control': all(
            counts['intact']['top1'] > counts[name]['top1'] for name in controls),
        'intact_top3_not_below_any_control': all(
            counts['intact']['top3'] >= counts[name]['top3'] for name in controls),
        'strongest_control_exact_mcnemar_p_below_0_05': all(t['pass'] for t in tests),
    }
    return {'positions': len(trials), 'counts': counts,
            'strongest_top1_controls': strongest, 'paired_tests': tests,
            'checks': checks, 'status': 'PASS' if all(checks.values()) else 'FAIL'}


def summarize(path, expected_positions=175, expected_steps=16):
    raw = path.read_bytes()
    report = json.loads(raw)
    if report['report_version'] != 1 or report['control_set'] != 'causal-controls-v1':
        raise ValueError('unsupported report version or controls')
    if len(report['trials']) != expected_positions:
        raise ValueError('unexpected suite size')
    if report['calibration']['settle_steps'] != expected_steps:
        raise ValueError('unexpected settling steps')
    gate = evaluate_gate(report)
    return {
        'raw_report': str(path), 'raw_report_sha256': hashlib.sha256(raw).hexdigest(),
        **{key: report[key] for key in (
            'model_kind', 'inputs', 'graph', 'maps', 'calibration', 'controls',
            'checkpoint_delta_edges', 'suite_source', 'metadata_source',
            'sections', 'limitations')},
        'gate': gate,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', action='append', type=Path, required=True)
    parser.add_argument('--provenance', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    provenance = json.loads(args.provenance.read_text())
    reports = [summarize(path) for path in args.report]
    if any(r['inputs']['binary_sha256'] != provenance['binary_sha256'] for r in reports):
        raise ValueError('provenance executable does not match report')
    result = {'summary_version': 1, 'acceptance_rule': GATE,
              'provenance': provenance, 'reports': reports,
              'status': 'PASS' if all(r['gate']['status'] == 'PASS' for r in reports) else 'FAIL'}
    with args.out.open('x') as target:
        json.dump(result, target, indent=2)
        target.write('\n')


if __name__ == '__main__':
    main()

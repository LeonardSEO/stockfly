import copy
from fractions import Fraction
import unittest

from summarize_causal import CONDITIONS, evaluate_gate, exact_mcnemar


def report(intact, controls=None, top3=None):
    controls = controls or {}
    trials = []
    for i, correct in enumerate(intact):
        row = {'bestmove': 'a2a3'}
        for name, prefix in CONDITIONS.items():
            hit = correct if name == 'intact' else controls.get(name, [False] * len(intact))[i]
            row[f'{prefix}_move'] = 'a2a3' if hit else 'b2b3'
            row[f'{prefix}_top3'] = top3[name][i] if top3 and name in top3 else hit
        trials.append(row)
    summary = {'positions': len(trials)}
    for name, prefix in CONDITIONS.items():
        summary[name] = {
            'top1': sum(t[f'{prefix}_move'] == t['bestmove'] for t in trials),
            'top3': sum(t[f'{prefix}_top3'] for t in trials),
        }
    return {'trials': trials, 'summary': summary}


class CausalGateTest(unittest.TestCase):
    def test_exact_tail_direction_and_zero_discordance(self):
        self.assertEqual(exact_mcnemar(0, 0), 1)
        self.assertEqual(exact_mcnemar(5, 0), Fraction(1, 32))
        self.assertEqual(exact_mcnemar(0, 5), 1)
        self.assertEqual(exact_mcnemar(3, 1), Fraction(5, 16))

    def test_clear_paired_advantage_passes_all_strongest_ties(self):
        result = evaluate_gate(report([True] * 5))
        self.assertEqual(result['status'], 'PASS')
        self.assertEqual(len(result['paired_tests']), 5)
        self.assertEqual(result['paired_tests'][0]['intact_only_correct'], 5)

    def test_top1_advantage_alone_is_insufficient(self):
        result = evaluate_gate(report([True] * 4))
        self.assertEqual(result['status'], 'FAIL')
        self.assertTrue(result['checks']['intact_top1_exceeds_every_control'])
        self.assertEqual(result['paired_tests'][0]['p_value'], 0.0625)

    def test_top3_regression_fails_despite_significant_top1(self):
        result = evaluate_gate(report([True] * 5 + [False],
                                     top3={'brain_bypass': [True] * 6}))
        self.assertEqual(result['status'], 'FAIL')
        self.assertFalse(result['checks']['intact_top3_not_below_any_control'])

    def test_selects_strongest_control_and_exact_paired_counts(self):
        result = evaluate_gate(report([True, True, False, False],
                                     {'weight_reset': [True, False, True, True]}))
        self.assertEqual(result['strongest_top1_controls'], ['weight_reset'])
        test = result['paired_tests'][0]
        self.assertEqual((test['intact_only_correct'], test['control_only_correct']), (1, 2))
        self.assertEqual(test['p_value'], 0.875)
        self.assertEqual(result['status'], 'FAIL')

    def test_equal_top1_fails_strict_comparison(self):
        result = evaluate_gate(report([True], {'brain_bypass': [True]}))
        self.assertFalse(result['checks']['intact_top1_exceeds_every_control'])
        self.assertEqual(result['paired_tests'][0]['p_value'], 1)

    def test_rejects_inconsistent_summary(self):
        value = copy.deepcopy(report([True]))
        value['summary']['intact']['top1'] = 0
        with self.assertRaisesRegex(ValueError, 'summary disagrees'):
            evaluate_gate(value)


if __name__ == '__main__':
    unittest.main()

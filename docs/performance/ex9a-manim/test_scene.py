"""Independent closed-form and actual rendered-geometry verification."""
import json
import unittest
from fractions import Fraction
from pathlib import Path

from model import ETAS, STEPS, trajectory, axis_x, AXIS_MIN, AXIS_MAX


class MathematicalContract(unittest.TestCase):
    def test_real_endpoints_against_exact_closed_form(self):
        for eta in ETAS:
            q = 1 - 2 * Fraction(str(eta))
            for k, row in enumerate(trajectory(eta)):
                e = -2 * q**k
                expected = dict(w=2+e, e=e, loss=e*e,
                                delta=-2*Fraction(str(eta))*e,
                                length=2*Fraction(str(eta))*abs(e), ratio=abs(q))
                for field, value in expected.items():
                    self.assertAlmostEqual(row[field], float(value), places=10, msg=f"{eta}/{k}/{field}")
                self.assertTrue(AXIS_MIN <= row['w'] <= AXIS_MAX)

    def test_equal_losses_different_directions(self):
        slow, cross = trajectory(.2), trajectory(.8)
        for k in range(STEPS + 1):
            self.assertAlmostEqual(slow[k]['loss'], cross[k]['loss'])
            self.assertAlmostEqual(slow[k]['e'], cross[k]['e'] * (-1)**k)

    def test_rendered_geometry_and_crossings(self):
        evidence = json.loads(Path("scene-evidence.json").read_text(encoding="utf-8"))
        self.assertEqual(len(evidence['events']), STEPS * 4)
        for event in evidence['events']:
            k = event['k']
            for i, eta in enumerate(ETAS):
                q = 1-2*Fraction(str(eta))
                e = float(-2*q**k)
                if event.get('endpoint'):
                    d = event['distances'][i]
                    # Measure actual Manim line endpoints, against independent scale .9.
                    self.assertAlmostEqual(abs(d['end'][0]-d['start'][0]), abs(e)*.9, places=8)
                    self.assertAlmostEqual(event['rows'][i]['w'], 2+e, places=10)
                else:
                    target = 2+e + float(-2*Fraction(str(eta)))*e*event['progress']
                    self.assertAlmostEqual(event['dots'][i][0], axis_x(target), places=8)
                    arrow = event['arrows'][i]
                    self.assertAlmostEqual(arrow['end'][0]-arrow['start'][0], -2*eta*e*.9, places=8)
                    if event['crossing_eta'] == eta:
                        self.assertAlmostEqual(event['dots'][i][0], axis_x(2), places=8)


if __name__ == "__main__":
    unittest.main()

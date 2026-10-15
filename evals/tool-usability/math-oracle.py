"""Exact counterexamples for TC2A's two-segment average-speed statements."""
from fractions import Fraction
import json
from pathlib import Path

cases = []
for label, v1, v2, t1, t2 in [
    ('equal-speed-unequal-time', 10, 10, 1, 2),
    ('unequal-speed-equal-time', 10, 20, 1, 1),
    ('original-equal-distance', 60, 30, 1, 2),
    ('equal-speed-equal-distance', 10, 10, 1, 1),
]:
    weighted = Fraction(v1 * t1 + v2 * t2, t1 + t2)
    arithmetic = Fraction(v1 + v2, 2)
    difference = Fraction((v1-v2) * (t1-t2), 2 * (t1+t2))
    assert weighted - arithmetic == difference
    assert (weighted == arithmetic) == ((v1-v2) * (t1-t2) == 0)
    cases.append(dict(case=label, v1=v1, v2=v2, t1=t1, t2=t2,
                      weighted=str(weighted), arithmetic=str(arithmetic), equal=weighted == arithmetic))
output = Path(__file__).resolve().parents[2] / 'docs/performance/tool-usability/to14/math-oracle.json'
output.write_text(json.dumps({'identity': 'weighted-arithmetic=(v1-v2)*(t1-t2)/(2*(t1+t2))',
                              'domain': 't1>0, t2>0; constant speed in each segment', 'cases': cases}, indent=2), encoding='utf8')
print('4 exact cases; equal-time-only necessity is disproved by equal-speed-unequal-time')

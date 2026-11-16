"""Measure fundamental pitch from the browser's real piano output.

Run salamander-piano.mjs first. This measures the sampled audio, rather than
deriving correctness from the requested playbackRate/detune parameters.
"""
import json
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = ROOT / 'docs/performance/salamander-lossless-20261104'


def frequency(samples, rate, target):
    window = (samples - samples.mean()) * np.hanning(len(samples))
    size = 1 << 20
    spectrum = np.abs(np.fft.rfft(window, size))
    low, high = int(target * 0.97 * size / rate), int(target * 1.03 * size / rate)
    peak = low + int(np.argmax(spectrum[low:high]))
    values = np.log(np.maximum(spectrum[peak - 1:peak + 2], 1e-20))
    adjustment = .5 * (values[0] - values[2]) / (values[0] - 2 * values[1] + values[2])
    return float((peak + adjustment) * rate / size)


def main():
    browser = json.loads((DIRECTORY / 'browser.json').read_text())
    notes = []
    for recording in browser['recordings']:
        audio = np.fromfile(recording['file'], dtype=np.float32).astype(float)
        rate = recording['sample_rate']
        onset = int(np.flatnonzero(np.abs(audio) > .0001)[0])
        target = 440 * 2 ** ((recording['midi_pitch'] - 69) / 12)
        # Exclude the hammer transient and measure the early sustained tone.
        measured = frequency(audio[onset + int(.10 * rate):onset + int(.50 * rate)], rate, target)
        cents = float(1200 * np.log2(measured / target))
        notes.append({'midi_pitch': recording['midi_pitch'], 'target_hz': round(target, 6),
                      'measured_hz': round(measured, 6), 'deviation_cents': round(cents, 4),
                      'peak': round(float(np.max(np.abs(audio))), 6)})
    maximum = max(abs(note['deviation_cents']) for note in notes)
    report = {'reference': 'A4=440 Hz, twelve-tone equal temperament',
              'measurement': 'Contiguous native browser PCM captured on the audio thread by AudioWorklet, after sample transposition, fine tuning, envelope and compressor. Hann-windowed FFT with log-peak interpolation, 0.10–0.50 s after audible onset.',
              'sample_rate': browser['recordings'][0]['sample_rate'],
              'tolerance_cents': 5, 'max_absolute_deviation_cents': maximum,
              'passed': maximum <= 5, 'notes': notes}
    (DIRECTORY / 'pitch-audit.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']:
        raise SystemExit('Investigate the recorded pitch before shipping')


if __name__ == '__main__':
    main()

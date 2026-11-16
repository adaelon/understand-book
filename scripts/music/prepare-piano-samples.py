"""Prepare original lossless Salamander V3 recordings for the browser piano.

Requires ffmpeg and numpy. Original FLAC files remain in the local source cache.
"""
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import shutil
import subprocess
import urllib.parse
import urllib.request
import urllib.error

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / '.tmp/salamander-source'
DESTINATION = ROOT / 'packages/web/public/piano/salamander-v3'
BASE = 'https://raw.githubusercontent.com/sfzinstruments/SalamanderGrandPiano/master/'
FFMPEG = shutil.which('ffmpeg')


def source(relative):
    file = CACHE / relative.replace('/', '-')
    if not file.exists():
        request = urllib.request.Request(BASE + urllib.parse.quote(relative), headers={'User-Agent': 'UnderstandBook-Piano'})
        for attempt in range(2):
            try:
                file.write_bytes(urllib.request.urlopen(request, timeout=60).read())
                break
            except (urllib.error.URLError, TimeoutError):
                if attempt:
                    raise
    return file


def decoded_audio(file):
    audio = subprocess.run([FFMPEG, '-v', 'error', '-i', str(file), '-ac', '1', '-ar', '48000', '-f', 'f32le', '-'], check=True, capture_output=True).stdout
    return np.frombuffer(audio, dtype=np.float32).astype(float)


def measured_frequency(samples, target, start_seconds, duration_seconds):
    start = int(48000 * start_seconds)
    count = int(48000 * duration_seconds)
    window = samples[start:start + count]
    window = (window - window.mean()) * np.hanning(len(window))
    size = 1 << 20
    spectrum = np.abs(np.fft.rfft(window, size))
    low = max(1, int(target * 0.96 * size / 48000))
    high = int(target * 1.04 * size / 48000)
    peak = low + int(np.argmax(spectrum[low:high]))
    neighbors = np.log(np.maximum(spectrum[peak-1:peak+2], 1e-20))
    adjustment = 0.5 * (neighbors[0] - neighbors[2]) / (neighbors[0] - 2 * neighbors[1] + neighbors[2])
    return float((peak + adjustment) * 48000 / size)


def main():
    if not FFMPEG:
        raise SystemExit('ffmpeg is required')
    CACHE.mkdir(parents=True, exist_ok=True)
    DESTINATION.mkdir(parents=True, exist_ok=True)
    regions = source('Data/region.txt').read_text()
    tuning = dict(re.findall(r'\$TUNE(\d+)\s+(-?\d+)', source('Data/tune_ret.txt').read_text()))
    offsets = dict(re.findall(r'\$OFF(\d+)\s+(\d+)', source('Data/vel_08.txt').read_text()))
    entries = []
    for line in regions.splitlines():
        if not line.startswith('<region>'):
            continue
        fields = dict(re.findall(r'(\w+)=([^ ]+)', line))
        number = fields['region_label']
        original = fields['sample'].replace('$VEL', 'v8').replace('$EXT', 'flac')
        entries.append({'low': int(fields['lokey']), 'high': int(fields['hikey']), 'root': int(fields['pitch_keycenter']),
                        'file': original.replace('#', 's'), 'original': original,
                        'offset_frames': int(offsets[number]), 'upstream_tune_cents': int(tuning[number])})

    def prepare(entry):
        original = source('Samples/' + entry['original'])
        if original.read_bytes()[:4] != b'fLaC':
            raise ValueError(f'Expected a FLAC recording at {original}')
        output = DESTINATION / entry['file']
        shutil.copyfile(original, output)
        audio = decoded_audio(output)
        target = 440 * 2 ** ((entry['root'] - 69) / 12)
        frequency = measured_frequency(audio, target, 0.3 if target >= 100 else 0.6, 0.9 if target >= 100 else 3)
        entry['tune_cents'] = round(float(1200 * np.log2(target / frequency)), 6)
        entry['measured_hz'] = round(frequency, 6)
        entry['fine_adjustment_cents'] = round(entry['tune_cents'] - entry['upstream_tune_cents'], 6)
        # Neighboring keys play at different rates. Calibrate the early sustained
        # tone at each key's actual rate, including the upstream start offset.
        # A single long-window root correction can miss short-note beating.
        entry['key_tuning_cents'] = {}
        for pitch in range(entry['low'], entry['high'] + 1):
            target_pitch = 440 * 2 ** ((pitch - 69) / 12)
            tune = entry['tune_cents']
            for _ in range(4):
                rate = 2 ** ((pitch - entry['root']) / 12 + tune / 1200)
                start = entry['offset_frames'] / 48000 + rate * (0.10 if target_pitch >= 100 else 0.6)
                duration = rate * (0.40 if target_pitch >= 100 else 3)
                heard = measured_frequency(audio, target, start, duration) * rate
                correction = float(1200 * np.log2(target_pitch / heard))
                tune += correction
                if abs(correction) < 0.05:
                    break
            entry['key_tuning_cents'][str(pitch)] = round(tune, 6)
        entry['bytes'] = output.stat().st_size
        return entry

    with ThreadPoolExecutor(max_workers=4) as pool:
        samples = list(pool.map(prepare, entries))
    for relative, name in [('LICENSE', 'LICENSE.txt'), ('README.md', 'UPSTREAM-README.md')]:
        shutil.copyfile(source(relative), DESTINATION / name)
    (DESTINATION / 'NOTICE.txt').write_text(
        'Salamander Grand Piano V3 — Yamaha C5\n'
        'Recordings by Alexander Holm. Retuned mapping by Markus Fiedler. SFZ reconstruction by kinwie.\n'
        'Source https://github.com/sfzinstruments/SalamanderGrandPiano\n'
        'Licensed under Creative Commons Attribution 3.0 Unported.\n'
        'https://creativecommons.org/licenses/by/3.0/\n\n'
        'Understand Book uses the original v8 medium-touch FLAC recordings unchanged: 48 kHz, 24-bit stereo. No lossy encoding or sample-rate/bit-depth reduction.\n'
        'The upstream instrument records in minor thirds and maps those recordings to adjacent keys.\n'
        'The mapping preserves the upstream fast-response offsets and records the upstream retuned cents.\n'
        'Per-key playback adjustments align the measured early sustained fundamental to A4=440 Hz equal temperament.\n'
        'The reference window is 0.10–0.50 seconds after sample start; low notes below 100 Hz use 0.6–3.6 seconds. Natural string beating and decay remain.\n'
        'Recordings are loaded by musical range. Other velocity layers, release-noise and pedal-noise recordings are available upstream.\n', encoding='utf-8')
    (DESTINATION / 'credits.html').write_text('''<!doctype html>
<html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>钢琴音色来源 · Understand Book</title>
<style>body{font:16px/1.8 system-ui,sans-serif;max-width:640px;margin:60px auto;padding:0 24px;color:#263e35;background:#faf9f5}h1{font-size:26px}a{color:inherit}</style>
<h1>Salamander Grand Piano V3</h1>
<p>Yamaha C5 三角钢琴采样，由 Alexander Holm 录制。Markus Fiedler 提供重调律配置，kinwie 重建 SFZ 映射。</p>
<p>音源文件直接使用原始 48 kHz、24 bit、双声道 FLAC，无损保存，未经有损转码、降采样率或降位深。录音文件保留原始长度，播放时根据谱例时值控制松键。</p>
<p>目前采用中等触键的第 8 力度层。原音源有 16 个力度层，分别记录不同触键轻重产生的音色；其余力度层和踏板、松键噪声尚未接入。原作者按小三度间隔录制，再映射至邻近琴键。</p>
<p>逐键按起音后的持续段基频微调，以 A4＝440 Hz 十二平均律为参照；保留钢琴本身的拍频和自然衰减。浏览器负责解码和音频设备输出。</p>
<p>采样采用 <a href="https://creativecommons.org/licenses/by/3.0/">Creative Commons Attribution 3.0 Unported</a> 授权。</p>
<p><a href="https://github.com/sfzinstruments/SalamanderGrandPiano">原项目与作者说明</a> · <a href="LICENSE.txt">完整许可</a> · <a href="NOTICE.txt">改动记录</a></p>
</html>''', encoding='utf-8')
    pitch_tuning = []
    for pitch in range(128):
        sample = next((item for item in samples if item['low'] <= pitch <= item['high']), samples[0] if pitch < 21 else samples[-1])
        pitch_tuning.append(sample['key_tuning_cents'].get(str(pitch), sample['tune_cents']))
    manifest = {'name': 'Salamander Grand Piano V3', 'velocity_layer': 8, 'sample_rate': 48000,
                'audio_format': 'FLAC', 'bit_depth': 24, 'channels': 2,
                'pitch_tuning_cents': pitch_tuning,
                'source': 'https://github.com/sfzinstruments/SalamanderGrandPiano', 'samples': samples}
    (ROOT / 'packages/web/src/piano-sample-map.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'samples': len(samples), 'total_bytes': sum(item['bytes'] for item in samples),
                      'largest_fine_adjustment_cents': max(abs(item['fine_adjustment_cents']) for item in samples)}))


if __name__ == '__main__':
    main()

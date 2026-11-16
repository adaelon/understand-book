import bank from './piano-sample-map.json';

const decoded = new Map<number, Promise<AudioBuffer>>();

export function pianoSample(pitch: number) {
  return bank.samples.find(sample => sample.low <= pitch && pitch <= sample.high)
    ?? (pitch < bank.samples[0].low ? bank.samples[0] : bank.samples[bank.samples.length - 1]);
}

export function pianoTuning(pitch: number) {
  return bank.pitch_tuning_cents[pitch];
}

/** All samples for a phrase are ready before its first audio-clock onset. */
export async function loadPianoSamples(context: AudioContext, pitches: number[]) {
  const samples = [...new Set(pitches.map(pianoSample))];
  return new Map(await Promise.all(samples.map(async sample => {
    let buffer = decoded.get(sample.root);
    if (!buffer) {
      buffer = (async () => {
        const response = await fetch(`${import.meta.env.BASE_URL}piano/salamander-v3/${encodeURIComponent(sample.file)}`);
        if (!response.ok) throw new Error(`Piano sample ${sample.file}: HTTP ${response.status}`);
        return context.decodeAudioData(await response.arrayBuffer());
      })();
      decoded.set(sample.root, buffer);
      void buffer.catch(() => { decoded.delete(sample.root); });
    }
    return [sample.root, await buffer] as const;
  })));
}

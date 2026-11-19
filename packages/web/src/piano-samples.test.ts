import { afterEach, expect, it, vi } from 'vitest';

afterEach(() => { vi.unstubAllGlobals(); vi.resetModules(); });

it('loads only the distinct recorded keys needed for a phrase and reuses decoded samples', async () => {
  const { loadPianoSamples, pianoSample } = await import('./piano-samples');
  const fetch = vi.fn(async () => new Response(new Uint8Array([1, 2, 3])));
  vi.stubGlobal('fetch', fetch);
  const context = { decodeAudioData: vi.fn(async () => ({} as AudioBuffer)) } as unknown as AudioContext;
  expect(pianoSample(60).root).toBe(60); expect(pianoSample(61).root).toBe(60);
  expect(pianoSample(64).root).toBe(63); expect(pianoSample(69).root).toBe(69);
  expect(pianoSample(0).root).toBe(21); expect(pianoSample(127).root).toBe(108);
  const buffers = await loadPianoSamples(context, [60, 61, 64, 69, 69]);
  expect(buffers.size).toBe(3); expect(fetch).toHaveBeenCalledTimes(3);
  await loadPianoSamples(context, [61, 64]); expect(fetch).toHaveBeenCalledTimes(3);
});

it('retries a failed sample instead of retaining its rejected load', async () => {
  const { loadPianoSamples } = await import('./piano-samples');
  const fetch = vi.fn().mockResolvedValueOnce(new Response('', { status: 503 })).mockResolvedValue(new Response(new Uint8Array([1])));
  vi.stubGlobal('fetch', fetch);
  const context = { decodeAudioData: vi.fn(async () => ({} as AudioBuffer)) } as unknown as AudioContext;
  await expect(loadPianoSamples(context, [69])).rejects.toThrow('HTTP 503');
  expect((await loadPianoSamples(context, [69])).size).toBe(1);
  expect(fetch).toHaveBeenCalledTimes(2);
});

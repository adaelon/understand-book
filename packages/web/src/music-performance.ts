/** Bounded development measurements used by the real-material visual review page. */
const samples = new Map<string, number[]>();
export function recordMusicTiming(name: string, duration: number) {
  if (!import.meta.env.DEV) return;
  let values = samples.get(name); if (!values) { values = []; samples.set(name, values); }
  if (values.length >= 600) values.shift();
  values.push(duration);
}
export function musicPerformanceSnapshot() {
  return Object.fromEntries([...samples].map(([name, values]) => {
    const sorted = [...values].sort((a, b) => a - b);
    return [name, { count: values.length, median_ms: sorted[Math.floor(sorted.length / 2)], p95_ms: sorted[Math.max(0, Math.ceil(sorted.length * .95) - 1)], max_ms: sorted.at(-1), total_ms: values.reduce((a, b) => a + b, 0) }];
  }));
}

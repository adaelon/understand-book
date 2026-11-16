import type { MusicContext } from './music-api';
import { MusicTimeline } from './music-playback';
import { t as uiText } from './i18n';

type ScorePage = MusicContext['score_pages'][number];
type Manifest = MusicContext['manifest'];
type PageTurn = { ms: number; id: string };

/** A performance can repeat a page or skip a printed passage. */
export function scorePageTurns(manifest: Manifest, objects: ReadonlyMap<string, Manifest['score_objects'][number]>) {
  const turns: PageTurn[] = [];
  function add(ms: number, id?: string) {
    if (id && id !== turns.at(-1)?.id) turns.push({ ms, id });
  }
  const pageEvents = manifest.events.flatMap(event => {
    const id = event.evidence.flatMap(item => item.score_page_ids ?? [])[0];
    return id ? [{ start_ms: event.start_ms, end_ms: event.end_ms, id }] : [];
  });
  if (!pageEvents.length) {
    for (const bar of manifest.occurrences) add(bar.start_ms, objects.get(bar.score_object_ids[0])?.page_id);
    return turns;
  }
  // Where aligned bars end, the reader can continue with timed PDF page evidence.
  const coverage = new MusicTimeline(manifest.coverage.filter(range => range.status === 'accepted' && range.granularity !== 'phrase'));
  const bars = new MusicTimeline(manifest.occurrences), evidence = new MusicTimeline(pageEvents);
  const boundaries = new Set<number>();
  for (const ranges of [bars.entries, evidence.entries, coverage.entries]) {
    for (const range of ranges) { boundaries.add(range.start_ms); boundaries.add(range.end_ms); }
  }
  for (const ms of [...boundaries].sort((a, b) => a - b)) {
    const bar = coverage.at(ms).length ? bars.at(ms).at(-1) : undefined;
    add(ms, (bar && objects.get(bar.score_object_ids[0])?.page_id) ?? evidence.at(ms).at(-1)?.id);
  }
  return turns;
}

export function upcomingScorePages(turns: PageTurn[], current: string, ms: number, loop: { start_ms: number; end_ms: number } | null, rate: number) {
  const index = turns.findIndex(turn => turn.ms > ms && turn.id !== current);
  const next = turns[index];
  if (loop && (!next || next.ms >= loop.end_ms)) {
    const first = turns.filter(turn => turn.ms <= loop.start_ms).at(-1)?.id;
    return { next: first !== current ? first : undefined };
  }
  const later = next && turns[index + 1];
  // Short orchestral pages can pass in 3–6 seconds. Warm one more file, without mounting it.
  const soonAfter = later && later.id !== current && (!loop || later.ms < loop.end_ms)
    && later.ms - next.ms < 8000 * rate ? later.id : undefined;
  return { next: next?.id, soonAfter };
}

/** One reader context. Keep source text for nearby returns without retaining offscreen SVG trees. */
export function createScorePageReader(signal: AbortSignal) {
  const recent = new Map<string, string>();
  const downloads = new Map<string, Promise<string>>();

  async function download(page: ScorePage, priority: RequestPriority) {
    const response = await fetch(page.url, { credentials: 'same-origin', signal, priority });
    if (!response.ok) throw new Error(uiText('乐谱暂不可读取，请重试'));
    const svg = await response.text();
    const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
    if (parsed.querySelector('parsererror') || parsed.documentElement.localName !== 'svg') throw new Error(uiText('乐谱内容无法显示'));
    recent.set(page.page_id, svg);
    if (recent.size > 4) recent.delete(recent.keys().next().value!);
    return svg;
  }
  return function read(page: ScorePage, priority: RequestPriority) {
    const id = page.page_id, cached = recent.get(id);
    if (cached !== undefined) {
      recent.delete(id); recent.set(id, cached);
      return Promise.resolve(cached);
    }
    const pending = downloads.get(id);
    if (pending) return pending;
    const request = download(page, priority).finally(() => downloads.delete(id));
    downloads.set(id, request);
    return request;
  };
}

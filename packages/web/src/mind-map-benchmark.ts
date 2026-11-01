import '@fontsource-variable/noto-sans-sc';
import { makeRustMindMapFixture } from './fixtures/mind-map';
import { layoutMindMaps, MIND_MAP_FONT_FAMILY } from './mind-map-layout';
import { exportMindMapPng } from './mind-map-export';
import type { BoardLink } from './generated/BoardLink';

const asBase64 = async (blob: Blob) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(',')[1]); reader.onerror = reject; reader.readAsDataURL(blob);
});

async function runMindMapBenchmark(titles: string[]) {
  const allText = titles.join('') || makeRustMindMapFixture(500).items.map(item => item.title).join('');
  const loadStart = performance.now();
  await document.fonts.load(`500 16px ${MIND_MAP_FONT_FAMILY}`, allText);
  await document.fonts.ready;
  const fontLoadMs = performance.now() - loadStart;
  const measurements = [];
  for (const count of [500, 1000]) {
    const fixture = makeRustMindMapFixture(count, titles.length ? titles : undefined), samples: number[] = [];
    let last: ReturnType<typeof layoutMindMaps> | undefined;
    for (let index = 0; index < 11; index++) {
      fixture.items[20].title = `${titles[19] || '切片借用数组的一段'} · 修改 ${index}`;
      const start = performance.now(); last = layoutMindMaps(fixture.items, fixture.mindMaps); samples.push(performance.now() - start);
    }
    const sorted = samples.slice(1).sort((a, b) => a - b);
    measurements.push({ topics: count, measured_nodes: last!.nodes.length, first_layout_ms: samples[0], warm_samples_ms: samples.slice(1),
      median_ms: (sorted[4] + sorted[5]) / 2, max_ms: sorted.at(-1), bounds: last!.bounds });
  }
  const small = makeRustMindMapFixture(32), initial = layoutMindMaps(small.items, small.mindMaps, { collapsedIds: ['topic-1'] });
  const images = [];
  for (const [name, rootItemId] of [['rust-full.png', undefined], ['rust-branch.png', 'topic-1']] as const) {
    const start = performance.now(), blob = await exportMindMapPng(initial, { rootItemId, pixelRatio: 2 });
    const bitmap = await createImageBitmap(blob);
    images.push({ name, base64: await asBase64(blob), width: bitmap.width, height: bitmap.height, bytes: blob.size, elapsed_ms: performance.now() - start }); bitmap.close();
  }
  const linkFixture = makeRustMindMapFixture(4);
  linkFixture.items[0].title = '借用的两种权限'; linkFixture.items[1].title = '只读引用'; linkFixture.items[2].title = '原数组'; linkFixture.items[3].title = '可写引用';
  const links: BoardLink[] = [{ link_id: 'contrast', from: { kind: 'item', item_id: 'topic-1' }, to: { kind: 'item', item_id: 'topic-3' }, label: '同一时刻不能混用', based_on: [] }];
  const linkedStart = performance.now(), linkedBlob = await exportMindMapPng(layoutMindMaps(linkFixture.items, linkFixture.mindMaps), { links });
  const linkedBitmap = await createImageBitmap(linkedBlob);
  images.push({ name: 'rust-relations.png', base64: await asBase64(linkedBlob), width: linkedBitmap.width, height: linkedBitmap.height, bytes: linkedBlob.size, elapsed_ms: performance.now() - linkedStart }); linkedBitmap.close();
  let oversizedExport = '';
  const oversized = makeRustMindMapFixture(1000, titles.length ? titles : undefined);
  try { await exportMindMapPng(layoutMindMaps(oversized.items, oversized.mindMaps)); }
  catch (error) { oversizedExport = String(error); }
  if (!oversizedExport.includes('选中一个分支')) throw new Error('Oversized export did not preserve readable pixel size and request a branch.');
  const png = images.map(({ base64: _base64, ...metadata }) => metadata);
  return { font_load_ms: fontLoadMs, measurements, png, oversized_export: oversizedExport, images };
}

declare global { interface Window { runMindMapBenchmark: typeof runMindMapBenchmark } }
window.runMindMapBenchmark = runMindMapBenchmark;

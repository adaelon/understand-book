import { describe, expect, it } from 'vitest';
import { activeOutline, readerOutline, type ReaderManifest } from './reader-outline';

const manifest: ReaderManifest = { stats_by_lid: {}, tree: [
  { lid:'1', kind:'section', children:['1.1','1.2','1.3'], span:{ start:0, end:200 }, title:'![Image](images/image.jpeg)' },
  { lid:'1.1', kind:'paragraph', children:[], span:{ start:0, end:20 } },
  { lid:'1.2', kind:'paragraph', children:[], span:{ start:20, end:40 } },
  { lid:'1.3', kind:'paragraph', children:[], span:{ start:100, end:200 } },
] };
describe('restored reader outline', () => {
  it('uses real leaf anchors and follows reading between headings', () => {
    const entries = [
      { lid:'1.2', kind:'section' as const, depth:0, title:'引言', span:{ start:20, end:100 } },
      { lid:'1.3', kind:'section' as const, depth:0, title:'第一章 若干事例', span:{ start:100, end:200 } },
    ];
    const items=readerOutline({ ...manifest, reader_outline:entries }, '正文');
    expect(items.map(item => item.lid)).toEqual(['1.2','1.3']);
    expect(activeOutline(items,'1.18',75)?.title).toBe('引言');
    expect(activeOutline(items,'1.3',100)?.title).toBe('第一章 若干事例');
    expect(activeOutline(items,'1.99',199)?.title).toBe('第一章 若干事例');
    expect(activeOutline(items,'1.1',0)).toBeNull();
  });
  it('never uses an image link as the fallback directory title', () => {
    expect(readerOutline(manifest,'正文').map(item => item.title)).toEqual(['正文']);
  });
  it('preserves native heading hierarchy for books without a restored outline', () => {
    const entries = readerOutline({ ...manifest, tree:[
      { ...manifest.tree[0], lid:'2', kind:'chapter', title:'# 原章节' },
      { ...manifest.tree[0], lid:'2.1', title:'## 原小节' },
    ] }, '正文');
    expect(activeOutline(entries,'2.1.3')?.title).toBe('原小节');
  });
});

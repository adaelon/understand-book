// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { recapEntries, recapShareSource, recapTime, type SessionRecap } from './session-recap';
import { shareSections } from './reading-share';

it('formats native millisecond recap cutoffs consistently with Unix seconds and ISO history', () => {
  const iso = '2026-10-10T09:00:00+08:00', millis = Date.parse(iso);
  expect(recapTime(String(millis))).toBe(recapTime(iso));
  expect(recapTime(String(millis / 1000))).toBe(recapTime(iso));
  expect(recapTime('unknown')).toBe('unknown');
});

it('RS4 copies only selected facts, keeps source/unavailable/status text and excludes private navigation fields', () => {
  const evidence = [{ turn_id: 'private-turn', event_seq: 2 }];
  const recap: SessionRecap = {
    session_id: 'private-chat', through_seq: 12, through_at: '2026-10-10T09:00:00+08:00', generated_at: '2026-10-10T09:01:00+08:00',
    questions: [{ text: '选中的问题 $price * 2', status: 'answered', evidence }, { text: '未选的私人问题', status: 'running', evidence: [{ turn_id: 'other', event_seq: 3 }] }],
    sources: [{ source_ref_id: 'private-source', label: '《原材料》· 第一章', quote: '保留的原句', published_book_ref: { book_id: 'private-book', publication_id: 'private-publication' }, evidence, unavailable_reason: '原发布当前不可用' }],
    effects: [{ effect_id: 'private-effect', label: '笔记：适用范围', status: 'undone', object_id: null, published_book_ref: null, evidence, unavailable_reason: '原成果已撤销', effect: { kind: 'presentation', reference: { presentation_id: 'private-presentation', revision: 2 } } }],
    continuations: [{ text: '下次比较边界', status: 'open', goal_id: 'private-goal', evidence }],
  };
  const entries = recapEntries(recap);
  expect(new Set(entries.map(item => item.id)).size).toBe(5);
  const source = recapShareSource(recap, entries.filter(item => item.text !== '未选的私人问题').map(item => item.id), '前提与结论');
  const snapshot = JSON.stringify(source);
  const output = shareSections({ source, selected: source.parts.map(part => part.id), title: '本次阅读回顾', reflection: '', layout: 'recap', palette: 'blue' });
  const text = [...output.map(section => section.textContent), source.association, ...source.sources].join('\n');
  for (const expected of ['已回答', '选中的问题 $price * 2', '《原材料》· 第一章', '保留的原句', '原发布当前不可用', '已撤销', '原成果已撤销', '待继续', '下次比较边界', '前提与结论', '截至']) expect(text).toContain(expected);
  for (const excluded of ['未选的私人问题', 'private-', '已懂', 'through_seq']) expect(text).not.toContain(excluded);
  expect(output[0].querySelector('.katex,em')).toBeNull();
  recap.questions[0].text = '后来改动'; recap.sources[0].label = '其他材料'; recap.through_seq = 20;
  expect(JSON.stringify(source)).toBe(snapshot);
  expect(recapShareSource(recap, []).parts).toEqual([]);
});

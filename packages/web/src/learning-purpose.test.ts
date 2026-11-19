import { expect, it } from 'vitest';
import { currentLearningSection } from './learning-purpose';
import { readerOutline, type ReaderManifest } from './reader-outline';

// Same shape as quantification-essence: one document title, then real subsections.
const manifest: ReaderManifest = { stats_by_lid: {}, tree: [
  { lid: '1', kind: 'chapter', title: '你认为量化的本质是什么？', children: ['1.1', '1.2', '1.3'], span: { start: 0, end: 500 } },
  { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: 18 } },
  { lid: '1.2', kind: 'section', title: '引言', children: ['1.2.1'], span: { start: 18, end: 200 } },
  { lid: '1.2.1', kind: 'paragraph', children: [], span: { start: 24, end: 200 } },
  { lid: '1.3', kind: 'section', title: '经典的 LSTM 股价预测', children: ['1.3.1'], span: { start: 200, end: 500 } },
  { lid: '1.3.1', kind: 'paragraph', children: [], span: { start: 230, end: 500 } },
] };
it('uses the real subsection instead of treating the document title as a chapter', () => {
  const outline = readerOutline(manifest, '正文');
  expect(currentLearningSection(manifest, outline, '1.2.1')).toMatchObject({ lid: '1.2', title: '引言', kind: 'section' });
  expect(currentLearningSection(manifest, outline, '1.3.1')).toMatchObject({ lid: '1.3', title: '经典的 LSTM 股价预测' });
  expect(currentLearningSection(manifest, outline, '1.1')).toBeNull();
  expect(currentLearningSection(manifest, outline, '1')).toBeNull();
});
it('keeps an actual chapter when it covers only part of the material', () => {
  const chapters = structuredClone(manifest);
  chapters.tree.push({ lid: '2', kind: 'chapter', title: '后续章节', children: [], span: { start: 500, end: 700 } });
  expect(currentLearningSection(chapters, readerOutline(chapters, '正文'), '1.1')).toMatchObject({ lid: '1', kind: 'chapter' });
});

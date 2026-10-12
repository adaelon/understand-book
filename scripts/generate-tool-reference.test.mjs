import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createReference, executorDefinitions, renderReference, writeReference } from './generate-tool-reference.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const reference = JSON.parse(readFileSync(path.join(ROOT, 'docs/tool-contracts.json'), 'utf8'));
const markdown = renderReference(reference);
const surfaces = ['resident', 'local_mcp', 'remote_mcp', 'executor'];
const examplesBlock = examples => `\`\`\`json\n${JSON.stringify(examples, null, 2)}\n\`\`\``;

test('every exported tool has exactly one entry and preserves its published examples', () => {
  for (const surface of surfaces) {
    const section = markdown.split(`<a id="${surface}"></a>`)[1].split(/\n## /)[1];
    const headings = [...section.matchAll(/^### (.+)$/gm)].map(match => match[1]);
    assert.deepEqual(headings, reference[surface].map(tool => tool.name));
    for (const tool of reference[surface]) {
      const entry = section.split(`### ${tool.name}\n`)[1].split('\n### ')[0];
      assert.ok(entry.includes(tool.description), tool.name);
      if (tool.inputSchema.examples?.length) assert.ok(entry.includes(examplesBlock(tool.inputSchema.examples)), tool.name);
      assert.ok(entry.includes(tool.outputSchema ? '| 字段路径 |' : '当前定义未发布 outputSchema'), tool.name);
    }
  }
  assert.ok(reference.resident.some(tool => tool.name === 'reader.note'));
  assert.ok(!reference.remote_mcp.some(tool => /reader|query|synthesize/.test(tool.name)));
  assert.ok(reference.executor.every(tool => tool.name.startsWith('executor.')));
});

test('Tutor contexts, unavailable states and state-projected examples stay paired', () => {
  for (const [name, state] of Object.entries(reference.tutor_states)) {
    const entry = markdown.split(`### ${name}\n`)[1].split('\n### ')[0];
    assert.ok(entry.includes(examplesBlock(state.context)), name);
    if (state.spec) assert.ok(entry.includes(examplesBlock(state.spec.parameters.examples)), name);
    else assert.ok(entry.includes('该状态不暴露 tutor.step'), name);
  }
});

test('exported common fields and examples reach each Book surface in the document', () => {
  const changed = structuredClone(reference);
  for (const [surface, name] of [['resident', 'book.text'], ['local_mcp', 'book_text'], ['remote_mcp', 'book_text']]) {
    const tool = changed[surface].find(tool => tool.name === name);
    tool.inputSchema.properties.lid.description = `TO13 ${surface} field change`;
    tool.inputSchema.examples[0].lid = `TO13 ${surface} example change`;
  }
  const result = renderReference(changed);
  for (const surface of ['resident', 'local_mcp', 'remote_mcp']) {
    assert.ok(result.includes(`TO13 ${surface} field change`));
    assert.ok(result.includes(`TO13 ${surface} example change`));
  }
  assert.notEqual(result, markdown);
});

test('new authority entries are included without editing the generator tool list', () => {
  const changed = structuredClone(reference);
  const newTool = { ...changed.resident[0], name: 'future.tool' };
  changed.resident.push(newTool);
  const rebuilt = createReference(changed, changed.tutor_states, changed.executor);
  assert.ok(renderReference(rebuilt).includes('### future.tool\n'));
  assert.equal(rebuilt.resident.length, reference.resident.length + 1);
});

test('Executor directory is the actual MCP projection of the current TypeScript contracts', () => {
  const actual = executorDefinitions(path.join(tmpdir(), 'ub-to13-discovery-only'));
  assert.deepEqual([...actual].sort((a, b) => a.name < b.name ? -1 : 1), reference.executor);
});

test('generated links resolve and examples are valid JSON after Markdown rendering', () => {
  const anchors = new Set([...markdown.matchAll(/<a id="([^"]+)"><\/a>/g)].map(match => match[1]));
  for (const [, target] of markdown.matchAll(/\]\(([^)]+)\)/g)) {
    if (target.startsWith('#')) assert.ok(anchors.has(target.slice(1)), target);
    else assert.ok(existsSync(path.resolve(ROOT, 'docs', target.split('#')[0])), target);
  }
  for (const [, body] of markdown.matchAll(/```json\n([\s\S]*?)\n```/g)) assert.doesNotThrow(() => JSON.parse(body));
});

test('check detects missing or stale artifacts, accepts CRLF, and never rewrites files', () => {
  const temp = mkdtempSync(path.join(tmpdir(), 'ub-to13-check-'));
  try {
    assert.throws(() => writeReference(reference, temp, true), /stale or missing/);
    writeReference(reference, temp);
    writeReference(reference, temp, true);
    const mdPath = path.join(temp, '工具契约参考.md');
    writeFileSync(mdPath, markdown.replaceAll('\n', '\r\n'));
    writeReference(reference, temp, true);
    writeFileSync(mdPath, 'stale');
    assert.throws(() => writeReference(reference, temp, true), /工具契约参考.md is stale/);
    assert.equal(readFileSync(mdPath, 'utf8'), 'stale');
    writeReference(reference, temp);
    writeFileSync(path.join(temp, 'tool-contracts.json'), '{}');
    assert.throws(() => writeReference(reference, temp, true), /tool-contracts.json is stale/);
  } finally {
    assert.equal(path.dirname(temp), path.resolve(tmpdir()));
    rmSync(temp, { recursive: true, force: true });
  }
});

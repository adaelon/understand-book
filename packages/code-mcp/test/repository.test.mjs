import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, unlink } from 'node:fs/promises';
import path from 'node:path';
import { fixture } from './fixture.mjs';
const texts = result => result.segments.map(p => p.text).join('\n');

test('live reads include edits and untracked source, preserving Chinese paths and line numbers', async t => {
  const { repository, write } = await fixture(t);
  await write('packages/demo/src/main.ts', 'current\nnew behavior\n');
  await write('docs/新设计.md', '# 新设计\n当前内容\n');
  assert.equal(texts(await repository.read({ path: 'packages/demo/src/main.ts' })), 'current\nnew behavior');
  assert.ok((await repository.list()).files.includes('docs/新设计.md'));
  const result = await repository.search({ query: '当前' });
  assert.deepEqual(result.matches[0], { path: 'docs/新设计.md', line: 2, column: 1, snippet: '当前内容', snippet_start_column: 1, snippet_truncated: false });
  await write('docs/新设计.md', 'updated again');
  assert.equal(texts(await repository.read({ path: 'docs/新设计.md' })), 'updated again');
});

test('all entry points exclude actual private/output locations, ignored source and root handoffs', async t => {
  const { repository, write, git } = await fixture(t);
  const denied = ['.env', '.codex/config.toml', '.executor-private/input.json', 'tmp/snapshot/packages/main.ts',
    'handoff-session.md', 'packages/demo/.env', 'packages/demo/credentials.json', 'packages/demo/private.json',
    'packages/demo/node_modules/lib/index.js', 'packages/demo/dist/main.js', 'packages/ignored.ts',
    'docs/performance/run.json', 'docs/performance/run.traces/evidence.md', 'apps/desktop/src-tauri/resources/settings.json'];
  for (const file of denied) await write(file, 'PRIVATE_MARKER');
  await git('add', '-f', '.env', 'packages/demo/credentials.json');
  const listed = (await repository.list()).files;
  const changed = (await repository.changes()).changes.map(p => p.path);
  assert.equal((await repository.search({ query: 'PRIVATE_MARKER' })).matches.length, 0);
  for (const file of denied) {
    assert.ok(!listed.includes(file), file);
    assert.ok(!changed.includes(file), file);
    await assert.rejects(repository.read({ path: file }));
    await assert.rejects(repository.changes({ path: file }));
  }
  await write('skills/build/driver.ts', 'allowed build skill');
  await write('plugins/example/skills/build/SKILL.md', 'allowed plugin skill');
  await write('crates/memory/src/lib.rs', 'pub struct MemoryStore;');
  assert.ok((await repository.list()).files.includes('skills/build/driver.ts'));
  assert.ok((await repository.list()).files.includes('plugins/example/skills/build/SKILL.md'));
  assert.ok((await repository.list()).files.includes('crates/memory/src/lib.rs'));
});

test('traversal and absolute paths are rejected at read and diff boundaries', async t => {
  const { repository } = await fixture(t);
  for (const file of ['../secret.md', 'packages/../../secret.md', 'C:\\secret.md', '/etc/passwd', 'packages/x.ts:stream', 'packages/*']) {
    await assert.rejects(repository.read({ path: file }));
    await assert.rejects(repository.changes({ path: file }));
  }
});

test('list/search cursors do not drop results; literal matching does not interpret regex', async t => {
  const { repository, write } = await fixture(t);
  await write('packages/demo/src/a.ts', 'a.b\na.b\naxb\n');
  await write('packages/demo/src/b.ts', 'a.b\n');
  const files = []; let after;
  do {
    const result = await repository.list({ path: 'packages', limit: 1, after });
    files.push(...result.files); after = result.next_after;
  } while (after);
  assert.deepEqual(files, (await repository.list({ path: 'packages' })).files);
  const matches = []; after = undefined;
  do {
    const result = await repository.search({ query: 'a.b', limit: 1, after });
    matches.push(...result.matches); after = result.next_after;
  } while (after);
  assert.equal(matches.length, 3);
  assert.deepEqual(matches.map(p => p.line), [1, 2, 1]);
});

test('read continuation preserves a long line and an explicit range', async t => {
  const { repository, write } = await fixture(t);
  const long = 'x'.repeat(25000);
  await write('docs/long.md', 'first\n' + long + '\nlast\n');
  let args = { path: 'docs/long.md', start_line: 2, end_line: 2 };
  let combined = '';
  do {
    const result = await repository.read(args);
    combined += result.segments.map(s => s.text).join('');
    args = result.next && { path: args.path, ...result.next };
  } while (args);
  assert.equal(combined, long);
  await assert.rejects(repository.read({ path: 'docs/long.md', start_line: 9 }));
  await write('docs/empty.md', '');
  assert.deepEqual((await repository.read({ path: 'docs/empty.md' })).segments, []);
});

test('binary and oversized text are refused and search reports skipped files', async t => {
  const { repository, write } = await fixture(t);
  await write('packages/demo/binary.ts', Buffer.from([0, 1, 2]));
  await write('packages/demo/huge.ts', 'x'.repeat(2 * 1024 * 1024 + 1));
  await assert.rejects(repository.read({ path: 'packages/demo/binary.ts' }), /Binary/);
  await assert.rejects(repository.read({ path: 'packages/demo/huge.ts' }), /2 MiB/);
  assert.equal((await repository.search({ query: 'unmatched', path: 'packages/demo' })).skipped_files, 2);
});

test('diff includes staged and unstaged changes, new/deleted files, and never invokes external drivers', async t => {
  const { root, repository, write, git } = await fixture(t);
  const file = 'packages/demo/src/main.ts';
  await write(file, 'staged\nsecond line\n');
  await git('add', file);
  await write(file, 'staged\nworking\n');
  await write('.gitattributes', '*.ts diff=forbidden\n');
  await git('config', 'diff.forbidden.command', 'this-command-must-never-run');
  await git('config', 'diff.forbidden.textconv', 'this-command-must-never-run');
  const before = await readFile(path.join(root, '.git/index'));
  const result = await repository.changes({ path: file });
  assert.match(texts(result), /\+staged/);
  assert.match(texts(result), /\+working/);
  assert.match(texts(result), /-export const original/);
  await write('packages/demo/new.ts', 'new source');
  assert.equal((await repository.changes({ path: 'packages/demo/new.ts' })).status, '??');
  await unlink(path.join(root, 'docs/architecture.md'));
  assert.match(texts(await repository.changes({ path: 'docs/architecture.md' })), /-# Architecture/);
  assert.deepEqual(await readFile(path.join(root, '.git/index')), before);
});

test('new repository diff does not omit unstaged changes after staging', async t => {
  const { repository, write } = await fixture(t, false);
  await write('packages/demo/src/main.ts', 'unstaged after initial add');
  const result = await repository.changes({ path: 'packages/demo/src/main.ts' });
  assert.match(texts(result), /\+export const original/);
  assert.match(texts(result), /\+unstaged after initial add/);
  assert.equal((await repository.overview()).head, null);
});

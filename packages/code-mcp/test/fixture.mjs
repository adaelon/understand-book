import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';
import { ProjectRepository } from '../repository.mjs';
const exec = promisify(execFile);

export async function fixture(t, commit = true) {
  const root = await mkdtemp(path.join(tmpdir(), 'ub-code-mcp-'));
  t.after(async () => {
    assert.ok(path.resolve(root).startsWith(path.join(path.resolve(tmpdir()), 'ub-code-mcp-')));
    await rm(root, { recursive: true, force: true });
  });
  const git = async (...args) => (await exec('git', ['-C', root, ...args], { windowsHide: true })).stdout;
  const write = async (file, contents) => {
    await mkdir(path.dirname(path.join(root, file)), { recursive: true });
    await writeFile(path.join(root, file), contents);
  };
  await git('init', '-q');
  await git('config', 'core.autocrlf', 'false');
  await write('.gitignore', 'packages/ignored.ts\n');
  await write('README.md', '# Sample project\nArchitecture entry\n');
  await write('packages/demo/src/main.ts', 'export const original = 1;\nsecond line\n');
  await write('docs/architecture.md', '# Architecture\nmain.ts owns the example\n');
  await git('add', '.');
  if (commit) await git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-qm', 'fixture');
  return { root, git, write, repository: await new ProjectRepository(root).initialize() };
}

import assert from 'node:assert/strict';
import { test } from 'node:test';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { resolvePluginConfig } from '../src/config.ts';

test('explicit Engine and registry paths remain authoritative; invalid installation fails locally', () => {
  const driverRoot = path.join(tmpdir(), '中文 registry');
  const config = resolvePluginConfig({ executable: process.execPath, driverRoot });
  assert.equal(config.executable, process.execPath);
  assert.equal(config.driverRoot, driverRoot);
  assert.equal(config.handoffsPerCall, 8);
  assert.throws(() => resolvePluginConfig({ executable: path.join(driverRoot, 'missing.exe') }), /build_engine_missing/);
  assert.throws(() => resolvePluginConfig({ executable: process.execPath, driverRoot: 'relative' }), /configuration_invalid/);
  assert.throws(() => resolvePluginConfig({ executable: process.execPath, maxOutputTokens: 0 }), /configuration_invalid/);
});

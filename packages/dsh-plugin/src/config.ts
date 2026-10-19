import { existsSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import type { BuildControlConfig } from './build-control.ts';
import type { OperatorBuildHostConfig } from './operator-build-host.ts';

export type PluginConfig = Partial<BuildControlConfig> & {operatorBuild?:OperatorBuildHostConfig};

function installedEngine(): string | undefined {
  if (process.platform !== 'win32') return undefined;
  // reg.exe uses the machine's console code page; ask PowerShell for explicit UTF-8
  // so an installed Reader path containing Chinese is preserved.
  const result = spawnSync(path.join(process.env.SystemRoot ?? 'C:/Windows', 'System32/WindowsPowerShell/v1.0/powershell.exe'),
    ['-NoProfile', '-NonInteractive', '-Command',
      '[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); ' +
      '$value = (Get-ItemProperty -LiteralPath HKCU:\\Software\\UnderstandBook -Name InstallDir -ErrorAction SilentlyContinue).InstallDir; ' +
      'if ($value) { $value | ConvertTo-Json -Compress }'],
    { encoding: 'utf8', windowsHide: true, shell: false });
  const directory = result.status === 0 && result.stdout.trim() ? JSON.parse(result.stdout.trim()) : undefined;
  return directory ? path.join(directory, 'understand-book-build.exe') : undefined;
}

/** An explicit path wins; discovery only reads the Reader installer's own registry key. */
export function resolvePluginConfig(input: PluginConfig = {}): BuildControlConfig & {operatorBuild?:OperatorBuildHostConfig} {
  const executable = input.executable || process.env.UNDERSTAND_BOOK_DSH_ENGINE || installedEngine();
  if (!executable || !path.isAbsolute(executable) || !existsSync(executable)) throw new Error('build_engine_missing');
  const driverRoot = input.driverRoot || process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT
    || path.join(tmpdir(), 'understand-book-automatic-build-driver-v1');
  const costConfigPath = process.env.UNDERSTAND_BOOK_EXTERNAL_BUILD_COST_CONFIG;
  const externalCost = input.externalCost ?? (costConfigPath ? JSON.parse(readFileSync(costConfigPath, 'utf8')) as BuildControlConfig['externalCost'] : undefined);
  const operatorPath = process.env.UNDERSTAND_BOOK_OPERATOR_BUILD_CONFIG;
  const operatorBuild = input.operatorBuild ?? (operatorPath ? JSON.parse(readFileSync(operatorPath, 'utf8')) as OperatorBuildHostConfig : undefined);
  if (operatorBuild && (!operatorBuild.rootSessionId || !operatorBuild.connection?.base_url)) throw new Error('operator_build_configuration_invalid');
  if (operatorBuild?.video && ([operatorBuild.video.codexExecutable,operatorBuild.video.publisherExecutable]
    .some(value=>typeof value !== 'string' || !path.isAbsolute(value))
    || operatorBuild.video.publicationServiceRoot !== undefined && !path.isAbsolute(operatorBuild.video.publicationServiceRoot))) {
    throw new Error('operator_video_configuration_invalid');
  }
  const config = { ...input, executable, driverRoot, ...(externalCost ? { externalCost } : {}), ...(operatorBuild ? {operatorBuild} : {}),
    maxOutputTokens: input.maxOutputTokens ?? 4096,
    safetyMarginTokens: input.safetyMarginTokens ?? 4096,
    handoffsPerCall: input.handoffsPerCall ?? 8 };
  if (!path.isAbsolute(driverRoot) || !Number.isSafeInteger(config.maxOutputTokens) || config.maxOutputTokens < 1
    || !Number.isSafeInteger(config.safetyMarginTokens) || config.safetyMarginTokens < 0
    || !Number.isSafeInteger(config.handoffsPerCall) || config.handoffsPerCall < 1) {
    throw new Error('build_plugin_configuration_invalid');
  }
  return config;
}

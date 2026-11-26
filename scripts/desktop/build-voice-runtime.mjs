import {
  closeSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, openSync,
  readFileSync, readSync, readdirSync, renameSync, rmSync, statSync, writeFileSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { desktopTarget, repoRoot, run } from './platform.mjs';

const pythonVersion = '3.13.17';
const pythonArchive = `python-${pythonVersion}-embed-amd64.zip`;
const modelRevision = '536b0662742c02347bc0e980a01041f333bce120';
const modelSource = `https://huggingface.co/Systran/faster-whisper-small/resolve/${modelRevision}`;
const modelBin = 'model.bin';
const modelSize = 483_546_902;
const modelSha256 = '3e305921506d8872816023e4c273e75d2419fb89b24da97b4fe7bce14170d671';
const modelFiles = ['config.json', 'tokenizer.json', 'vocabulary.txt', modelBin];
const modelNotices = ['MODEL-CARD.md', 'LICENSE-MIT.txt'];
const msvcFiles = ['msvcp140.dll', 'msvcp140_1.dll', 'vcruntime140.dll', 'vcruntime140_1.dll'];
const nativeLicenses = {
  'FFmpeg-COPYING.GPLv2': 'https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.1.3/COPYING.GPLv2',
  'CTranslate2-LICENSE.txt': 'https://raw.githubusercontent.com/OpenNMT/CTranslate2/v4.8.2/LICENSE',
  'FlatBuffers-LICENSE.txt': 'https://raw.githubusercontent.com/google/flatbuffers/v25.12.19/LICENSE',
  'Tokenizers-LICENSE.txt': 'https://raw.githubusercontent.com/huggingface/tokenizers/v0.23.3/LICENSE',
};
const requirementsFile = fileURLToPath(new URL('voice-requirements-windows-x64.txt', import.meta.url));
const pipTarget = ['--platform', 'win_amd64', '--python-version', '3.13', '--implementation', 'cp', '--abi', 'cp313', '--only-binary=:all:'];
const cacheRoot = path.join(repoRoot, '.cache', 'desktop-voice', 'windows-x64');
const prepared = path.join(cacheRoot, 'runtime');

function lockedPackages() {
  return readFileSync(requirementsFile, 'utf8').trim().split(/\r?\n/).map(line => {
    const [name, version] = line.split('==');
    return `${name.toLowerCase().replace(/-/g, '_')}-${version}`;
  });
}

function runtimeFiles() {
  const metadata = lockedPackages().map(name => `python/packages/${name}.dist-info/METADATA`);
  return [
    'python/python.exe', 'python/python313.dll', 'python/python313.zip', 'python/python313._pth',
    'python/LICENSE.txt', ...msvcFiles.map(name => `python/${name}`),
    'THIRD-PARTY.md', ...Object.keys(nativeLicenses), ...metadata,
  ];
}

export function assertVoiceRuntime(output, { allowBundledModel = false } = {}) {
  if (desktopTarget().platform !== 'win32') return;
  for (const relative of runtimeFiles()) {
    const status = statSync(path.join(output, relative), { throwIfNoEntry: false });
    if (!status?.isFile() || status.size === 0) {
      throw new Error(`Missing voice runtime file ${relative}. Run node scripts/desktop/build-voice-runtime.mjs --prepare.`);
    }
  }
  if (!allowBundledModel && existsSync(path.join(output, 'model', modelBin))) {
    throw new Error('Whisper model.bin must not be bundled in Setup; download it on demand in Desktop Settings.');
  }
}

export function buildVoiceRuntime(output) {
  if (desktopTarget().platform !== 'win32') return;
  assertVoiceRuntime(prepared, { allowBundledModel: true });
  mkdirSync(output, { recursive: true });
  cpSync(path.join(prepared, 'python'), path.join(output, 'python'), { recursive: true });
  for (const file of ['THIRD-PARTY.md', ...Object.keys(nativeLicenses)]) {
    copyFileSync(path.join(prepared, file), path.join(output, file));
  }
  assertVoiceRuntime(output);
}

function fetchFile(url, destination) {
  if (existsSync(destination)) return;
  const temporary = `${destination}.partial`;
  run('curl.exe', ['--location', '--fail', '--silent', '--show-error', '--retry', '3',
    '--retry-all-errors', '--connect-timeout', '20', '--output', temporary, url]);
  renameSync(temporary, destination);
}

function matchesPinnedModel(file) {
  if (statSync(file, { throwIfNoEntry: false })?.size !== modelSize) return false;
  // A local file with the upstream LFS digest lets provisioning skip the 486 MB download.
  const digest = createHash('sha256');
  const stream = openSync(file, 'r');
  const buffer = Buffer.alloc(1024 * 1024);
  try {
    let length;
    while ((length = readSync(stream, buffer)) !== 0) digest.update(buffer.subarray(0, length));
  } finally {
    closeSync(stream);
  }
  return digest.digest('hex') === modelSha256;
}

function prepareModel(output, existingModelDir) {
  const downloads = path.join(cacheRoot, modelRevision);
  mkdirSync(downloads, { recursive: true });
  for (const name of modelFiles.filter(name => name !== modelBin)) {
    fetchFile(`${modelSource}/${name}`, path.join(downloads, name));
  }
  for (const [name, url] of [
    ['MODEL-CARD.md', `https://huggingface.co/Systran/faster-whisper-small/raw/${modelRevision}/README.md`],
    ['LICENSE-MIT.txt', 'https://raw.githubusercontent.com/openai/whisper/v20250625/LICENSE'],
  ]) fetchFile(url, path.join(downloads, name));

  const weights = path.join(downloads, modelBin);
  if (existingModelDir) {
    const localWeights = path.resolve(existingModelDir, modelBin);
    if (!matchesPinnedModel(localWeights)) {
      throw new Error(`The local weights at ${localWeights} do not match Systran/faster-whisper-small@${modelRevision}.`);
    }
    copyFileSync(localWeights, path.join(output, modelBin));
  } else {
    fetchFile(`${modelSource}/${modelBin}`, weights);
    if (!matchesPinnedModel(weights)) {
      rmSync(weights);
      throw new Error('The Whisper weight download is incomplete or mismatched. Try provisioning again.');
    }
    copyFileSync(weights, path.join(output, modelBin));
  }
  for (const name of [...modelFiles.filter(name => name !== modelBin), ...modelNotices]) {
    copyFileSync(path.join(downloads, name), path.join(output, name));
  }
}

function copyVisualCppRuntime(output) {
  const vswhere = path.join(process.env['ProgramFiles(x86)'], 'Microsoft Visual Studio', 'Installer', 'vswhere.exe');
  const lookup = spawnSync(vswhere, ['-latest', '-products', '*', '-requires',
    'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], { encoding: 'utf8', windowsHide: true });
  if (lookup.error || lookup.status !== 0 || !lookup.stdout.trim()) {
    throw new Error('Install Visual Studio C++ Build Tools with the x64 redistributable files before preparing voice.');
  }
  const redist = path.join(lookup.stdout.trim(), 'VC', 'Redist', 'MSVC');
  const version = readdirSync(redist, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && /^\d+\.\d+\.\d+$/.test(entry.name))
    .map(entry => entry.name).sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))[0];
  if (!version) throw new Error(`No MSVC redistributable under ${redist}.`);
  const architecture = path.join(redist, version, 'x64');
  const crt = readdirSync(architecture, { withFileTypes: true })
    .find(entry => entry.isDirectory() && /^Microsoft\.VC\d+\.CRT$/i.test(entry.name));
  if (!crt) throw new Error(`No x64 MSVC CRT under ${architecture}.`);
  const runtime = path.join(architecture, crt.name);
  for (const dll of msvcFiles) {
    copyFileSync(path.join(runtime, dll), path.join(output, dll));
  }
  return version;
}

function preparePython(output) {
  const archive = path.join(cacheRoot, pythonArchive);
  fetchFile(`https://www.python.org/ftp/python/${pythonVersion}/${pythonArchive}`, archive);
  run('tar.exe', ['-xf', archive, '-C', output]);
  // The embedded interpreter has its own import path and ignores installed Pythons.
  writeFileSync(path.join(output, 'python313._pth'), 'python313.zip\r\n.\r\npackages\r\n');
  const msvcVersion = copyVisualCppRuntime(output);
  const wheelhouse = path.join(cacheRoot, 'wheels');
  mkdirSync(wheelhouse, { recursive: true });
  const pinnedWheels = ['--disable-pip-version-check', '--no-deps', ...pipTarget, '-r', requirementsFile];
  const cachedWheels = readdirSync(wheelhouse).filter(name => name.endsWith('.whl'));
  if (!lockedPackages().every(name => cachedWheels.some(wheel => wheel.startsWith(`${name}-`)))) {
    run('py.exe', ['-3', '-m', 'pip', 'download', ...pinnedWheels, '--dest', wheelhouse,
      '--index-url', 'https://pypi.org/simple']);
  }
  const packages = path.join(output, 'packages');
  run('py.exe', ['-3', '-m', 'pip', 'install', ...pinnedWheels, '--no-index', '--find-links',
    wheelhouse, '--no-compile', '--target', packages]);
  // The wheel entry points are for the build Python. The installed app invokes its own Python directly.
  for (const name of ['bin', 'Scripts']) rmSync(path.join(packages, name), { recursive: true, force: true });
  return msvcVersion;
}

function isolatedEnvironment() {
  const ignored = ['path', 'pythonhome', 'pythonpath', 'virtual_env', 'conda_prefix', 'hf_hub_offline'];
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !ignored.includes(name.toLowerCase())));
  env.PATH = path.join(process.env.SystemRoot, 'System32');
  env.HF_HUB_OFFLINE = '1';
  return env;
}

function prepare(existingModelDir) {
  if (desktopTarget().platform !== 'win32') throw new Error('Prepare the bundled voice runtime on Windows x64.');
  mkdirSync(cacheRoot, { recursive: true });
  const staged = mkdtempSync(path.join(cacheRoot, 'prepare-'));
  const python = path.join(staged, 'python');
  const model = path.join(staged, 'model');
  mkdirSync(python);
  mkdirSync(model);
  try {
    const msvcVersion = preparePython(python);
    prepareModel(model, existingModelDir);
    for (const [name, url] of Object.entries(nativeLicenses)) {
      const license = path.join(cacheRoot, name);
      fetchFile(url, license);
      copyFileSync(license, path.join(staged, name));
    }
    writeFileSync(path.join(staged, 'THIRD-PARTY.md'), [
      '# Local speech recognition',
      '',
      `- [CPython ${pythonVersion}](https://www.python.org/downloads/release/python-31317/) Windows x64 embeddable distribution. License: python/LICENSE.txt.`,
      `- [Microsoft Visual C++ CRT ${msvcVersion}](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files) from the Visual Studio x64 application-local redistributable files.`,
      '- Native Python wheels from PyPI. Package metadata and included licenses are under python/packages/*.dist-info.',
      '- [PyAV 18.1.1 source](https://github.com/PyAV-Org/PyAV/tree/v18.1.1) and its [wheel FFmpeg configuration](https://github.com/PyAV-Org/PyAV/blob/v18.1.1/scripts/ffmpeg-latest.json). The Windows wheel includes GPL-enabled FFmpeg 8.1.3. [FFmpeg source](https://github.com/FFmpeg/FFmpeg/tree/n8.1.3) and [PyAV FFmpeg build recipe and dependency sources](https://github.com/PyAV-Org/pyav-ffmpeg/tree/8.1.3-1). License: FFmpeg-COPYING.GPLv2.',
      `- [Systran/faster-whisper-small](https://huggingface.co/Systran/faster-whisper-small/tree/${modelRevision}), converted from OpenAI Whisper. Model card and MIT license: model/.`,
      '- Upstream notices for CTranslate2, FlatBuffers, and Tokenizers are included beside this file. ONNX Runtime includes LICENSE and ThirdPartyNotices.txt in its package folder.',
      '',
    ].join('\n'));
    assertVoiceRuntime(staged, { allowBundledModel: true });
    run(path.join(python, 'python.exe'), ['-B', '-c',
      'import sys; from faster_whisper import WhisperModel; assert WhisperModel(sys.argv[1], device="cpu", compute_type="int8", local_files_only=True).model.is_multilingual',
      model], { cwd: staged, env: isolatedEnvironment() });
    rmSync(prepared, { recursive: true, force: true });
    renameSync(staged, prepared);
    console.log(`Offline voice runtime ready: ${prepared}`);
  } finally {
    rmSync(staged, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const { values } = parseArgs({ options: { prepare: { type: 'boolean' }, 'model-dir': { type: 'string' } } });
  if (!values.prepare) throw new Error('Usage: node scripts/desktop/build-voice-runtime.mjs --prepare [--model-dir <existing model directory>]');
  prepare(values['model-dir']);
}

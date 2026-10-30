import { createWriteStream } from 'node:fs';
import { access, chmod, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { spawn } from 'node:child_process';
import { parseArgs } from 'node:util';
import { localEmbeddingIdentity, openLocalEmbeddingProvider, LOCAL_EMBEDDING_MODEL } from '../packages/core/src/local-embedding-provider.ts';
import { readLocalEmbeddingConfig } from '../packages/core/src/build-retrieval-config.ts';
import { validEmbeddingVector } from '../packages/core/src/embedding-provider.ts';

// The server owns this installation. Requests never download model files.
const revision = '2c4055b12046f11709e9df2c122e59ffbdc2f900';
const runtimeVersion = '3.8.1';
const files = ['config.json', 'tokenizer.json', 'tokenizer_config.json', 'special_tokens_map.json', 'unigram.json', 'onnx/model_quantized.onnx'];
const { values } = parseArgs({ options: { config: { type: 'string' }, check: { type: 'boolean', default: false } } });
const configFile = path.resolve(values.config || process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG || '/opt/understand-book/models/local-embedding.json');
const exists = async file => access(file).then(() => true, () => false);

async function verify(config) {
  const identity = localEmbeddingIdentity(config);
  const session = await openLocalEmbeddingProvider(config, identity);
  try {
    const { vector } = await session.provider.embed_query('用自己的话解释，才能发现理解中的缺口。', {});
    if (!validEmbeddingVector(vector, 384)) throw new Error('MiniLM did not return a usable 384-dimensional vector');
    return { model: identity.model_id, revision: identity.model_revision, dimensions: vector.length };
  } finally { await session.dispose(); }
}

async function install() {
  if (await exists(configFile)) {
    const result = await verify(readLocalEmbeddingConfig(configFile));
    return { status: 'ready', reused: true, config: configFile, ...result };
  }
  if (values.check) throw new Error(`Semantic retrieval is not installed: ${configFile}. Run scripts/linux/build-multi-reader.sh before publishing.`);
  const root = path.dirname(configFile);
  const runtime_dir = path.join(root, `transformers-${runtimeVersion}`);
  const model_dir = path.join(root, `minilm-${revision}`);
  await mkdir(runtime_dir, { recursive: true });
  const installedVersion = await readFile(path.join(runtime_dir, 'node_modules/@huggingface/transformers/package.json'), 'utf8')
    .then(text => JSON.parse(text).version, () => undefined);
  if (installedVersion !== runtimeVersion || !await exists(path.join(runtime_dir, 'package-lock.json'))) {
    await writeFile(path.join(runtime_dir, 'package.json'), JSON.stringify({ private: true, dependencies: { '@huggingface/transformers': runtimeVersion } }, null, 2) + '\n');
    // npm-cli is beside the configured Node executable on the supported Linux host.
    const npmCli = path.resolve(path.dirname(process.execPath), '../lib/node_modules/npm/bin/npm-cli.js');
    await new Promise((resolve, reject) => {
      const child = process.platform === 'win32'
        ? spawn(process.env.ComSpec || 'cmd.exe', ['/d', '/s', '/c', 'npm install --omit=dev --no-audit --no-fund'], { cwd: runtime_dir, stdio: ['ignore', 'inherit', 'inherit'], windowsHide: true })
        : spawn(process.execPath, [npmCli, 'install', '--omit=dev', '--no-audit', '--no-fund'], { cwd: runtime_dir, env: { ...process.env, ONNXRUNTIME_NODE_INSTALL_CUDA: 'skip' }, stdio: ['ignore', 'inherit', 'inherit'] });
      child.on('error', reject);
      child.on('exit', code => code === 0 ? resolve() : reject(new Error(`Embedding runtime installation failed (${code})`)));
    });
  }
  for (const name of files) {
    const file = path.join(model_dir, name);
    if (await exists(file)) continue;
    await mkdir(path.dirname(file), { recursive: true });
    const pending = `${file}.download`;
    try {
      process.stderr.write(`Installing MiniLM: ${name}\n`);
      const response = await fetch(`https://huggingface.co/${LOCAL_EMBEDDING_MODEL}/resolve/${revision}/${name}`, { signal: AbortSignal.timeout(300_000) });
      if (!response.ok || !response.body) throw new Error(`Model download failed (${response.status}): ${name}`);
      await pipeline(Readable.fromWeb(response.body), createWriteStream(pending));
      await rename(pending, file);
    } finally { await rm(pending, { force: true }); }
  }
  await writeFile(path.join(model_dir, 'model-metadata.json'), JSON.stringify({ model: LOCAL_EMBEDDING_MODEL, revision }) + '\n');
  const result = await verify({ runtime_dir, model_dir });
  const pending = `${configFile}.tmp`;
  await writeFile(pending, JSON.stringify({ version: 'local_embedding_config.v1', runtime_dir: path.relative(root, runtime_dir), model_dir: path.relative(root, model_dir) }, null, 2) + '\n');
  await chmod(pending, 0o644);
  await rename(pending, configFile);
  return { status: 'ready', reused: false, config: configFile, ...result };
}

try { console.log(JSON.stringify(await install())); }
catch (error) { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; }

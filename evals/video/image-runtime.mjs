import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { cases, output, question, root, mcpResult, save } from './image-experiment.mjs';

const [mode, ...selected] = process.argv.slice(2);
if (!['record', 'live'].includes(mode)) throw new Error('usage: image-runtime.mjs record|live [CASE ...]');
if (mode === 'live' && ['OPENCODE_API_KEY', 'OPENCODE_BASE_URL', 'FLUID_LLM_MODEL'].some(key => !process.env[key])) {
  throw new Error('Live mode requires OPENCODE_API_KEY, OPENCODE_BASE_URL and FLUID_LLM_MODEL');
}
const manifest = JSON.parse(readFileSync(join(output, 'inputs.json'), 'utf8'));
let server;
let currentDir;
const env = { ...process.env };
if (mode === 'record') {
  server = createServer(async (req, res) => {
    let body = '';
    for await (const chunk of req) body += chunk;
    writeFileSync(join(currentDir, 'http-body.json'), body);
    save(join(currentDir, 'http-metadata.json'), { method: req.method, url: req.url, body_bytes: Buffer.byteLength(body), content_length: req.headers['content-length'] });
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ choices: [{ message: { content: 'RECORDING ONLY: no model inference.' } }] }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  env.OPENCODE_BASE_URL = `http://127.0.0.1:${server.address().port}`;
  env.OPENCODE_API_KEY = 'vi0c-local-recorder';
  env.FLUID_LLM_MODEL = 'vi0c-projection-recorder';
  env.UNDERSTAND_BOOK_PROVIDER = 'native';
}
try {
  for (const id of selected.length ? selected : Object.keys(cases)) {
    const result = mcpResult(manifest, id);
    currentDir = join(output, `runtime-${mode}`, id);
    const plan = join(currentDir, 'input.json');
    save(plan, { question, images: result.content.filter(c => c.type === 'image').map(c => c.data) });
    const child = spawn('cargo', ['run', '--quiet', '-p', 'runtime', '--example', 'video_image_probe', '--', plan, currentDir],
      { cwd: root, env, windowsHide: true, stdio: ['ignore', 'inherit', 'inherit'] });
    const exit = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
    save(join(currentDir, 'host.json'), { mode, model: env.FLUID_LLM_MODEL, provider_mode: env.UNDERSTAND_BOOK_PROVIDER || 'native', exit_code: exit, semantic_inference: mode === 'live' });
    console.log(JSON.stringify({ case: id, mode, exit_code: exit }));
    if (exit !== 0) { process.exitCode = 1; break; }
  }
} finally { server?.close(); }

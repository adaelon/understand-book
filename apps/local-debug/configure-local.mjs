// Explicitly provision this machine's debug profile from the repository configuration.
// Packaging never calls this script and never copies the resulting private .env.
import { parseEnv } from 'node:util';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('../../', import.meta.url));
const source = parseEnv(readFileSync(path.join(root, '.env'), 'utf8'));
const directory = path.resolve(process.argv[2] ?? path.join(root, 'dist/UnderstandBookLocalDebug/data'));
const destination = path.join(directory, '.env');
const configured = existsSync(destination) ? parseEnv(readFileSync(destination, 'utf8')) : {};
const names = [
  'OPENCODE_API_KEY', 'OPENCODE_BASE_URL', 'FLUID_LLM_MODEL',
  'UNDERSTAND_BOOK_PROVIDER', 'UNDERSTAND_BOOK_AGENT_REASONING_EFFORT',
  'UNDERSTAND_BOOK_VOICE_PYTHON', 'UNDERSTAND_BOOK_VOICE_MODEL_DIR',
  'VIDEO_WHISPER_PYTHON', 'VIDEO_WHISPER_MODEL_DIR',
  'FFMPEG', 'FFPROBE',
  'UNDERSTAND_BOOK_PLOT_PYTHON', 'UNDERSTAND_BOOK_ANIMATION_PYTHON',
  'UNDERSTAND_BOOK_PREVIEW_BROWSER',
];
for (const name of names) {
  const value = process.env[name] || source[name];
  if (value?.trim()) configured[name] = value;
}
configured.UB_OBSERVABILITY_MODE = 'off';
mkdirSync(directory, { recursive: true });
writeFileSync(destination, '# Machine-local debug configuration. Do not distribute.\n'
  + Object.entries(configured).map(([name, value]) => `${name}=${JSON.stringify(value)}`).join('\n') + '\n');
console.log(`Configured ${names.filter(name => configured[name]?.trim()).join(', ')} in ${destination}. Restart the local application to apply.`);

import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { createRequire } from 'node:module';

const root = fileURLToPath(new URL('../../', import.meta.url));
const require = createRequire(path.join(root, 'packages/web/package.json'));
const output = path.join(root, 'dist/UnderstandBookLocalDebug');
const temp = path.join(root, 'target/local-debug-temp');
mkdirSync(temp, { recursive: true });
mkdirSync(output, { recursive: true });
const data = path.join(output, 'data');
mkdirSync(data, { recursive: true });
if (!existsSync(path.join(data, 'settings.json'))) {
  writeFileSync(path.join(data, 'settings.json'), JSON.stringify({
    schema: 'understand_book.desktop_settings.v1',
    library_root: path.join(root, '.understand-book'),
  }, null, 2));
}
const env = { ...process.env, TEMP: temp, TMP: temp, VITE_MULTI_USER: '0', VITE_LOCAL_DEBUG: '1' };
function run(program, args, cwd = root) {
  const result = spawnSync(program, args, { cwd, env, stdio: 'inherit', windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run(process.execPath, [require.resolve('vue-tsc/bin/vue-tsc.js'), '--noEmit', '-p', 'tsconfig.local.json'], path.join(root, 'packages/web'));
run(process.execPath, [path.join(path.dirname(require.resolve('vite/package.json')), 'bin/vite.js'), 'build', '--outDir', 'dist-local-debug', '--sourcemap'], path.join(root, 'packages/web'));
run('cargo', ['build', '-p', 'understand-book-local-debug', '--bin', 'UnderstandBookLocalDebug']);
cpSync(path.join(root, 'target/debug/UnderstandBookLocalDebug.exe'), path.join(output, 'UnderstandBookLocalDebug.exe'));
const symbols = path.join(root, 'target/debug/UnderstandBookLocalDebug.pdb');
if (existsSync(symbols)) cpSync(symbols, path.join(output, 'UnderstandBookLocalDebug.pdb'));
cpSync(path.join(root, 'packages/web/dist-local-debug'), path.join(output, 'resources/web-dist'), { recursive: true });

// Reuse the production build engine packager; never ship the obsolete cached engine.
env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR = path.join(temp, 'engine');
run(process.execPath, ['scripts/desktop/build-sidecar.mjs']);
cpSync(path.join(temp, 'engine/understand-book-build-x86_64-pc-windows-msvc.exe'), path.join(output, 'understand-book-build.exe'));
cpSync(path.join(temp, 'engine/resources'), path.join(output, 'resources'), { recursive: true });
cpSync(path.join(temp, 'engine/node_modules'), path.join(output, 'node_modules'), { recursive: true });
cpSync(path.join(root, 'apps/local-debug/README.md'), path.join(output, 'README.md'));
cpSync(path.join(root, '.env.example'), path.join(output, 'model.env.example'));
writeFileSync(path.join(output, 'Start.cmd'), '@echo off\r\ncd /d "%~dp0"\r\nset "UNDERSTAND_BOOK_DEBUG_DATA_DIR=%~dp0data"\r\nif not exist "data" mkdir "data"\r\n"%~dp0UnderstandBookLocalDebug.exe" %* >> "data\\debug.log" 2>&1\r\nif errorlevel 1 (\r\n echo Local Debug failed. See data\\debug.log\r\n pause\r\n)\r\n');
console.log(`Local debug application: ${output}`);

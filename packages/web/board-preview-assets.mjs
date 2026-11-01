import { build } from 'vite';
import vue from '@vitejs/plugin-vue';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile, writeFile } from 'node:fs/promises';

const root = path.dirname(fileURLToPath(import.meta.url));

// The backend's isolated browser consumes the same compiled BoardCanvas as Reader.
// Keep the bundle alongside Reader resources so desktop resource copying and the
// network deployment carry it without a Node runtime on the serving machine.
export function boardPreviewAssets() {
  let output;
  let building = Promise.resolve();
  const render = () => {
    building = building.catch(() => {}).then(() => build({
      configFile: false, root, publicDir: false, logLevel: 'warn', plugins: [vue()],
      define: { 'process.env.NODE_ENV': JSON.stringify('production') },
      build: {
        outDir: output, emptyOutDir: true, cssCodeSplit: false,
        assetsInlineLimit: Number.MAX_SAFE_INTEGER,
        lib: { entry: path.join(root, 'src/board-preview.ts'), name: 'BoardPreview', formats: ['iife'], fileName: () => 'renderer.js', cssFileName: 'renderer' },
        rollupOptions: { output: { inlineDynamicImports: true } },
      },
    })).then(async () => {
      // Keep the math faces used by BoardCanvas; they are inlined above so the
      // isolated browser renders the same equations without external requests.
      // Semantic topics measure and render in the same Noto Sans face as Reader.
      const stylesheet = path.join(output, 'renderer.css');
      await writeFile(stylesheet, (await readFile(stylesheet, 'utf8')).replace(/@font-face\s*\{[^}]*\}/g,
        face => /font-family:\s*["']?(?:KaTeX_|Noto Sans SC)/i.test(face) ? face : ''));
    });
    return building;
  };
  return {
    name: 'board-preview-assets',
    apply(_config, environment) { return environment.mode !== 'test'; },
    configResolved(config) { output = path.resolve(config.root, config.build.outDir, 'board-preview'); },
    // The parent build empties its output before writing, so emit this after it.
    async closeBundle() { await render(); },
    async configureServer(server) {
      await render();
      server.watcher.on('change', file => {
        if (/[/\\](BoardCanvas\.vue|BoardDrawingText\.vue|MindMapLayer\.vue|mind-map-[^/\\]*|board-(?:canvas|math)[^/\\]*|board-preview\.ts|board-workspace\.ts|style\.css|brand\.css)$/.test(file)
          || file.includes(`${path.sep}i18n${path.sep}src${path.sep}`)) {
          void render().catch(error => server.config.logger.error(String(error)));
        }
      });
    },
  };
}

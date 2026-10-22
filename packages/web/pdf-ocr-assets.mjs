import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
const require = createRequire(import.meta.url);

/** Both the web Reader and the offline desktop Reader carry their OCR language files. */
export function pdfOcrAssets() {
  const files = ['eng', 'chi_sim'].map(code => ({
    fileName: `pdf-ocr/${code}.traineddata.gz`,
    path: require.resolve(`@tesseract.js-data/${code}/4.0.0_best_int/${code}.traineddata.gz`),
  }));
  return {
    name: 'pdf-ocr-assets',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const file = files.find(file => request.url?.split('?')[0] === `/${file.fileName}`);
        if (!file) return next();
        response.setHeader('Content-Type', 'application/octet-stream');
        response.end(readFileSync(file.path));
      });
    },
    generateBundle() {
      for (const file of files) this.emitFile({ type: 'asset', fileName: file.fileName, source: readFileSync(file.path) });
    },
  };
}

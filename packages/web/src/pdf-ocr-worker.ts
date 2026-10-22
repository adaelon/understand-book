import { createWorker, PSM } from 'tesseract.js';
import workerUrl from 'tesseract.js/dist/worker.min.js?url';
import coreUrl from 'tesseract.js-core/tesseract-core-lstm.wasm.js?url';

export async function createPdfOcrWorker() {
  const worker = await createWorker('eng+chi_sim', 1, {
    workerPath: new URL(workerUrl, location.href).href,
    corePath: new URL(coreUrl, location.href).href,
    langPath: new URL(`${import.meta.env.BASE_URL}pdf-ocr/`, location.href).href,
    workerBlobURL: false, cacheMethod: 'none', errorHandler: () => {},
  });
  await worker.setParameters({ tessedit_pageseg_mode: PSM.AUTO });
  return worker;
}

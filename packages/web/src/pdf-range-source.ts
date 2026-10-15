import { PDFDataRangeTransport, type getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';

// 256 KiB reduces dependency round trips without the overfetch of MiB-sized chunks.
const CHUNK_SIZE = 256 * 1024;

// Start with a bounded request: PDF.js's URL loader otherwise starts a full GET
// before discovering range support, filling a slow connection even when aborted.
export async function pdfRangeSource(
  url: string,
  controller: AbortController,
  onError: (error: Error) => void,
): Promise<NonNullable<Parameters<typeof getDocument>[0]>> {
  const read = (begin: number, end: number) => fetch(url, {
    headers: { Range: `bytes=${begin}-${end - 1}` },
    credentials: 'same-origin', signal: controller.signal,
  });
  const first = await read(0, CHUNK_SIZE);
  if (!first.ok) throw new Error(`PDF 读取失败（${first.status}）`);
  const initialData = new Uint8Array(await first.arrayBuffer());
  // The local Reader's endpoint returns the complete PDF when Range is unsupported.
  if (first.status === 200) return { data: initialData };
  const length = Number(first.headers.get('Content-Range')?.match(/\/(\d+)$/)?.[1]);
  if (first.status !== 206 || !Number.isSafeInteger(length) || length <= 0) {
    throw new Error('PDF 分段响应缺少文件长度');
  }
  const range = new PDFDataRangeTransport(length, initialData, true);
  range.abort = () => controller.abort();
  range.requestDataRange = (begin, end) => {
    void (async () => {
      const response = await read(begin, end);
      if (response.status !== 206) throw new Error(`PDF 分段读取失败（${response.status}）`);
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (!controller.signal.aborted) range.onDataRange(begin, bytes);
    })().catch(error => {
      if (!controller.signal.aborted) {
        controller.abort();
        onError(error instanceof Error ? error : new Error(String(error)));
      }
    });
  };
  return { range, disableStream: true, disableAutoFetch: true, rangeChunkSize: CHUNK_SIZE };
}

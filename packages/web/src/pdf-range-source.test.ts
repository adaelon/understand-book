import { afterEach, expect, it, vi } from 'vitest';
import { pdfRangeSource } from './pdf-range-source';

afterEach(() => vi.unstubAllGlobals());

it('starts with a bounded range and loads only subsequently requested bytes', async () => {
  const first = new Uint8Array(262144); first.set([37, 80, 68, 70]);
  const fetcher = vi.fn()
    .mockResolvedValueOnce(new Response(first, { status: 206, headers: { 'Content-Range': 'bytes 0-262143/500000' } }))
    .mockResolvedValueOnce(new Response(new Uint8Array([4, 5]), { status: 206 }));
  vi.stubGlobal('fetch', fetcher);
  const controller = new AbortController();
  const source = await pdfRangeSource('/paper.pdf', controller, vi.fn());
  expect(fetcher).toHaveBeenCalledTimes(1);
  expect(fetcher.mock.calls[0][1].headers.Range).toBe('bytes=0-262143');
  expect(source.rangeChunkSize).toBe(262144);
  expect(source.range?.length).toBe(500000);
  expect(source.range?.initialData).toEqual(first);
  expect(source.disableAutoFetch).toBe(true);
  const delivered = vi.spyOn(source.range!, 'onDataRange');
  source.range!.requestDataRange(499998, 500000);
  await vi.waitFor(() => expect(delivered).toHaveBeenCalledWith(499998, new Uint8Array([4, 5])));
  expect(fetcher.mock.calls[1][1].headers.Range).toBe('bytes=499998-499999');
  source.range!.abort();
  expect(controller.signal.aborted).toBe(true);
});

it('uses the complete response from local endpoints without range support', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(new Uint8Array([37, 80, 68, 70]))));
  expect(await pdfRangeSource('/api/book/pdf/original', new AbortController(), vi.fn()))
    .toEqual({ data: new Uint8Array([37, 80, 68, 70]) });
});

it('completes PDF.js coalesced image ranges using bounded native reads', async () => {
  const file = new Uint8Array(3 * 1024 * 1024 + 666);
  for (let i = 0; i < file.length; i++) file[i] = i % 251;
  const requested: number[] = [];
  vi.stubGlobal('fetch', vi.fn(async (_url: string, init: RequestInit) => {
    const [begin, last] = (init.headers as Record<string, string>).Range.slice(6).split('-').map(Number);
    requested.push(last - begin + 1);
    const end = Math.min(last + 1, begin + 1024 * 1024, file.length);
    return new Response(file.slice(begin, end), { status: 206, headers: { 'Content-Range': `bytes ${begin}-${end - 1}/${file.length}` } });
  }));
  const source = await pdfRangeSource('/native-image.pdf', new AbortController(), vi.fn());
  const delivered = vi.spyOn(source.range!, 'onDataRange');
  source.range!.requestDataRange(262144, 2883584);
  await vi.waitFor(() => expect(delivered).toHaveBeenCalledOnce());
  const [begin, bytes] = delivered.mock.calls[0];
  expect(begin).toBe(262144);
  expect(bytes?.length).toBe(2883584 - 262144);
  expect(bytes?.every((byte, index) => byte === file[begin + index])).toBe(true);
  expect(Math.max(...requested)).toBeLessThanOrEqual(262144);
});

it('reports later authorization failure and cancels pending reads', async () => {
  vi.stubGlobal('fetch', vi.fn()
    .mockResolvedValueOnce(new Response(new Uint8Array(262144), { status: 206, headers: { 'Content-Range': 'bytes 0-262143/500000' } }))
    .mockResolvedValueOnce(new Response('', { status: 403 })));
  const failed = vi.fn(); const controller = new AbortController();
  const source = await pdfRangeSource('/paper.pdf', controller, failed);
  source.range!.requestDataRange(262144, 500000);
  await vi.waitFor(() => expect(failed).toHaveBeenCalledWith(expect.objectContaining({ message: 'PDF 分段读取失败（403）' })));
  expect(controller.signal.aborted).toBe(true);
});

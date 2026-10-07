import { afterEach, expect, it, vi } from 'vitest';
import { pdfRangeSource } from './pdf-range-source';

afterEach(() => vi.unstubAllGlobals());

it('starts with a bounded range and loads only subsequently requested bytes', async () => {
  const first = new Uint8Array(65536); first.set([37, 80, 68, 70]);
  const fetcher = vi.fn()
    .mockResolvedValueOnce(new Response(first, { status: 206, headers: { 'Content-Range': 'bytes 0-65535/200000' } }))
    .mockResolvedValueOnce(new Response(new Uint8Array([4, 5]), { status: 206 }));
  vi.stubGlobal('fetch', fetcher);
  const controller = new AbortController();
  const source = await pdfRangeSource('/paper.pdf', controller, vi.fn());
  expect(fetcher).toHaveBeenCalledTimes(1);
  expect(fetcher.mock.calls[0][1].headers.Range).toBe('bytes=0-65535');
  expect(source.range?.length).toBe(200000);
  expect(source.range?.initialData).toEqual(first);
  expect(source.disableAutoFetch).toBe(true);
  const delivered = vi.spyOn(source.range!, 'onDataRange');
  source.range!.requestDataRange(199998, 200000);
  await vi.waitFor(() => expect(delivered).toHaveBeenCalledWith(199998, new Uint8Array([4, 5])));
  expect(fetcher.mock.calls[1][1].headers.Range).toBe('bytes=199998-199999');
  source.range!.abort();
  expect(controller.signal.aborted).toBe(true);
});

it('uses the complete response from local endpoints without range support', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(new Uint8Array([37, 80, 68, 70]))));
  expect(await pdfRangeSource('/api/book/pdf/original', new AbortController(), vi.fn()))
    .toEqual({ data: new Uint8Array([37, 80, 68, 70]) });
});

it('reports later authorization failure and cancels pending reads', async () => {
  vi.stubGlobal('fetch', vi.fn()
    .mockResolvedValueOnce(new Response(new Uint8Array(65536), { status: 206, headers: { 'Content-Range': 'bytes 0-65535/200000' } }))
    .mockResolvedValueOnce(new Response('', { status: 403 })));
  const failed = vi.fn(); const controller = new AbortController();
  const source = await pdfRangeSource('/paper.pdf', controller, failed);
  source.range!.requestDataRange(65536, 131072);
  await vi.waitFor(() => expect(failed).toHaveBeenCalledWith(expect.objectContaining({ message: 'PDF 分段读取失败（403）' })));
  expect(controller.signal.aborted).toBe(true);
});

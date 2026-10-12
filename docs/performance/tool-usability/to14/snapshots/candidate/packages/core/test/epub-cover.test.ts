import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { strToU8, zipSync } from 'fflate';
import { describe, expect, it } from 'vitest';
import { extractEpubCover } from '../src/epub-cover';
import { buildAssetManifest } from '../src/asset-manifest';
import { AssetManifestZ } from '../src/zod';

function files(declaration: string) {
  return {
    'META-INF/container.xml': strToU8('<container><rootfiles><rootfile full-path="OPS/book.opf"/></rootfiles></container>'),
    'OPS/book.opf': strToU8(`<package>${declaration}<spine/></package>`),
    'OPS/Images/front cover.png': new Uint8Array([137, 80, 78, 71]),
    'OPS/Text/cover.xhtml': strToU8('<html><body><svg><image xlink:href="../Images/front%20cover.png"/></svg></body></html>'),
  };
}
describe('automatic EPUB covers', () => {
  it.each([
    '<manifest><item id="art" href="Images/front%20cover.png" properties="cover-image" media-type="image/png"/></manifest>',
    '<metadata><meta content="art" name="cover"/></metadata><manifest><item id="art" href="Images/front%20cover.png"/></manifest>',
    '<guide><reference type="cover" href="Text/cover.xhtml"/></guide>',
  ])('extracts declared covers independently of the spine: %s', declaration => {
    expect(extractEpubCover(files(declaration))).toMatchObject({ extension: '.png', mime: 'image/png', bytes: new Uint8Array([137, 80, 78, 71]) });
  });
  it('does not promote an arbitrary illustration or a missing cover', () => {
    expect(extractEpubCover(files('<manifest><item href="Images/front%20cover.png"/></manifest>'))).toBeNull();
    expect(extractEpubCover(files('<manifest><item properties="cover-image" href="missing.png"/></manifest>'))).toBeNull();
  });
  it('saves the cover into the same resource manifest used by publication', () => {
    const dir = mkdtempSync(path.join(tmpdir(), 'ub-cover-'));
    const book = path.join(dir, 'book.epub');
    writeFileSync(book, zipSync(files('<manifest><item properties="cover-image" href="Images/front%20cover.png"/></manifest>')));
    const manifest = buildAssetManifest({ book_id: 'book', book_path: book, output_dir: path.join(dir, 'out'), source_blocks: [], lid_nodes: [] });
    expect(AssetManifestZ.parse(manifest).cover).toEqual({ stored_path: 'assets/cover.png', mime: 'image/png' });
    expect(readFileSync(path.join(dir, 'out', manifest.cover!.stored_path))).toEqual(Buffer.from([137, 80, 78, 71]));
    expect(manifest.images).toEqual([]);
  });
});

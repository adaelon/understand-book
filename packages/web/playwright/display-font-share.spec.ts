import { expect, test } from '@playwright/test';

test('share titles load and embed Smiley Sans while body text keeps its reading font', async ({ page }) => {
  test.setTimeout(60_000);
  await page.route('**/display-font-share', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html>
    <html><head><meta charset="utf-8"></head><body><script type="module">
    import '/src/style.css';
    window.shareLayout = await import('/src/reading-share-layout.ts');
    </script></body></html>` }));
  await page.goto('/display-font-share');
  await page.waitForFunction(() => !!(window as any).shareLayout);
  const result = await page.evaluate(async () => {
    const layout = (window as any).shareLayout;
    const draft = {
      source: { parts: [{ id: 'body', label: '我的笔记', text: '先有自己的想法，再对照原文验证。' }], sources: ['阅读与理解'], association: '' },
      selected: ['body'], title: '阅读随记 Reading 2026', reflection: '', layout: 'understanding', palette: 'paper',
    };
    const pages = await layout.paginateShare(draft);
    const heading = pages.pages[0].querySelector('.caption h1');
    const paragraph = pages.pages[0].querySelector('.share-section p');
    const titleStyle = getComputedStyle(heading);
    const bodyFont = getComputedStyle(paragraph).fontFamily;
    const titleFont = titleStyle.fontFamily, titleWeight = titleStyle.fontWeight;
    const faceLoaded = Array.from(document.fonts).some(face => face.family.replace(/["']/g, '') === 'Smiley Sans' && face.status === 'loaded');
    const titleBox = heading.getBoundingClientRect(), sheetBox = pages.pages[0].getBoundingClientRect();
    const titleRect = { x: titleBox.x - sheetBox.x, y: titleBox.y - sheetBox.y, width: titleBox.width, height: titleBox.height };
    pages.dispose();

    // Inspect the self-contained SVG sent to rasterization; missing font data
    // would otherwise silently fall back when the exported image is decoded.
    const svgBlobs: Blob[] = [];
    const original = FileReader.prototype.readAsDataURL;
    FileReader.prototype.readAsDataURL = function(blob) {
      if (blob.type === 'image/svg+xml') svgBlobs.push(blob);
      return original.call(this, blob);
    };
    let images: Blob[];
    try { images = await layout.renderShare(draft); }
    finally { FileReader.prototype.readAsDataURL = original; }
    const xml = new DOMParser().parseFromString(await svgBlobs[0].text(), 'image/svg+xml');
    const faceRules = [...(xml.querySelector('style')?.textContent ?? '').matchAll(/@font-face\s*\{[^}]*\}/g)].map(match => match[0]);
    const embedded = faceRules.find(rule => rule.includes('Smiley Sans')) ?? '';
    const image = new Image(), url = URL.createObjectURL(images[0]);
    try {
      image.src = url; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const ctx = canvas.getContext('2d')!; ctx.drawImage(image, 0, 0);
      const pixels = ctx.getImageData(Math.floor(titleRect.x), Math.floor(titleRect.y), Math.floor(titleRect.width), Math.floor(titleRect.height)).data;
      let titlePixels = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i] < 200 && pixels[i] > pixels[i + 1] * 1.5 && pixels[i + 1] < 130) titlePixels++;
      }
      return { titleFont, titleWeight, bodyFont, faceLoaded, embedded: /url\(["']?data:[^,]+;base64,/.test(embedded),
        width: image.width, height: image.height, titlePixels, count: images.length };
    } finally { URL.revokeObjectURL(url); }
  });
  expect(result.titleFont).toContain('Smiley Sans');
  expect(result.titleWeight).toBe('400');
  expect(result.bodyFont).toContain('Noto Serif SC Variable');
  expect(result.faceLoaded).toBe(true);
  expect(result.embedded).toBe(true);
  expect([result.width, result.height, result.count]).toEqual([1080, 1440, 1]);
  expect(result.titlePixels).toBeGreaterThan(500);
});

import { createCanvas } from '@napi-rs/canvas';

export interface FixturePage {
  lines?: Array<{ text: string; x: number; y: number; size?: number }>;
  scan?: { lines: string[]; box: [number, number, number, number]; jpeg?: Buffer };
}
/** Original test material, including an actual raster scan rather than an empty page. */
export function pdfFixture(pages: FixturePage[]): Uint8Array {
  const objects: Buffer[] = [];
  const add = (value: string | Buffer) => { objects.push(typeof value === 'string' ? Buffer.from(value, 'latin1') : value); return objects.length; };
  const stream = (dictionary: string, bytes: Buffer) => Buffer.concat([Buffer.from(`<< ${dictionary} /Length ${bytes.length} >>\nstream\n`), bytes, Buffer.from('\nendstream')]);
  add('<< /Type /Catalog /Pages 2 0 R >>'); add(''); add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  const ids: number[] = [];
  for (const page of pages) {
    const id = add(''); ids.push(id);
    let operations = (page.lines ?? []).map(line => `BT /F1 ${line.size ?? 14} Tf ${line.x} ${line.y} Td (${line.text.replace(/[\\()]/g, '\\$&')}) Tj ET\n`).join('');
    let resources = '/Font << /F1 3 0 R >>';
    if (page.scan) {
      const canvas = createCanvas(1200, 360), context = canvas.getContext('2d');
      context.fillStyle = 'white'; context.fillRect(0, 0, 1200, 360); context.fillStyle = 'black'; context.font = '46px Arial';
      page.scan.lines.forEach((text, index) => context.fillText(text, 30, 75 + index * 70));
      const imageId = add(stream('/Type /XObject /Subtype /Image /Width 1200 /Height 360 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode', page.scan.jpeg ?? canvas.toBuffer('image/jpeg')));
      const [x, y, right, top] = page.scan.box;
      operations += `q ${right - x} 0 0 ${top - y} ${x} ${y} cm /Scan Do Q\n`;
      resources += ` /XObject << /Scan ${imageId} 0 R >>`;
    }
    const contentId = add(stream('', Buffer.from(operations, 'latin1')));
    objects[id - 1] = Buffer.from(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << ${resources} >> /Contents ${contentId} 0 R >>`);
  }
  objects[1] = Buffer.from(`<< /Type /Pages /Kids [${ids.map(id => `${id} 0 R`).join(' ')}] /Count ${ids.length} >>`);
  const chunks = [Buffer.from('%PDF-1.4\n')], offsets = [0]; let length = chunks[0].length;
  objects.forEach((object, index) => { offsets.push(length); const bytes = Buffer.concat([Buffer.from(`${index + 1} 0 obj\n`), object, Buffer.from('\nendobj\n')]); chunks.push(bytes); length += bytes.length; });
  chunks.push(Buffer.from(`xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${length}\n%%EOF\n`));
  return new Uint8Array(Buffer.concat(chunks));
}

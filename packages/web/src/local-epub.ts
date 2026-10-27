import { t as uiText } from './i18n';
import { unzipSync, strFromU8 } from 'fflate';

export interface EpubChapter { title: string; html: string }
const readerStyle = 'body{font-family:Georgia,"Microsoft YaHei",serif;line-height:1.9;color:#302d27;margin:2rem auto;padding:0 2rem;max-width:800px}img,svg{max-width:100%;height:auto}table{border-collapse:collapse;max-width:100%}td,th{border:1px solid #ddd;padding:.4rem}';
export function readerDocument(html: string): string {
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src blob: data:; style-src 'unsafe-inline'; font-src data: blob:"><style>${readerStyle}</style></head><body>${html}</body></html>`;
}
/** EPUB contents stay inside a scriptless, networkless iframe. */
export function openLocalEpub(buffer: ArrayBuffer): { chapters: EpubChapter[]; dispose(): void } {
  const files = unzipSync(new Uint8Array(buffer)), parser = new DOMParser(), urls: string[] = [];
  const text = (path: string) => { if (!files[path]) throw new Error(uiText('EPUB 缺少 {0}', [path])); return strFromU8(files[path]); };
  const resolve = (base: string, relative: string) => decodeURIComponent(new URL(relative, `https://epub.local/${base}`).pathname.slice(1));
  try {
    const container = parser.parseFromString(text('META-INF/container.xml'), 'application/xml');
    const packagePath = container.querySelector('rootfile')?.getAttribute('full-path');
    if (!packagePath) throw new Error(uiText('EPUB 缺少阅读顺序。'));
    const document = parser.parseFromString(text(packagePath), 'application/xml');
    const manifest = new Map(Array.from(document.querySelectorAll('manifest > item')).map(item => [item.getAttribute('id'), {
      path: resolve(packagePath, item.getAttribute('href') ?? ''), type: item.getAttribute('media-type') ?? '',
    }]));
    const images = new Map<string, string>();
    for (const asset of manifest.values()) {
      if (!asset.type.startsWith('image/') || !files[asset.path]) continue;
      const url = URL.createObjectURL(new Blob([files[asset.path].slice().buffer], { type: asset.type }));
      images.set(asset.path, url); urls.push(url);
    }
    const chapters: EpubChapter[] = [];
    for (const entry of document.querySelectorAll('spine > itemref')) {
      const asset = manifest.get(entry.getAttribute('idref'));
      if (!asset) continue;
      const page = parser.parseFromString(text(asset.path), 'text/html');
      page.querySelectorAll('script,iframe,object,embed,form,base,link,meta').forEach(element => element.remove());
      for (const element of page.querySelectorAll('*')) {
        for (const attribute of Array.from(element.attributes)) if (attribute.name.startsWith('on')) element.removeAttribute(attribute.name);
      }
      for (const image of page.querySelectorAll('img,image')) {
        const source = image.getAttribute('src') ?? image.getAttribute('href') ?? image.getAttribute('xlink:href');
        if (!source) continue;
        const url = images.get(resolve(asset.path, source));
        image.removeAttribute('srcset');
        if (url) { if (image.tagName.toLowerCase() === 'img') image.setAttribute('src', url); else { image.setAttribute('href', url); image.setAttribute('xlink:href', url); } }
      }
      for (const link of page.querySelectorAll('a')) {
        if (!link.getAttribute('href')?.startsWith('#')) link.removeAttribute('href');
        link.removeAttribute('target');
      }
      chapters.push({ title: page.querySelector('h1,h2,h3,title')?.textContent?.trim() || uiText('第 {0} 节', [chapters.length + 1]), html: readerDocument(page.body.innerHTML) });
    }
    if (!chapters.length) throw new Error(uiText('EPUB 没有可显示的章节。'));
    return { chapters, dispose: () => urls.forEach(url => URL.revokeObjectURL(url)) };
  } catch (error) { urls.forEach(url => URL.revokeObjectURL(url)); throw error; }
}

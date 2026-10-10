import { renderMarkdown } from './md';
import katexCss from 'katex/dist/katex.min.css?inline';
import type { ShareDraft } from './reading-share';
import slowerArtwork from './assets/share-slower.svg?raw';

export function shareSize(draft: ShareDraft) {
  return draft.source.diagram && draft.orientation === 'landscape' ? { width: 1440, height: 1080 } : { width: 1080, height: 1440 };
}
const css = `
.share-sheet { --accent:#a64735; --bright:#c56e54; --stage:#d9c5ac; --paper:#fff3df; --ink:#4f4639; box-sizing:border-box; width:1080px; height:1440px; background:#fff9ef; color:var(--ink); font:400 32px/1.8 "Noto Serif SC Variable"; position:relative; }
.share-sheet.blue { --accent:#2e65ae; --bright:#4c95e8; --stage:#bbcda9; --paper:#f6f0d8; --ink:#344e3b; background:#fffdec; }
.share-sheet * { box-sizing:border-box; }
.share-sheet header { position:absolute; top:66px; left:88px; right:88px; text-align:center; color:var(--accent); }
.share-sheet .kind { font:500 23px/1.6 "Noto Sans SC Variable"; letter-spacing:3px; }
.share-sheet .handwritten { position:absolute; left:64px; top:130px; width:230px; height:145px; transform:rotate(-12deg); color:var(--bright); z-index:2; }
.share-sheet .handwritten svg { display:block; width:100%; height:100%; }
.share-sheet > .content { position:absolute; top:196px; left:170px; right:170px; display:flow-root; }
.share-sheet .hero { position:relative; padding:54px 62px; background:var(--stage); border-radius:56px; isolation:isolate; }
.share-sheet .light { position:absolute; inset:0; border-radius:inherit; overflow:hidden; z-index:-1; }
.share-sheet .light::before { content:''; position:absolute; inset:-20%; background:linear-gradient(105deg,transparent 25%,#eff2dd 25%,#eff2dd 40%,transparent 40%,transparent 47%,#eff2dd 47%,#eff2dd 57%,transparent 57%); opacity:.45; transform:skewY(-14deg); }
.share-sheet .paper { position:relative; padding:30px 48px 36px 50px; background:var(--paper); transform:rotate(-4deg); box-shadow:-7px 5px 0 #a6ae8e66,14px 23px 30px #52654c26; }
.share-sheet .paper::before { content:''; position:absolute; top:0; bottom:0; left:16px; width:1px; background:#9da48452; }
.share-sheet .paper::after { content:''; display:block; width:45px; height:2px; background:var(--ink); opacity:.5; margin-top:25px; }
.share-sheet .share-section { margin:0 0 24px; }
.share-sheet .share-section:last-child { margin-bottom:0; }
.share-sheet .identity { color:var(--accent); font:400 20px/1.6 "Noto Sans SC Variable"; margin-bottom:16px; }
.share-sheet.excerpt .paper > .share-section:first-child > .identity { font-size:0; margin:0; height:62px; }
.share-sheet.excerpt .paper > .share-section:first-child > .identity::before { content:'“'; display:block; font:90px/1.2 Georgia,serif; color:var(--ink); opacity:.65; }
.share-sheet .caption { text-align:center; padding-top:38px; }
.share-sheet .eyebrow { font:400 20px/1.7 "Noto Sans SC Variable"; letter-spacing:3px; color:#7b846e; }
.share-sheet .caption h1 { font:500 60px/1.4 "Noto Serif SC Variable"; color:var(--accent); margin:14px 0 0; letter-spacing:4px; overflow-wrap:anywhere; }
.share-sheet .reflection { text-align:center; margin:24px 0 0; font-size:30px; color:var(--accent); }
.share-sheet .reflection .identity { font-size:18px; margin-bottom:6px; color:#7b846e; }
.share-sheet .reflection p:last-child { margin:0; }
.share-sheet .understanding-label { display:none; }
.share-sheet:is(.understanding,.recap) .paper { transform:rotate(-1.5deg); }
.share-sheet:is(.understanding,.recap) .caption { text-align:left; }
.share-sheet:is(.understanding,.recap) .reflection { text-align:left; }
.share-sheet:is(.understanding,.recap) .handwritten { left:auto; right:52px; transform:rotate(8deg); }
.share-sheet footer { position:absolute; bottom:54px; left:150px; right:150px; color:#6e7b68; text-align:center; font:400 22px/1.8 "Noto Sans SC Variable"; overflow-wrap:anywhere; }
.share-sheet footer p { margin:0; }
.share-sheet .page-number { margin-top:14px; font-size:19px; letter-spacing:2px; }
.share-sheet .content p { margin:0 0 16px; white-space:pre-wrap; overflow-wrap:anywhere; }
.share-sheet .share-section > :last-child { margin-bottom:0; }
.share-sheet .content h1,.share-sheet .content h2,.share-sheet .content h3,.share-sheet .content h4,.share-sheet .content h5,.share-sheet .content h6 { font-size:38px; line-height:1.5; margin:12px 0; }
.share-sheet .content .caption h1 { font-size:60px; line-height:1.4; margin:14px 0 0; }
.share-sheet blockquote { margin:0 0 16px; padding-left:20px; border-left:3px solid var(--accent); }
.share-sheet ul,.share-sheet ol { margin:0 0 16px; padding-left:44px; }
.share-sheet li { padding-left:3px; }
.share-sheet code { font:400 28px/1.6 monospace; white-space:pre-wrap; background:#e7e3dc; }
.share-sheet pre { font:400 28px/1.6 monospace; white-space:pre-wrap; overflow-wrap:anywhere; padding:14px; margin:0 0 16px; background:#e7e3dc; }
.share-sheet pre code { background:transparent; }
.share-sheet table { border-collapse:collapse; font-size:28px; line-height:1.5; margin:0 0 16px; width:100%; }
.share-sheet th,.share-sheet td { border:2px solid #a9aaa9; padding:10px; overflow-wrap:normal; }
.share-sheet .katex { font-size:1.1em; }
.share-sheet .katex-display { margin:16px 0; }
.share-sheet a { color:inherit; text-decoration:underline; }
.share-sheet.diagram { font-size:32px; }
.share-sheet.diagram header { top:24px; }
.share-sheet.diagram > .content { top:74px; left:64px; right:64px; }
.share-sheet.diagram .hero { padding:16px; border-radius:24px; }
.share-sheet.diagram .paper { padding:16px; transform:none; box-shadow:none; }
.share-sheet.diagram .paper::before,.share-sheet.diagram .paper::after,.share-sheet.diagram .handwritten,.share-sheet.diagram .eyebrow { display:none; }
.share-sheet.diagram .caption { padding-top:12px; text-align:left; }
.share-sheet.diagram .content .caption h1 { font-size:40px; margin:0; }
.share-sheet.diagram .identity { font-size:28px; margin-bottom:8px; }
.share-sheet.diagram footer { left:64px; right:64px; bottom:32px; font-size:28px; line-height:1.5; }
.share-sheet.diagram .diagram-image { display:block; max-width:100%; margin:auto; object-fit:contain; }
`;
function element(tag: string, text?: string, className?: string): HTMLElement {
  const el = document.createElement(tag);
  if (text !== undefined) el.textContent = text;
  if (className) el.className = className;
  return el;
}

/** Use the reader's Markdown/KaTeX contract; unknown resources must not become empty output. */
export function shareSections(draft: ShareDraft): HTMLElement[] {
  const parts = draft.source.parts.filter(p => draft.selected.includes(p.id));
  if (!parts.length) throw new Error('请至少选择一部分内容。');
  const ordered = draft.layout === 'excerpt' ? [...parts].sort((a, b) => Number(b.id === 'excerpt') - Number(a.id === 'excerpt')) : parts;
  const sections: HTMLElement[] = [];
  if (draft.source.diagram) {
    const graph = draft.source.diagram, { width, height } = shareSize(draft);
    const section = element('section'); section.append(element('div', '图解 · 助手呈现', 'identity'));
    const image = document.createElement('img'); image.src = graph.png; image.className = 'diagram-image';
    const scale = Math.min((width - 192) / graph.width, (height - 510) / graph.height);
    if (scale < .75) throw new Error('图解过长或过宽，缩放后难以阅读；请选择竖版或调整演示布局后重新取图。');
    image.width = Math.round(graph.width * scale); image.height = Math.round(graph.height * scale);
    section.append(image); sections.push(section);
  }
  for (const part of [...ordered, ...(draft.reflection.trim() ? [{ label: '我的感想', text: draft.reflection, format: 'plain' }] : [])]) {
    const section = element('section');
    section.append(element('div', part.label, 'identity'));
    const body = element('div');
    if (part.format === 'plain') body.append(element('p', part.text));
    else body.innerHTML = renderMarkdown(part.text);
    if (body.querySelector('img,video,audio,iframe')) throw new Error('所选内容含尚未支持导出的图片或内嵌资源，请返回选材选择文字部分。');
    if (body.querySelector('.katex-error')) throw new Error('所选公式无法解析，请返回选材检查公式。');
    section.append(...body.childNodes);
    sections.push(section);
  }
  return sections;
}

async function fontsReady(text: string) {
  try {
    const faces = await Promise.all([
      document.fonts.load('400 44px "Noto Serif SC Variable"', text),
      document.fonts.load('600 56px "Noto Serif SC Variable"', text),
      document.fonts.load('400 28px "Noto Sans SC Variable"', text),
    ]);
    if (faces.some(list => !list.length)) throw new Error('missing font');
    await document.fonts.ready;
  } catch { throw new Error('中文字体未能加载，请重试预览。分享编辑仍保留。'); }
}

// Split only at rendered line boundaries. Formulas and table rows remain indivisible.
function splitBlock(block: HTMLElement): HTMLElement[] {
  if (block.matches('table')) {
    const rows = [...block.querySelectorAll('tbody > tr')];
    if (rows.length < 2 || block.querySelector('[rowspan]')) return [block];
    const columns = element('colgroup');
    for (const cell of block.querySelector('tr')?.children ?? []) {
      const column = element('col'); column.style.width = `${cell.getBoundingClientRect().width}px`; columns.append(column);
    }
    return rows.map(row => {
      const table = block.cloneNode(false) as HTMLElement;
      table.append(columns.cloneNode(true));
      const head = block.querySelector('thead'); if (head) table.append(head.cloneNode(true));
      const body = element('tbody'); body.append(row.cloneNode(true)); table.append(body); return table;
    });
  }
  if (block.matches('.katex-display') || block.querySelector('.katex-display')) return [block];
  const points: { node: Node; offset: number; bottom: number }[] = [];
  function visit(node: Node) {
    if (node instanceof Element && node.matches('.katex')) {
      const range = document.createRange(); range.selectNode(node);
      points.push({ node: node.parentNode!, offset: [...node.parentNode!.childNodes].indexOf(node as ChildNode) + 1, bottom: range.getBoundingClientRect().bottom });
    } else if (node.nodeType === Node.TEXT_NODE) {
      const segments = new Intl.Segmenter('zh', { granularity: 'grapheme' });
      for (const { segment, index } of segments.segment(node.textContent ?? '')) {
        const range = document.createRange(); range.setStart(node, index); range.setEnd(node, index + segment.length);
        points.push({ node, offset: index + segment.length, bottom: range.getBoundingClientRect().bottom });
      }
    } else for (const child of node.childNodes) visit(child);
  }
  visit(block);
  const ends = points.filter((point, index) => index === points.length - 1 || points[index + 1].bottom > point.bottom + 20);
  if (ends.length < 2) return [block];
  const result: HTMLElement[] = [];
  let startNode: Node = block, startOffset = 0;
  for (let i = 0; i < ends.length; i++) {
    const end = ends[i], range = document.createRange(); range.setStart(startNode, startOffset);
    if (i === ends.length - 1) range.setEnd(block, block.childNodes.length); else range.setEnd(end.node, end.offset);
    const piece = block.cloneNode(false) as HTMLElement; piece.append(range.cloneContents());
    piece.style.marginBottom = i === ends.length - 1 ? '' : '0';
    if (i > 0 && piece.matches('ol,ul')) (piece.firstElementChild as HTMLElement).style.listStyleType = 'none';
    result.push(piece); startNode = end.node; startOffset = end.offset;
  }
  return result;
}

/** The returned pages stay mounted until dispose, so measurement and output share one layout. */
export async function paginateShare(draft: ShareDraft) {
  const { width, height } = shareSize(draft);
  const sections = shareSections(draft);
  const host = element('div'); host.style.cssText = 'position:fixed;left:-20000px;top:0;pointer-events:none;';
  host.append(element('style', css)); document.body.append(host);
  const pages: HTMLElement[] = [];
  let content: HTMLElement, paper: HTMLElement, hero: HTMLElement, sectionContent: HTMLElement, footer: HTMLElement;
  function startSection(label: string) {
    const reflection = label === '我的感想';
    sectionContent = element('section', undefined, `share-section${reflection ? ' reflection' : ''}`);
    sectionContent.append(element('div', label, 'identity'));
    (reflection ? content : paper).append(sectionContent);
    hero.hidden = !paper.children.length;
  }
  function newPage(label?: string) {
    const page = element('article', undefined, `share-sheet ${draft.palette} ${draft.layout}`);
    page.style.width = `${width}px`; page.style.height = `${height}px`;
    const header = element('header'); header.append(element('div', 'UNDERSTAND BOOK', 'kind'));
    const handwriting = element('div', undefined, 'handwritten'); handwriting.innerHTML = slowerArtwork;
    content = element('div', undefined, 'content'); footer = element('footer');
    hero = element('div', undefined, 'hero'); paper = element('div', undefined, 'paper');
    hero.append(element('div', undefined, 'light'), paper);
    const caption = element('div', undefined, 'caption');
    caption.append(element('div', draft.layout === 'recap' ? '本次阅读的已记录事实' : '读到这里，留下一点想法', 'eyebrow'), element('h1', draft.title.trim() || (draft.layout === 'recap' ? '本次阅读回顾' : '阅读随记')));
    content.append(hero, caption);
    for (const source of [draft.source.association, ...draft.source.sources].filter(Boolean)) footer.append(element('p', source));
    footer.append(element('p', '01 / 01', 'page-number'));
    page.append(header, handwriting, content, footer); host.append(page); pages.push(page);
    if (label) startSection(label);
    return page;
  }
  const fits = () => content.getBoundingClientRect().bottom <= footer.getBoundingClientRect().top - 28;
  const continuePage = (label: string) => {
    if (sectionContent.children.length === 1) sectionContent.remove();
    hero.hidden = !paper.children.length;
    newPage(label);
  };
  try {
    newPage();
    // Mount content before waiting: KaTeX fonts are requested by actual rendered formulas.
    const probe = element('div', undefined, 'share-sheet'); probe.style.height = 'auto';
    const probeContent = element('div', undefined, 'content'); probe.append(probeContent); host.append(probe);
    sections.forEach(section => probeContent.append(section));
    await fontsReady([host.textContent, draft.title, draft.reflection].join('\n'));
    await Promise.all([...host.querySelectorAll('img')].map(image => image.decode()));
    const hasPreviousContent = () => sectionContent.children.length > 1 || paper.children.length > (sectionContent.parentElement === paper ? 1 : 0);
    const add = (block: HTMLElement, label: string) => {
      if (block.matches('ol,ul') && block.children.length > 1) {
        const start = Number(block.getAttribute('start') || 1);
        [...block.children].forEach((item, index) => {
          const list = block.cloneNode(false) as HTMLElement;
          if (block.tagName === 'OL') list.setAttribute('start', String(start + index));
          list.append(item); add(list, label);
        });
        return;
      }
      sectionContent.append(block);
      if (fits()) return;
      block.remove();
      if (hasPreviousContent()) continuePage(label);
      sectionContent.append(block);
      if (fits()) return;
      // Measure in place at the final content width before splitting.
      const pieces = splitBlock(block); block.remove();
      if (pieces.length === 1) throw new Error('所选表格、公式或内容块无法完整放入一页，请返回选材缩小范围或缩短标题与来源。');
      for (const piece of pieces) {
        const previous = sectionContent.lastElementChild;
        if (piece.matches('table') && previous?.matches('table')) {
          const row = piece.querySelector('tbody > tr')!;
          previous.querySelector('tbody')!.append(row);
          if (fits()) continue;
          piece.querySelector('tbody')!.append(row);
        }
        sectionContent.append(piece);
        if (!fits()) { piece.remove(); if (hasPreviousContent()) continuePage(label); sectionContent.append(piece); }
        if (!fits()) throw new Error('所选内容行无法完整放入一页，请返回选材缩小范围。');
      }
    };
    for (const section of sections) {
      const label = section.firstElementChild!.textContent!;
      section.firstElementChild!.remove();
      startSection(label);
      for (const child of [...section.children]) add(child as HTMLElement, label);
    }
    probe.remove();
    for (const [index, page] of pages.entries()) {
      page.querySelector('.page-number')!.textContent = `${String(index + 1).padStart(2, '0')} / ${String(pages.length).padStart(2, '0')}`;
      // Keep attribution next to short content, instead of stranding it at the bottom of the canvas.
      if (pages.length === 1) {
        (page.querySelector('.page-number') as HTMLElement).style.display = 'none';
        const pageFooter = page.querySelector<HTMLElement>('footer')!;
        const top = page.querySelector('.content')!.getBoundingClientRect().bottom - page.getBoundingClientRect().top + 36;
        pageFooter.style.top = `${Math.min(top, height - 64 - pageFooter.getBoundingClientRect().height)}px`;
        pageFooter.style.bottom = 'auto';
      }
      // Detect actual wide objects before rasterizing, rather than cropping them.
      for (const node of page.querySelectorAll<HTMLElement>('pre,table,.katex,td,th')) {
        if (node.scrollWidth > node.clientWidth + 2 && node.clientWidth > 0 || node.offsetWidth > node.closest('.share-section')!.clientWidth + 2)
          throw new Error('所选表格、公式或代码过宽，无法完整放入版面，请返回选材缩小范围。');
      }
    }
    return { pages, dispose: () => host.remove() };
  } catch (error) { host.remove(); throw error; }
}

function dataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = reject; reader.readAsDataURL(blob); });
}

async function embeddedFonts(text: string, includeMath: boolean): Promise<string> {
  const codes = [...text].map(char => char.codePointAt(0)!);
  const rules: { rule: CSSFontFaceRule; base: string }[] = [];
  const walk = (sheet: CSSStyleSheet) => {
    for (const rule of sheet.cssRules) {
      if (rule instanceof CSSFontFaceRule) rules.push({ rule, base: sheet.href || location.href });
      else if (rule instanceof CSSImportRule && rule.styleSheet) walk(rule.styleSheet);
    }
  };
  for (const sheet of document.styleSheets) walk(sheet);
  const selected = rules.filter(({ rule }) => {
    const family = rule.style.fontFamily;
    if (!/Noto (Serif|Sans) SC Variable/.test(family) && !(includeMath && /KaTeX_/.test(family))) return false;
    const ranges = rule.style.getPropertyValue('unicode-range');
    return !ranges || ranges.split(',').some(range => {
      const [start, end = start] = range.trim().replace(/^U\+/i, '').split('-');
      return codes.some(code => code >= parseInt(start.replace(/\?/g, '0'), 16) && code <= parseInt(end.replace(/\?/g, 'f'), 16));
    });
  });
  return (await Promise.all(selected.map(async ({ rule, base }) => {
    let text = rule.cssText;
    for (const match of text.matchAll(/url\(["']?([^"')]+)["']?\)/g)) {
      const response = await fetch(new URL(match[1], base));
      if (!response.ok) throw new Error('导出所需字体读取失败，请重试。');
      text = text.replace(match[0], `url("${await dataUrl(await response.blob())}")`);
    }
    return text;
  }))).join('\n');
}

export async function renderShare(draft: ShareDraft): Promise<Blob[]> {
  const { width, height } = shareSize(draft);
  const result = await paginateShare(draft);
  try {
    const fonts = await embeddedFonts(result.pages.map(page => page.textContent).join(''), result.pages.some(page => !!page.querySelector('.katex')));
    const blobs: Blob[] = [];
    for (const page of result.pages) {
      const clone = page.cloneNode(true) as HTMLElement;
      // Replay the same explicit layout rules and embedded fonts used for pagination.
      clone.prepend(element('style', `${katexCss.replace(/@font-face\s*\{[^}]*\}/g, '')}\n${css}\n${fonts}`));
      const xml = new XMLSerializer().serializeToString(clone);
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><foreignObject width="100%" height="100%">${xml}</foreignObject></svg>`;
      const image = new Image(); image.src = await dataUrl(new Blob([svg], { type: 'image/svg+xml' }));
      await image.decode();
      // SVG decode completes before Chromium finishes painting embedded fonts in foreignObject.
      // Wait for that layout/paint before taking the Canvas snapshot (covered by page-header pixels).
      await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      const canvas = document.createElement('canvas'); canvas.width = width; canvas.height = height;
      const ctx = canvas.getContext('2d'); if (!ctx) throw new Error('当前浏览器无法生成图片。');
      ctx.drawImage(image, 0, 0);
      blobs.push(await new Promise<Blob>((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('图片生成失败，请重试预览。分享编辑仍保留。')), 'image/png')));
    }
    return blobs;
  } finally { result.dispose(); }
}

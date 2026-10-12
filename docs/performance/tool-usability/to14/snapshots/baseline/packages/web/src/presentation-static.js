// Runs inside the existing opaque-origin frame. Freeze pixels and DOM synchronously
// with the bridge's state read; decoding/rasterization happens later in the host.
function () {
  if (document.fonts.status !== 'loaded') throw new Error('图解字体尚未就绪，请稍后重试。');
  const root = document.body;
  const width = root.offsetWidth, height = Math.max(root.offsetHeight, root.scrollHeight);
  if (!width || !height) throw new Error('当前图解没有可导出的画面。');
  if ([...document.fonts].some(face => face.status === 'error')) throw new Error('图解字体加载失败，请重试。');
  const rules = [], resources = new Set();
  let serial = 0;
  const copyStyle = (style, target) => {
    for (const name of style) target.style.setProperty(name, style.getPropertyValue(name));
    target.style.animation = 'none'; target.style.transition = 'none'; target.style.caretColor = 'transparent';
  };
  function clone(node) {
    if (!(node instanceof Element)) return node.cloneNode(true);
    if (node.matches('script,style,link,[data-presentation-mask]')) return document.createTextNode('');
    const style = getComputedStyle(node);
    if (style.display === 'none' || style.visibility === 'hidden') return document.createTextNode('');
    if (node.matches('video,audio,iframe,object,embed') || node.shadowRoot)
      throw new Error('当前图解包含尚不支持静态输出的媒体或嵌入内容。');
    if (node instanceof HTMLElement && /auto|scroll/.test(style.overflow + style.overflowX + style.overflowY)
        && (node.scrollWidth > node.clientWidth + 2 || node.scrollHeight > node.clientHeight + 2))
      throw new Error('图解含滚动区域，请展开内容后重新导出，避免遗漏。');
    for (const property of ['background-image', 'mask-image', 'list-style-image']) {
      for (const match of style.getPropertyValue(property).matchAll(/url\(["']?([^"')]+)["']?\)/g)) {
        if (match[1].startsWith('#')) continue;
        if (!match[1].startsWith('data:')) throw new Error('图解包含未就绪的图片资源，请使用版本内资源。');
        resources.add(match[1]);
      }
    }
    if (node instanceof SVGImageElement) {
      const href = node.href.baseVal;
      if (!href.startsWith('data:')) throw new Error('SVG 图片资源无法嵌入，请使用版本内资源。');
      resources.add(href);
    }
    let target;
    if (node instanceof HTMLCanvasElement) {
      if (!node.getContext('2d')) throw new Error('此 Canvas 使用 WebGL，当前无法静态输出。');
      target = document.createElement('img');
      try { target.src = node.toDataURL('image/png'); } catch { throw new Error('Canvas 资源无法读取，请重新加载图解。'); }
    } else if (node.matches('input,textarea,select')) {
      target = document.createElement('span');
      target.textContent = node.type === 'checkbox' || node.type === 'radio' ? (node.checked ? '☑' : '☐')
        : node instanceof HTMLSelectElement ? Array.from(node.selectedOptions, option => option.text).join('、') : node.value;
    } else {
      if (node instanceof HTMLImageElement && (!node.complete || !node.naturalWidth))
        throw new Error('图解图片资源尚未就绪或加载失败，请重试。');
      target = node.cloneNode(false);
      for (const child of node.childNodes) target.append(clone(child));
    }
    copyStyle(style, target);
    if (node instanceof HTMLCanvasElement) { target.style.width = style.width; target.style.height = style.height; }
    for (const attr of [...target.attributes]) if (attr.name.startsWith('on')) target.removeAttribute(attr.name);
    for (const pseudo of ['::before', '::after']) {
      const value = getComputedStyle(node, pseudo);
      if (!value.content || value.content === 'none' || value.content === 'normal') continue;
      const id = `static-${++serial}`; target.classList.add(id);
      const holder = document.createElement('span'); copyStyle(value, holder);
      rules.push(`.${id}${pseudo}{${holder.style.cssText}}`);
    }
    return target;
  }
  const body = clone(root);
  body.style.margin = '0'; body.style.width = `${width}px`; body.style.height = `${height}px`;
  // Retain only self-contained font faces. Other computed styles are already frozen.
  for (const sheet of document.styleSheets) for (const rule of sheet.cssRules)
    if (rule instanceof CSSFontFaceRule) {
      if ([...rule.style.getPropertyValue('src').matchAll(/url\(["']?([^"')]+)["']?\)/g)].some(match => !match[1].startsWith('data:')))
        throw new Error('图解字体资源无法嵌入，请使用版本内字体。');
      rules.push(rule.cssText);
    }
  const style = document.createElement('style'); style.textContent = rules.join('\n'); body.prepend(style);
  if (root.scrollWidth > width + 2) throw new Error('图解超出当前画布，请展开演示后重新导出。');
  return { width, height, resources: [...resources], svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><foreignObject width="100%" height="100%">${new XMLSerializer().serializeToString(body)}</foreignObject></svg>` };
}

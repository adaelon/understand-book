import { t as uiText } from './i18n';
import { getLocale, translationsFor } from './i18n';
import type { PresentationView } from "./generated/PresentationView";
import commonStyle from "./presentation.css?raw";
import actions from "./presentation-actions.js?raw";
import bridge from "./presentation-bridge.js?raw";
import staticCapture from "./presentation-static.js?raw";
import mediaLifecycle from "./presentation-media.js?raw";
import sourceChipScript from "./source-chip.js?raw";

/** Assemble private logical assets into one opaque-origin document; no host URL is exposed. */
export function presentationDocument(view: PresentationView, channel = crypto.randomUUID()): string {
  const doc = new DOMParser().parseFromString(view.content_files[view.entrypoint] ?? "", "text/html");
  doc.documentElement.lang = getLocale();
  const logicalPath = (path: string) => {
    const base = new URL(view.entrypoint, "https://presentation.invalid/");
    const url = new URL(path, base);
    if (url.origin !== base.origin || url.search || url.hash) throw new Error(uiText('内容只能使用版本内的本地资源。'));
    return decodeURIComponent(url.pathname.slice(1));
  };
  const asset = (path: string) => {
    const value = view.content_files[logicalPath(path)];
    if (value === undefined) throw new Error(uiText('内容资源缺失。'));
    return value;
  };
  const media = (path: string, poster: boolean) => {
    const logical = logicalPath(path);
    const extension = poster ? ".png" : ".mp4";
    const id = logical.startsWith("assets/") && logical.endsWith(extension) ? logical.slice(7, -extension.length) : "";
    const value = view.animation_assets?.[id];
    if (!value) throw new Error(uiText('动画资源缺失。'));
    return poster ? `data:image/png;base64,${value.poster_png_base64}` : `data:video/mp4;base64,${value.video_base64}`;
  };
  doc.querySelectorAll("base, meta[http-equiv]").forEach(node => node.remove());
  doc.querySelectorAll<HTMLScriptElement>("script[src]").forEach(node => {
    node.textContent = asset(node.getAttribute("src")!); node.removeAttribute("src");
  });
  doc.querySelectorAll<HTMLLinkElement>('link[rel="stylesheet"]').forEach(node => {
    const style = doc.createElement("style"); style.textContent = asset(node.getAttribute("href")!); node.replaceWith(style);
  });
  doc.querySelectorAll<HTMLImageElement>("img[src]").forEach(node => {
    const src = node.getAttribute("src")!;
    if (!src.startsWith("data:")) {
      if (src.toLowerCase().endsWith(".png") && logicalPath(src).startsWith("assets/animation-")) { node.src = media(src, true); return; }
      if (!src.toLowerCase().endsWith(".svg")) throw new Error(uiText('此版本只支持 SVG 图片资源。'));
      node.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(asset(src))}`;
    }
  });
  doc.querySelectorAll("video[src], video source[src]").forEach(node => {
    node.setAttribute("src", media(node.getAttribute("src")!, false));
  });
  doc.querySelectorAll("video[poster]").forEach(node => {
    node.setAttribute("poster", media(node.getAttribute("poster")!, true));
  });
  const policy = doc.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; media-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'";
  const style = doc.createElement("style"); style.textContent = commonStyle;
  const script = doc.createElement("script");
  // JSON escaping protects the script element, not the generated page from its own code.
  const data = JSON.stringify({ channel, sources: view.sources, initialState: view.initial_state, restoredState: view.restored_state,
    teachingBinding: view.teaching_binding, actionContract: view.action_contract,
    translations: translationsFor([
      '帮助区域尚未显示。', '帮助展示记录未确认。', '图解图片资源加载失败，请重新加载后重试。', '来源不可用', '参数 {0}', '已选', '未选',
      '图解字体尚未就绪，请稍后重试。', '当前图解没有可导出的画面。', '图解字体加载失败，请重试。',
      '当前图解包含尚不支持静态输出的媒体或嵌入内容。', '图解含滚动区域，请展开内容后重新导出，避免遗漏。',
      '图解包含未就绪的图片资源，请使用版本内资源。', 'SVG 图片资源无法嵌入，请使用版本内资源。',
      '三维画布尚未就绪，请重新加载图解。', '三维画布未保留画面；制作时请开启 preserveDrawingBuffer 后重新导出。',
      'Canvas 资源无法读取，请重新加载图解。', '图解图片资源尚未就绪或加载失败，请重试。',
      '图解字体资源无法嵌入，请使用版本内字体。', '图解超出当前画布，请展开演示后重新导出。',
    ]) }).replaceAll("<", "\\u003c");
  script.textContent = `${sourceChipScript.replace('export function', 'function')}\n(${bridge.trim()})(${data}, (${staticCapture.trim()}), (${actions.trim()}));\n${mediaLifecycle.replaceAll("\"agent-presentation\"", JSON.stringify(channel))}`;
  doc.head.prepend(policy, style, script);
  return "<!doctype html>" + doc.documentElement.outerHTML;
}

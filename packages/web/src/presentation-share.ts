import type { PresentationView } from './generated/PresentationView';
import type { PresentationState } from './generated/PresentationState';
import type { ShareSource } from './reading-share';
import { presentationDocument } from './presentation-document';
import { acceptsPresentationMessage } from './presentation-host';

export type StaticScene = { image: { svg: string; width: number; height: number; resources?: string[] }; state: PresentationState; parameters?: { label: string; value: string }[] };

export function capturePresentation(frame: HTMLIFrameElement, channel: string): Promise<StaticScene> {
  return new Promise((resolve, reject) => {
    const id = crypto.randomUUID();
    const finish = (error?: Error, result?: StaticScene) => {
      clearTimeout(timer); window.removeEventListener('message', receive);
      if (error) reject(error); else resolve(result!);
    };
    const receive = (event: MessageEvent) => {
      if (!acceptsPresentationMessage(event, frame.contentWindow, channel)) return;
      const message = event.data;
      if (message.kind === 'error') finish(new Error('图解运行失败，请重新打开后重试。'));
      if (message.request_id !== id) return;
      if (message.kind === 'static-result') finish(undefined, message);
      if (message.kind === 'static-error') finish(new Error(message.message));
    };
    const timer = setTimeout(() => finish(new Error('未收到完整图解，请等待资源就绪后重试。')), 10000);
    window.addEventListener('message', receive);
    frame.contentWindow?.postMessage({ channel, kind: 'export-static', request_id: id }, '*');
  });
}

// JSON objects may return from Rust with a different key order; arrays retain order.
function sameValues(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (!left || !right || typeof left !== 'object' || typeof right !== 'object') return false;
  if (Array.isArray(left) || Array.isArray(right)) return Array.isArray(left) && Array.isArray(right)
    && left.length === right.length && left.every((value, index) => sameValues(value, right[index]));
  const a = left as Record<string, unknown>, b = right as Record<string, unknown>;
  return Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key => Object.hasOwn(b, key) && sameValues(a[key], b[key]));
}

/** Restore the retained version in a separate sandbox, leaving the reader's live frame intact. */
export async function captureRetainedPresentation(view: PresentationView, observe: (text: string, ids: string[]) => Promise<{ accepted: boolean }>): Promise<StaticScene> {
  if (!view.restored_state) throw new Error('记录时的现场暂不可用，请重新打开笔记。');
  const channel = crypto.randomUUID(), frame = document.createElement('iframe');
  frame.setAttribute('sandbox', 'allow-scripts'); frame.setAttribute('aria-hidden', 'true');
  frame.title = '准备记录时的图解';
  frame.style.cssText = 'position:fixed;left:-20000px;top:0;width:960px;height:640px;border:0;';
  frame.srcdoc = presentationDocument(view, channel);
  let active = true, rejectReady: (error: Error) => void = () => {};
  let timer: ReturnType<typeof setTimeout>;
  let readyResolve: () => void = () => {};
  const ready = new Promise<void>((resolve, reject) => {
    rejectReady = reject;
    timer = setTimeout(() => reject(new Error('记录现场未能完成恢复，请重试。')), 10000);
    readyResolve = resolve;
  });
  function receive(event: MessageEvent) {
    if (!active || !acceptsPresentationMessage(event, frame.contentWindow, channel)) return;
    const message = event.data;
    if (message.kind === 'restore-partial') rejectReady(new Error('此版本无法完整恢复记录时的参数和步骤，不能分享记录图解。'));
    if (message.kind === 'error') rejectReady(new Error('记录时的图解恢复失败，请重试。'));
    if (message.kind === 'ready') readyResolve();
  }
  window.addEventListener('message', receive); document.body.append(frame);
  try {
    await ready; clearTimeout(timer!);
    const scene = await capturePresentation(frame, channel);
    const result = await observe(scene.state.observed_result, scene.state.source_ref_ids);
    if (!result.accepted) throw new Error('记录图解的文字或来源不可用。');
    const saved = view.restored_state;
    if (!sameValues(scene.state.values, saved.values) || scene.state.visible_step !== saved.visible_step
        || scene.state.observed_result !== saved.observed_result)
      throw new Error('恢复的图面与记录时的参数或结果不一致，无法分享；可打开演示查看。');
    return scene;
  } finally { active = false; clearTimeout(timer!); window.removeEventListener('message', receive); frame.remove(); }
}

export async function presentationShareSource(view: PresentationView, scene: StaticScene,
  resolveSource: (id: string) => Promise<{ label: string; material_title?: string | null; stale?: boolean }>, retained = false): Promise<ShareSource> {
  try {
    await Promise.all((scene.image.resources ?? []).map(async src => { const resource = new Image(); resource.src = src; await resource.decode(); }));
  } catch { throw new Error('图解的背景或 SVG 图片资源加载失败，请重试。'); }
  const image = new Image();
  image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(scene.image.svg)}`;
  try { await image.decode(); } catch { throw new Error('图解静态画面无法解码，请重新导出。'); }
  await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  const canvas = document.createElement('canvas'); canvas.width = scene.image.width * 2; canvas.height = scene.image.height * 2;
  const context = canvas.getContext('2d'); if (!context) throw new Error('当前浏览器无法生成图解。');
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  let png: string;
  try { png = canvas.toDataURL('image/png'); } catch { throw new Error('图解包含无法导出的资源。'); }
  const sources = await Promise.all(view.sources.map(async source => {
    try {
      const resolved = await resolveSource(source.source_ref_id);
      return `${resolved.material_title ? `《${resolved.material_title}》 · ` : ''}${resolved.label || source.label}${resolved.stale ? '（来源暂不可用）' : ''}`;
    } catch { return `${source.label}（来源暂不可用）`; }
  }));
  const parameters = (scene.parameters ?? []).map(parameter => `${parameter.label} = ${parameter.value}`).join('\n');
  return {
    parts: [
      { id: 'body', label: '图解现场', format: 'plain', text: [scene.state.visible_step && `步骤：${scene.state.visible_step}`, parameters && `参数：\n${parameters}`, scene.state.observed_result].filter(Boolean).join('\n') },
      { id: 'excerpt', label: '助手说明', format: 'markdown', text: [...view.readable_view.parts.flatMap(part => part.kind === 'markdown' ? [part.text] : []), ...view.assumptions].join('\n\n') },
    ].filter(part => part.text.trim()) as ShareSource['parts'],
    sources: sources.length ? sources : ['无已记录出处'],
    association: `${retained ? '记录时的图解' : '当前图解'} · 版本 ${view.reference.revision} · 图解与说明由助手提供`,
    diagram: { png, width: scene.image.width, height: scene.image.height, title: view.title },
  };
}

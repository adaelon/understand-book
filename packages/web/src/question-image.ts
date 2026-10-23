import type { VideoQuestionImage } from '../../core/src/generated/VideoQuestionImage';
import type { FrameRegion } from '../../core/src/generated/FrameRegion';

export function questionImageUrl(image: VideoQuestionImage) { return `data:image/png;base64,${image.png_base64}`; }

function pixels(source: CanvasImageSource, x: number, y: number, width: number, height: number, origin: VideoQuestionImage['origin']): VideoQuestionImage {
  const scale = Math.min(1, 2048 / Math.max(width, height));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('无法生成图片，请使用系统截图后粘贴');
  context.drawImage(source, x, y, width, height, 0, 0, canvas.width, canvas.height);
  const png_base64 = canvas.toDataURL('image/png').split(',')[1];
  if (!png_base64 || png_base64.length > 5_592_408) throw new Error('图片超过 4 MiB，请裁小后重试');
  return { png_base64, width: canvas.width, height: canvas.height, origin };
}

/** Capture the browser's displayed pixels, with no seek or server-side decode. */
export function captureQuestionImage(video: HTMLVideoElement, region: FrameRegion): VideoQuestionImage {
  if (video.seeking || video.readyState < 2) throw new Error('请等待画面加载完成后重新框选');
  const [x, y, width, height] = region.rect_normalized;
  const left = Math.round(x * video.videoWidth), top = Math.round(y * video.videoHeight);
  return pixels(video, left, top, Math.min(video.videoWidth - left, Math.round(width * video.videoWidth)),
    Math.min(video.videoHeight - top, Math.round(height * video.videoHeight)), 'selection');
}

export type ImageRect = [number, number, number, number];
export function scoreImageRect(box: { left: number; top: number; width: number; height: number }, start: { x: number; y: number }, end: { x: number; y: number }): ImageRect | null {
  if (box.width <= 0 || box.height <= 0) return null;
  const clamp = (n: number) => Math.max(0, Math.min(1, n));
  const x1 = clamp((start.x - box.left) / box.width), y1 = clamp((start.y - box.top) / box.height);
  const x2 = clamp((end.x - box.left) / box.width), y2 = clamp((end.y - box.top) / box.height);
  if (Math.abs(x2 - x1) * box.width < 4 || Math.abs(y2 - y1) * box.height < 4) return null;
  return [Math.min(x1, x2), Math.min(y1, y2), Math.abs(x2 - x1), Math.abs(y2 - y1)];
}

/** Snapshot the rendered notation, including its glyph definitions and current voice colors. */
export async function captureScoreQuestionImage(svg: SVGSVGElement, rect: ImageRect): Promise<VideoQuestionImage> {
  const box = svg.getBoundingClientRect();
  if (!box.width || !box.height) throw new Error('请等待乐谱显示后重新框选');
  const [x, y, w, h] = rect;
  const width = box.width * w, height = box.height * h;
  const scale = Math.min(2, 2048 / Math.max(width, height));
  const outputWidth = Math.max(1, Math.round(width * scale)), outputHeight = Math.max(1, Math.round(height * scale));
  const ns = 'http://www.w3.org/2000/svg', root = document.createElementNS(ns, 'svg');
  root.setAttribute('width', String(outputWidth)); root.setAttribute('height', String(outputHeight));
  root.setAttribute('viewBox', `${x * box.width} ${y * box.height} ${width} ${height}`);
  const background = document.createElementNS(ns, 'rect');
  background.setAttribute('width', String(box.width)); background.setAttribute('height', String(box.height)); background.setAttribute('fill', '#fffefa');
  const clone = svg.cloneNode(true) as SVGSVGElement;
  clone.setAttribute('width', String(box.width)); clone.setAttribute('height', String(box.height));
  clone.style.width = `${box.width}px`; clone.style.height = `${box.height}px`; clone.style.color = getComputedStyle(svg).color;
  root.append(background, clone);
  const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(root)], { type: 'image/svg+xml' }));
  try {
    const image = new Image();
    await new Promise<void>((resolve, reject) => { image.onload = () => resolve(); image.onerror = () => reject(new Error('无法生成谱面截图，请使用系统截图后粘贴')); image.src = url; });
    return pixels(image, 0, 0, outputWidth, outputHeight, 'selection');
  } finally { URL.revokeObjectURL(url); }
}

export async function questionImageFromFile(file: File): Promise<VideoQuestionImage> {
  if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) throw new Error('请选择 PNG、JPEG 或 WebP 图片');
  if (file.size > 20 * 1024 * 1024) throw new Error('原图片超过 20 MiB，请裁小后上传');
  let bitmap: ImageBitmap;
  try { bitmap = await createImageBitmap(file); }
  catch { throw new Error('无法读取这张图片，请重新截图或选择其他图片'); }
  try { return pixels(bitmap, 0, 0, bitmap.width, bitmap.height, 'upload'); }
  finally { bitmap.close(); }
}

export async function copyQuestionImage(image: VideoQuestionImage) {
  if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') throw new Error('当前环境不支持复制图片，可直接发送已附图片');
  const bytes = Uint8Array.from(atob(image.png_base64), c => c.charCodeAt(0));
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([bytes], { type: 'image/png' }) })]);
}

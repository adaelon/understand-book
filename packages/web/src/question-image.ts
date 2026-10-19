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

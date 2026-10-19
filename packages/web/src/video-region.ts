import type { FrameLocation } from '../../core/src/generated/FrameLocation';
import type { FrameRegion } from '../../core/src/generated/FrameRegion';

export type DisplayRect = { left: number; top: number; width: number; height: number };
export type Point = { x: number; y: number };

/** object-fit: contain, centered: exclude the player's letterbox/pillarbox. */
export function videoContentRect(box: DisplayRect, width: number, height: number): DisplayRect {
  const scale = Math.min(box.width / width, box.height / height);
  return { left: box.left + (box.width - width * scale) / 2,
    top: box.top + (box.height - height * scale) / 2, width: width * scale, height: height * scale };
}

export function videoRegion(frame: FrameLocation, box: DisplayRect, start: Point, end: Point): FrameRegion | null {
  if (box.width <= 0 || box.height <= 0 || start.x < box.left || start.x > box.left + box.width
    || start.y < box.top || start.y > box.top + box.height) return null;
  const point = (p: Point) => ({ x: Math.max(0, Math.min(1, (p.x - box.left) / box.width)) * frame.width,
    y: Math.max(0, Math.min(1, (p.y - box.top) / box.height)) * frame.height });
  const a = point(start), b = point(end);
  if (Math.abs(a.x - b.x) < 1 || Math.abs(a.y - b.y) < 1) return null;
  // Snap the pointer to the nearest source-pixel edge. Browser layout and pointer
  // coordinates have different subpixel precision; enclosing rounding adds a pixel.
  const left = Math.round(Math.min(a.x, b.x)), top = Math.round(Math.min(a.y, b.y));
  const right = Math.round(Math.max(a.x, b.x)), bottom = Math.round(Math.max(a.y, b.y));
  return { frame, rect_normalized: [left / frame.width, top / frame.height,
    (right - left) / frame.width, (bottom - top) / frame.height] };
}

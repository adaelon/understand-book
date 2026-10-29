import type { BoardItem } from './generated/BoardItem';
import { graphicTextLines } from './board-canvas-geometry';

interface DrawingTextLine {
  text: string;
  characters: string[];
  characterOffset: number;
  x: number;
  y: number;
  fontSize: number;
  weight: number;
}

export interface DrawingAppearance {
  outline: string;
  secondOutline: string;
  fill: string;
  stroke: string;
  textColor: string;
  characterCount: number;
  lines: DrawingTextLine[];
}

// Gentle curves keep the native SVG editable and the outline stable during playback.
function shapeOutline(width: number, height: number, ellipse: boolean, second = false): string {
  const left = 3, top = 3, right = Math.max(left, width - 3), bottom = Math.max(top, height - 3);
  const w = right - left, h = bottom - top, bend = Math.min(1.2, w / 180, h / 90) * (second ? -1 : 1);
  const number = (value: number) => Math.round(value * 100) / 100;
  const point = (x: number, y: number) => `${number(x)} ${number(y)}`;
  if (ellipse) {
    const cx = width / 2, cy = height / 2, ox = w * .276, oy = h * .276;
    return `M ${point(cx, top + bend)} C ${point(cx + ox, top - bend)}, ${point(right + bend, cy - oy)}, ${point(right, cy)} C ${point(right - bend, cy + oy)}, ${point(cx + ox, bottom + bend)}, ${point(cx, bottom)} C ${point(cx - ox, bottom - bend)}, ${point(left - bend, cy + oy)}, ${point(left, cy)} C ${point(left + bend, cy - oy)}, ${point(cx - ox, top - bend)}, ${point(cx, top + bend)} Z`;
  }
  const radius = Math.min(13, w / 6, h / 5);
  return `M ${point(left + radius, top)} C ${point(left + w * .36, top - bend)}, ${point(right - w * .28, top + bend)}, ${point(right - radius, top)} Q ${point(right + bend, top)}, ${point(right, top + radius)} C ${point(right - bend, top + h * .35)}, ${point(right + bend, bottom - h * .3)}, ${point(right, bottom - radius)} Q ${point(right, bottom + bend)}, ${point(right - radius, bottom)} C ${point(left + w * .62, bottom - bend)}, ${point(left + w * .3, bottom + bend)}, ${point(left + radius, bottom)} Q ${point(left - bend, bottom)}, ${point(left, bottom - radius)} C ${point(left + bend, top + h * .62)}, ${point(left - bend, top + h * .3)}, ${point(left, top + radius)} Q ${point(left, top - bend)}, ${point(left + radius, top)} Z`;
}

function titleText(text: string): string {
  return text.trim().replace(/^(?:[①-⑳]|\d+[.)、]|[一二三四五六七八九十]+[、.])\s*/u, '').replace(/[：:，,。.!！?？]+$/u, '').replace(/\s+/gu, '');
}

export function drawingAppearance(item: BoardItem): DrawingAppearance {
  const appearance: DrawingAppearance = { outline: '', secondOutline: '', fill: '', stroke: '', textColor: 'var(--ink)', characterCount: 0, lines: [] };
  if (item.target.kind !== 'drawing' || item.target.graphic.kind === 'path') return appearance;
  const graphic = item.target.graphic, shape = graphic.kind === 'shape', ellipse = shape && graphic.shape === 'ellipse';
  if (shape) {
    appearance.outline = shapeOutline(item.size.width, item.size.height, ellipse);
    appearance.secondOutline = shapeOutline(item.size.width, item.size.height, ellipse, true);
    appearance.fill = `color-mix(in srgb, ${graphic.color} 15%, var(--reader-canvas))`;
    appearance.stroke = `color-mix(in srgb, ${graphic.color} 76%, var(--ink))`;
    appearance.textColor = `color-mix(in srgb, ${graphic.color} 24%, var(--ink))`;
  }
  const fullText = graphic.text, paragraphs = fullText.split('\n');
  const width = Math.max(16, ellipse ? item.size.width * .7 : item.size.width - (shape ? 36 : 16));
  const height = Math.max(16, ellipse ? item.size.height * .7 : item.size.height - (shape ? 20 : 12));
  const heading = shape && paragraphs.length > 1 && paragraphs[0].trim().length <= 24 && Boolean(titleText(item.title)) && titleText(paragraphs[0]) === titleText(item.title);
  const makeLines = (size: number) => paragraphs.flatMap((paragraph, index) => {
    const isHeading = heading && index === 0;
    const fontSize = isHeading ? size + 3 : size;
    return graphicTextLines(paragraph, width, fontSize).map((text, lineIndex) => ({
      text, fontSize, weight: isHeading ? 620 : shape && paragraphs.length === 1 ? 500 : 400,
      lineHeight: fontSize * (isHeading ? 1.35 : 1.45), gap: heading && index === 1 && lineIndex === 0 ? 3 : 0,
    }));
  });
  let fontSize = shape ? (paragraphs.length === 1 ? 21 : 18) : 22;
  let lines = makeLines(fontSize);
  const totalHeight = () => lines.reduce((height, line) => height + line.lineHeight + line.gap, 0);
  while (fontSize > 10 && totalHeight() > height) { fontSize--; lines = makeLines(fontSize); }
  let y = shape ? (item.size.height - totalHeight()) / 2 : 6, sourceOffset = 0;
  appearance.characterCount = Array.from(fullText).length;
  appearance.lines = lines.map(line => {
    y += line.gap;
    const start = fullText.indexOf(line.text, sourceOffset);
    const characterOffset = Array.from(fullText.slice(0, start)).length;
    sourceOffset = start + line.text.length;
    const result = { text: line.text, characters: Array.from(line.text), characterOffset, x: shape ? item.size.width / 2 : 8, y: y + line.fontSize, fontSize: line.fontSize, weight: line.weight };
    y += line.lineHeight;
    return result;
  });
  return appearance;
}

export function revealDrawingLines(appearance: DrawingAppearance, progress: number): DrawingTextLine[] {
  if (progress >= 1) return appearance.lines;
  const count = Math.ceil(appearance.characterCount * Math.max(0, progress));
  return appearance.lines.map(line => ({ ...line, text: line.characters.slice(0, Math.max(0, count - line.characterOffset)).join('') }));
}

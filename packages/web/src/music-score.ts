import type { MusicScoreObject, MusicNoteAlignment } from '../../core/src/music-contract';
import { musicNoteOpacity } from './music-playback';
const NS = 'http://www.w3.org/2000/svg';
interface Mark { element: SVGGraphicsElement; dot: SVGCircleElement; halo: SVGCircleElement; color: string; baseColor: string; staticColor?: string; part: string; voice: string }
/** Geometry belongs to this rendered SVG. Both score and marks share its viewBox transform. */
export class MusicScoreMarkers {
  private marks = new Map<string, Mark>();
  private anchors = new Map<string, SVGGraphicsElement>();
  readonly ready: boolean = false;
  private visible = new Set<string>();
  private overlay = document.createElementNS(NS, 'g');
  constructor(readonly svg: SVGSVGElement, objects: MusicScoreObject[], color: (partId: string, voice?: string) => string) {
    const overlay = this.overlay;
    overlay.setAttribute('aria-hidden', 'true'); overlay.classList.add('music-note-overlay'); overlay.style.pointerEvents = 'none'; overlay.style.display = 'none';
    // A display:none ancestor can return identity matrices for every descendant.
    // Measure only when the SVG participates in layout, then rebuild on reveal.
    const bounds = svg.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const rootMatrix = svg.getScreenCTM();
    if (!rootMatrix) return;
    this.ready = true;
    const inverse = rootMatrix.inverse();
    for (const object of objects) {
      const element = svg.querySelector<SVGGraphicsElement>(`#${CSS.escape(object.svg_element_id)}`);
      if (element) this.anchors.set(object.source_id, element);
      if (object.rest) continue;
      const head = element?.querySelector<SVGGraphicsElement>('.notehead') ?? element;
      if (!element || !head?.getScreenCTM) continue;
      const box = head.getBBox(), matrix = head.getScreenCTM();
      if (!matrix) continue;
      const center = new DOMPoint(box.x + box.width / 2, box.y + box.height / 2).matrixTransform(matrix).matrixTransform(inverse);
      const edge = new DOMPoint(box.x + box.width, box.y + box.height / 2).matrixTransform(matrix).matrixTransform(inverse);
      const radius = Math.max(2, Math.abs(edge.x - center.x) * 0.8);
      const tint = color(object.part_id, object.voice);
      const halo = document.createElementNS(NS, 'circle'), dot = document.createElementNS(NS, 'circle');
      for (const marker of [halo, dot]) { marker.setAttribute('cx', String(center.x)); marker.setAttribute('cy', String(center.y)); marker.setAttribute('fill', tint); marker.style.opacity = '0'; overlay.appendChild(marker); }
      dot.classList.add('music-note-dot'); dot.dataset.scoreObjectId = object.source_id;
      halo.setAttribute('r', String(radius * 2.8)); dot.setAttribute('r', String(radius));
      halo.style.filter = `blur(${radius * 0.8}px)`;
      this.marks.set(object.source_id, { element, dot, halo, color: tint, baseColor: tint, part: object.part_id, voice: object.voice });
    }
  }
  /** Attach after all pages have been measured to avoid read/write layout cycles. */
  attach() { this.svg.appendChild(this.overlay); }
  render(notes: MusicNoteAlignment[], ms: number, playing: boolean) {
    const next = new Set<string>();
    for (const note of notes) {
      const mark = this.marks.get(note.score_object_id); if (!mark) continue;
      const opacity = musicNoteOpacity(note, ms, playing); if (!opacity) continue;
      next.add(note.score_object_id);
      mark.dot.style.opacity = String(opacity * (note.attack ? .9 : .4));
      mark.halo.style.opacity = String(note.attack ? opacity * .3 : 0);
      if (ms < note.end_ms) { mark.element.style.color = mark.color; mark.element.style.fill = mark.color; }
      else if (mark.staticColor) { mark.element.style.color = mark.staticColor; mark.element.style.fill = mark.staticColor; }
      else { mark.element.style.removeProperty('color'); mark.element.style.removeProperty('fill'); }
    }
    for (const id of this.visible) if (!next.has(id)) { const mark = this.marks.get(id)!; mark.dot.style.opacity = mark.halo.style.opacity = '0'; if (mark.staticColor) { mark.element.style.color = mark.staticColor; mark.element.style.fill = mark.staticColor; } else { mark.element.style.removeProperty('color'); mark.element.style.removeProperty('fill'); } }
    if (!!next.size !== !!this.visible.size) this.overlay.style.display = next.size ? '' : 'none';
    this.visible = next;
  }
  element(id: string): SVGGraphicsElement | undefined { return this.anchors.get(id); }
  colors(roleColors: ReadonlyMap<string, string> | null) { for (const [id, mark] of this.marks) { mark.staticColor = roleColors?.get(id); mark.color = mark.staticColor ?? mark.baseColor; mark.dot.setAttribute('fill', mark.color); mark.halo.setAttribute('fill', mark.color); if (mark.staticColor) { mark.element.style.color = mark.staticColor; mark.element.style.fill = mark.staticColor; } else { mark.element.style.removeProperty('fill'); if (!this.visible.has(id)) mark.element.style.removeProperty('color'); } } }
  focus(included: (part: string, voice: string) => boolean) { for (const mark of this.marks.values()) mark.element.style.opacity = included(mark.part, mark.voice) ? '1' : '.22'; }
  dispose() { this.svg.querySelector('.music-note-overlay')?.remove(); for (const mark of this.marks.values()) { mark.element.style.removeProperty('color'); mark.element.style.removeProperty('fill'); mark.element.style.removeProperty('opacity'); } this.marks.clear(); this.anchors.clear(); }
}

export interface TurnScreen {
  element: HTMLElement;
  start: { index: number; offset: number };
  eof: boolean;
  setInteractive?(active: boolean): void;
}
export interface PageTurnReader {
  pagination: { visible(): TurnScreen | undefined | null };
  adjacent(direction: number): TurnScreen | undefined | null;
  move(direction: number): unknown;
  setPaused(paused: boolean): void;
  cancelPending?(): void;
}

const ownsInput = (target: EventTarget | null) => target instanceof Element && !!target.closest(
  'a, button, input, textarea, select, [contenteditable="true"], .focus-object, .katex, [role="button"], [role="dialog"], [data-reader-selection-ignore]',
);
const hasSelection = () => { const selection = window.getSelection(); return !!selection && !selection.isCollapsed; };
const interactive = (screen: TurnScreen, active: boolean) => {
  if (screen.setInteractive) screen.setInteractive(active); else screen.element.inert = !active;
};
type Drag = { id: number; x: number; y: number; at: number; type: string; claimed: boolean; lastX: number; lastAt: number; velocity: number };
type Motion = { from: TurnScreen; to?: TurnScreen; direction: number; progress: number; committed: boolean };

/** Owns only the current/adjacent screen transforms. No source or geometry reads
 * occur in move/animation frames, and preparation stays paused until settling. */
export class FocusPageTurn {
  private drag?: Drag;
  private motion?: Motion;
  private animation?: Animation;
  private frame = 0;
  private left = 0;
  private width = 1;
  private sequence = 0;
  private enabled = true;
  private phase: 'idle' | 'dragging' | 'settling' = 'idle';
  constructor(private reader: PageTurnReader, private host: HTMLElement, private hooks: {
    reducedMotion(): boolean; tools(): void; settled(): void; ownsPointer?(event: PointerEvent): boolean;
  }) {
    host.addEventListener('pointerdown', this.down);
    host.addEventListener('pointermove', this.move);
    window.addEventListener('pointerup', this.up);
    window.addEventListener('pointercancel', this.cancelPointer);
    host.addEventListener('lostpointercapture', this.cancelPointer);
    host.addEventListener('click', this.click, true);
    document.addEventListener('keydown', this.key);
    document.addEventListener('selectionchange', this.selection);
  }
  bounds(left: number, width: number) { this.left = left; this.width = Math.max(1, width); }
  busy() { return this.phase !== 'idle' || !!this.drag; }
  snapshot() { return { phase: this.phase, direction: this.motion?.direction ?? 0, progress: this.motion?.progress ?? 0 }; }
  setEnabled(enabled: boolean) { this.enabled = enabled; if (!enabled) this.cancel(); }
  private setPhase(phase: typeof this.phase) { this.phase = phase; this.host.dataset.turnPhase = phase; }
  private resetScreens() {
    const current = this.reader.pagination.visible();
    for (const screen of [this.motion?.from, this.motion?.to]) {
      if (!screen) continue;
      screen.element.style.transform = ''; screen.element.style.zIndex = ''; screen.element.style.willChange = '';
      screen.element.hidden = screen !== current;
      interactive(screen, screen === current);
      if (screen !== current) screen.element.setAttribute('aria-hidden', 'true'); else screen.element.removeAttribute('aria-hidden');
    }
    this.motion = undefined;
  }
  private stopAnimation() {
    this.sequence++; this.animation?.cancel(); this.animation = undefined;
    cancelAnimationFrame(this.frame); this.frame = 0;
  }
  cancel() {
    this.stopAnimation(); this.drag = undefined; this.resetScreens(); this.setPhase('idle');
    this.reader.setPaused(false); this.hooks.settled();
  }
  private begin(direction: number) {
    this.resetScreens();
    const from = this.reader.pagination.visible(); if (!from) return false;
    const to = this.reader.adjacent(direction) ?? undefined;
    this.motion = { from, to, direction, progress: 0, committed: false };
    for (const screen of [from, to]) {
      if (!screen) continue;
      screen.element.hidden = false; interactive(screen, false);
      screen.element.setAttribute('aria-hidden', 'true');
    }
    const moving = direction > 0 || !to ? from : to;
    moving.element.style.willChange = 'transform'; moving.element.style.zIndex = '2';
    const beneath = moving === from ? to : from;
    if (beneath) beneath.element.style.zIndex = '1';
    this.paint(); return true;
  }
  private transform(progress: number) {
    const motion = this.motion!;
    const x = !motion.to ? -motion.direction * progress * this.width * .12
      : motion.direction > 0 ? -progress * this.width : (progress - 1) * this.width;
    return `translate3d(${x}px,0,0)`;
  }
  private moving() { const m = this.motion!; return m.direction > 0 || !m.to ? m.from.element : m.to.element; }
  private paint() { this.frame = 0; if (this.motion) this.moving().style.transform = this.transform(this.motion.progress); }
  private finish() {
    this.animation = undefined; this.resetScreens(); this.setPhase('idle');
    this.reader.setPaused(false); this.hooks.settled();
  }
  private settle(commit: boolean) {
    if (!this.motion) { this.finish(); return; }
    cancelAnimationFrame(this.frame); this.frame = 0;
    const motion = this.motion;
    if (commit) {
      if (!motion.to) { const direction = motion.direction; this.finish(); this.reader.move(direction); return; }
      this.setPhase('settling'); this.reader.move(motion.direction); motion.committed = true;
      // show() confirms the canonical page, while the outgoing DOM remains only
      // as the inert animation layer until this bounded transition completes.
      motion.from.element.hidden = false; interactive(motion.from, false);
      interactive(motion.to, false);
    }
    if (this.hooks.reducedMotion()) { this.finish(); return; }
    const target = commit ? 1 : 0;
    this.setPhase('settling');
    const sequence = ++this.sequence;
    const animation = this.moving().animate([{ transform: this.transform(motion.progress) }, { transform: this.transform(target) }], {
      duration: Math.max(80, 200 * Math.abs(target - motion.progress)), easing: 'cubic-bezier(.2,.7,.2,1)', fill: 'forwards',
    });
    // Start on the input's timeline rather than waiting for compositor startup
    // to acknowledge a pending animation while holding the unchanged first frame.
    animation.startTime = document.timeline.currentTime;
    this.animation = animation;
    void animation.finished.then(() => { if (sequence === this.sequence) { animation.cancel(); this.finish(); } }).catch(() => {});
  }
  turn(direction: number): boolean {
    if (!this.enabled || hasSelection()) return false;
    // A new input takes ownership immediately; there is no queued animation.
    this.stopAnimation(); this.drag = undefined; this.resetScreens();
    const current = this.reader.pagination.visible();
    if (!current || (direction > 0 ? current.eof : current.start.index === 0 && current.start.offset === 0)) { this.reader.cancelPending?.(); this.finish(); return true; }
    this.reader.setPaused(true);
    if (!this.begin(direction)) { this.finish(); return false; }
    const ready = !!this.motion?.to;
    this.settle(true); return ready;
  }
  private down = (event: PointerEvent) => {
    if (!this.enabled || event.button !== 0 || !event.isPrimary || ownsInput(event.target) || this.hooks.ownsPointer?.(event) || hasSelection()) return;
    this.stopAnimation(); this.resetScreens(); this.setPhase('idle'); this.reader.setPaused(true);
    this.drag = { id: event.pointerId, x: event.clientX, y: event.clientY, at: event.timeStamp, type: event.pointerType,
      claimed: false, lastX: event.clientX, lastAt: event.timeStamp, velocity: 0 };
  };
  private move = (event: PointerEvent) => {
    const drag = this.drag; if (!drag || event.pointerId !== drag.id || drag.type === 'mouse') return;
    const dx = event.clientX - drag.x, dy = event.clientY - drag.y;
    if (!drag.claimed) {
      if (hasSelection() || event.timeStamp - drag.at > 350 || Math.abs(dy) > 12 && Math.abs(dy) > Math.abs(dx)) { this.cancel(); return; }
      if (Math.abs(dx) < 8 || Math.abs(dx) < Math.abs(dy) * 1.25) return;
      drag.claimed = true; this.host.setPointerCapture(event.pointerId); this.setPhase('dragging');
    }
    event.preventDefault();
    const direction = dx < 0 ? 1 : -1;
    if (this.motion?.direction !== direction) this.begin(direction);
    if (!this.motion) return;
    drag.velocity = (event.clientX - drag.lastX) / Math.max(1, event.timeStamp - drag.lastAt);
    drag.lastX = event.clientX; drag.lastAt = event.timeStamp;
    this.motion.progress = Math.min(1, Math.abs(dx) / this.width);
    if (!this.frame) this.frame = requestAnimationFrame(() => this.paint());
  };
  private suppressClickUntil = 0;
  private click = (event: MouseEvent) => { if (performance.now() < this.suppressClickUntil) { event.preventDefault(); event.stopPropagation(); } };
  private up = (event: PointerEvent) => {
    const drag = this.drag; if (!drag || event.pointerId !== drag.id) return;
    this.drag = undefined;
    if (drag.claimed) {
      this.suppressClickUntil = performance.now() + 400;
      if (this.host.hasPointerCapture(event.pointerId)) this.host.releasePointerCapture(event.pointerId);
      const motion = this.motion;
      const fast = event.timeStamp - drag.lastAt < 100 && Math.abs(drag.velocity) > .5 && -Math.sign(drag.velocity) === motion?.direction;
      this.settle(!!motion && (motion.progress >= .22 || fast)); return;
    }
    this.reader.setPaused(false); this.hooks.settled();
    if (hasSelection() || event.timeStamp - drag.at > 350 || Math.hypot(event.clientX - drag.x, event.clientY - drag.y) > 8) return;
    const position = (event.clientX - this.left) / this.width;
    if (position < .16) this.turn(-1); else if (position > .84) this.turn(1); else this.hooks.tools();
  };
  private cancelPointer = (event: PointerEvent) => {
    // Touch starts with implicit capture on the text descendant. Transferring
    // it to our surface emits a bubbling lost event from that old descendant.
    if (event.type === 'lostpointercapture' && event.target !== this.host) return;
    if (this.drag?.id === event.pointerId) this.cancel();
  };
  private selection = () => { if (this.drag && hasSelection()) this.cancel(); };
  private key = (event: KeyboardEvent) => {
    const pageControl = event.target instanceof Element && !!event.target.closest('.focus-page-prev, .focus-page-next');
    if (!this.enabled || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || !pageControl && ownsInput(event.target) || hasSelection()) return;
    const target = event.target;
    if (target instanceof Element && target !== document.body && !this.host.closest('.reader-pane, .pdf-reader-pane')?.contains(target)) return;
    const direction = ['ArrowRight', 'PageDown'].includes(event.key) ? 1 : ['ArrowLeft', 'PageUp'].includes(event.key) ? -1 : 0;
    if (!direction) return;
    event.preventDefault(); event.stopPropagation(); this.turn(direction);
  };
  dispose() {
    this.enabled = false; this.stopAnimation(); this.drag = undefined; this.resetScreens();
    this.host.removeEventListener('pointerdown', this.down); this.host.removeEventListener('pointermove', this.move);
    window.removeEventListener('pointerup', this.up); window.removeEventListener('pointercancel', this.cancelPointer);
    this.host.removeEventListener('lostpointercapture', this.cancelPointer); this.host.removeEventListener('click', this.click, true);
    document.removeEventListener('keydown', this.key); document.removeEventListener('selectionchange', this.selection);
  }
}

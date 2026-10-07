Konva supply and small API patterns:
Pass libraries:["konva"] on every write that needs Konva, including based_on revisions. The host pins Konva 10.7.0 and its MIT license in the version and inserts a script before your page scripts. Do not copy the library, use a CDN or add another library script. read returns editable page code and a host-managed short script reference: preserve that tag verbatim; write removes/rebuilds it. Managed library paths return metadata, not code.
Create Stage/Layer/Group and stable named objects once, then update their positions, points and labels. Allocate layers by actual update needs; avoid rebuilding the stage each animation frame. Derive stage size from its container and recompute layout when the container changes; do not stretch the canvas independently with CSS. Keep labels and objects aligned at each width. The following are independent fragments, not a page template; adapt domain mappings to the problem.

```js
// Resize in CSS pixels, then recompute your domain-to-screen mapping.
let resizeFrame = 0, previousWidth = -1;
const observer = new ResizeObserver(() => {
  if (resizeFrame) return;
  resizeFrame = requestAnimationFrame(() => {
    resizeFrame = 0;
    const width = container.clientWidth;
    if (width === previousWidth) return;
    previousWidth = width;
    stage.width(width); layout(); renderAt(state);
  });
});
observer.observe(container);
// A small visual dot can still have a >=44px hit diameter at the current scale.
dot.hitFunc((ctx, shape) => {
  const scale = shape.getAbsoluteScale();
  const r = Math.max(shape.radius(), 22 / Math.min(Math.abs(scale.x), Math.abs(scale.y)));
  ctx.beginPath(); ctx.arc(0, 0, r, 0, Math.PI * 2); ctx.closePath(); ctx.fillStrokeShape(shape);
});
// Mouse/touch use the same inverse transform and domain mapping.
dot.on('dragmove', () => {
  const p = group.getRelativePointerPosition();
  if (p) { state.parameter = screenToDomain(p.x); renderAt(state); }
});
dot.on('dragend', () => window.presentation.commitState());
```
Schedule observed size writes in requestAnimationFrame and skip unchanged dimensions. Do not synchronously change an observed container's height from its ResizeObserver callback: Reader expansion can produce a ResizeObserver loop error and close the page. If layout depends on viewport height, include it in the size key and schedule window resize through the same callback. Redraw the hit canvas after resize. The full hit area must survive plot clipping and the stage edges: keep interactive hit shapes outside clipped plot groups, and leave at least half their hit diameter inside every canvas edge, including parameter endpoints. Measure the hit graph at the start/end and parameter boundaries; a nominal 48px circle clipped to 38px is still a failed target. Test the actual touch target after transforms, not just the dot's unscaled radius. Keep native keyboard-accessible controls for the same parameter. A drag is live while held; map screen position back to the domain, clamp in domain units, then render all related objects from that one value.

Presentation capability directory:

- HTML prose, formulas, native controls and inline SVG/Canvas can express static or live relationships without an extra renderer.
- editing: read/search/patch for local revisions; write for a new page, broad redesign or changed resource selection.
- static_plot: render_plot produces a fixed Python/Matplotlib SVG and an inspection image.
- state: save and restore custom page parameters, steps and visible results.
- continuous_scene: deterministic playback and fractional positioning; includes the state contract.
- konva: bundled stable graphic objects, coordinated geometry and direct dragging. It does not require a timeline; select state when custom interaction needs saving.
- manim: render_animation produces a fixed local movie, with media decoding and paused restoration; includes continuous_scene and state. A movie cannot recompute arbitrary parameter changes; use live browser graphics for those.

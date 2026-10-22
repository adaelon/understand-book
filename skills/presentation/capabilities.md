Presentation capability directory:

- HTML prose, formulas, native controls and inline SVG/Canvas can express static or live relationships without an extra renderer.
- editing: read/search/patch for local revisions; write for a new page, broad redesign or changed resource selection.
- static_plot: render_plot produces a fixed Python/Matplotlib SVG and an inspection image.
- state: save and restore custom page parameters, steps and visible results; this basic contract is always included during local construction and review.
- live actions: declare action_contract alongside state, register the shared control handler, and rehearse it with preview interact actions. presentation.interact observes and operates the open delivered version and returns its actual saved scene.
- continuous_scene: deterministic playback and fractional positioning; includes the state contract.
- konva: bundled stable graphic objects, coordinated geometry and direct dragging. It does not require a timeline; use the state contract for custom interaction.
- three: bundled Three.js for real 3D models, orbit and linked diagrams; use the state contract for saved camera/parameters and select continuous_scene when time is part of the explanation. spatial-linked-views@1 provides a reusable starting point.
- manim: render_animation produces a fixed local movie, with media decoding and paused restoration; includes continuous_scene and state. A movie cannot recompute arbitrary parameter changes; use live browser graphics for those.

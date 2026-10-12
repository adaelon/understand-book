# Continuous-scene regression example

This example belongs to the learning-rate demonstration; it is not a general page layout or mandatory scene position.

Check every displayed equation against an independent substitution, including secondary captions. Distinguish a discrete update computed at its start from the interpolated point shown during its motion. For example, with L(w)=(w-2)^2, Δw=-ηL′(w_t)=-2η(w_t-2); applying another factor 2 to L′ is wrong. During that step the arrow still represents this start-state update, not a newly evaluated gradient at the moving interpolated point. Label these quantities separately when showing both.

For the learning-rate model include η=0 and one-step arrival; check zero-motion arrows at later steps. A convex curve lying above its tangent alone does not prove overshoot: the actual next position must cross the target.

For a scene with enough steps, preview {semantic_state:2,transition_progress:0.35}, then {semantic_state:1,transition_progress:0.2}, then repeat the latter. Compare the requested position, screenshot, saved state and paused restored state. Use positions supported by the actual scene.

"""The five real updates used by the EX9a film; animation is not an optimizer."""

ETAS = (0.2, 0.8, 1.1)
STEPS = 5
AXIS_MIN, AXIS_MAX = -2.5, 7.5
AXIS_LEFT, AXIS_RIGHT = -3.5, 5.5


def trajectory(eta, steps=STEPS):
    w = 0.0
    rows = []
    for k in range(steps + 1):
        e = w - 2
        delta = -2 * eta * e
        rows.append(dict(k=k, w=w, e=e, loss=e * e, delta=delta,
                         length=abs(delta), ratio=abs(1 - 2 * eta)))
        w += delta
    return rows


def axis_x(w):
    # No clipping or minimum width: the same affine map serves every lane.
    return AXIS_LEFT + (w - AXIS_MIN) * (AXIS_RIGHT - AXIS_LEFT) / (AXIS_MAX - AXIS_MIN)


def fmt(value):
    return f"{value:.5f}".rstrip("0").rstrip(".") if value else "0"

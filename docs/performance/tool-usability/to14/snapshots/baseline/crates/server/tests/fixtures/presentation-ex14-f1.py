# Controlled engineering fixture: the red WRONG A card exists only at 5.8..6.2s.
class PresentationAnimation(Scene):
    def construct(self):
        clock = ValueTracker(0)
        square = Square(side_length=0.6, stroke_width=0, fill_opacity=1, fill_color="#00A0FF")
        square.add_updater(lambda m: m.move_to([-5 + 1.25 * clock.get_value(), 0, 0]))
        trail = Rectangle(width=0.01, height=0.12, stroke_width=0, fill_opacity=1, fill_color="#8000FF")
        def track(m):
            width = max(0.01, 1.25 * clock.get_value())
            m.stretch_to_fit_width(width).move_to([-5 + width / 2, -0.65, 0])
        trail.add_updater(track)
        good = Text("B", font_size=36).move_to([0, 1.5, 0])
        card = Rectangle(width=3, height=1, stroke_width=0, fill_opacity=1, fill_color="#FF2000")
        bad = VGroup(card, Text("WRONG A", font_size=30)).move_to([0, 1.5, 0])
        bad.add_updater(lambda m: m.set_opacity(1 if data['fault_start'] <= clock.get_value() < data['fault_end'] else 0))
        good.add_updater(lambda m: m.set_opacity(0 if data['fault_start'] <= clock.get_value() < data['fault_end'] else 1))
        self.add(trail, square, good, bad)
        self.play(clock.animate.set_value(8), run_time=8, rate_func=linear)

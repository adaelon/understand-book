"""Run with `python -m manim ... scene.py LearningRate` (see README)."""
import json
import os
from pathlib import Path

import numpy as np
from manim import *

from model import ETAS, STEPS, AXIS_MIN, AXIS_MAX, axis_x, fmt, trajectory

BG = "#F4F6F8"
INK = "#1C2938"
MUTED = "#526274"
COLORS = ("#1769AA", "#BA5A19", "#BD334A")
FONT = "Microsoft YaHei"


def label(text, size=24, color=INK):
    return Text(text, font=FONT, font_size=size, color=color)


def math(text, size=30, color=INK):
    return MathTex(text, font_size=size, color=color)


class LearningRate(Scene):
    def construct(self):
        self.camera.background_color = BG
        self.events = []
        rows = [trajectory(eta) for eta in ETAS]
        title = label("同样的损失，不同的路", 39).move_to([0, 3.38, 0])
        premise = math(r"L(w)=(w-2)^2,\quad w_0=0,\quad e_k=w_k-2", 29).move_to([0, 2.67, 0])
        rule = math(r"\Delta w_k=-2\eta e_k\qquad e_{k+1}=(1-2\eta)e_k", 34).move_to([0, 1.75, 0])
        words = label("学习率改变一步迈多远，也改变下一步在最优点的哪一侧。", 25).move_to([0, .7, 0])
        intro = VGroup(title, premise, rule, words)
        self.play(FadeIn(intro), run_time=1)
        self.wait(4)
        self.play(FadeOut(words), rule.animate.move_to([0, 2.05, 0]), run_time=.7)
        self.play(FadeOut(intro), run_time=.5)

        title = label("同一标尺，看清方向与距离", 34).move_to([0, 3.53, 0])
        premise = math(r"L(w)=(w-2)^2,\quad w_0=0,\quad e_k=w_k-2", 23).move_to([0, 2.98, 0])
        rule = math(r"e_{k+1}=(1-2\eta)e_k\quad |\Delta w_k|=2\eta|e_k|", 28).move_to([0, 2.46, 0])
        self.add(title, premise, rule)
        ys = [1.25, -.23, -1.71]
        dots, coefficients, readouts = [], [], []
        backgrounds = VGroup()
        for i, (eta, y, color) in enumerate(zip(ETAS, ys, COLORS)):
            panel = RoundedRectangle(width=13.35, height=1.39, corner_radius=.12,
                                     stroke_color="#DCE3E9", stroke_width=1,
                                     fill_color=WHITE, fill_opacity=1).move_to([0, y-.16, 0])
            backgrounds.add(panel)
            name = math(rf"\eta={eta}", 29, color).move_to([-5.6, y+.22, 0])
            coefficient = math(rf"1-2\eta={fmt(1-2*eta)}", 22, color).move_to([-5.35, y-.25, 0])
            coefficients.append(coefficient)
            axis = Line([axis_x(AXIS_MIN), y, 0], [axis_x(AXIS_MAX), y, 0], color="#B7C2CC", stroke_width=1.4)
            axis_marks = VGroup()
            for w in (-2, 0, 2, 4, 6):
                axis_marks.add(Line([axis_x(w), y-.05, 0], [axis_x(w), y+.05, 0], color=MUTED, stroke_width=1))
                axis_marks.add(label(str(w), 13, MUTED).move_to([axis_x(w), y-.18, 0]))
            dot = Dot([axis_x(0), y, 0], radius=.075, color=color).set_z_index(5)
            ghost = Circle(radius=.08, color=color, stroke_width=2).move_to(dot).set_opacity(.4)
            dots.append(dot)
            readout = label("真实端点 k=0    w=0    e=−2    L=4", 19, color).move_to([.95, y-.53, 0])
            readouts.append(readout)
            self.add(panel, name, coefficient, axis, axis_marks, ghost, dot, readout)
        optimum = DashedLine([axis_x(2), 1.78, 0], [axis_x(2), -2.05, 0], color=MUTED, stroke_width=1.5).set_z_index(2)
        optimum_label = label("最优点 w=2", 17, MUTED).move_to([axis_x(2), 1.97, 0])
        legend = label("圆环：真实端点  实心点：视觉过渡  箭头：完整更新  下方线段：端点距离", 18, MUTED).move_to([0, -2.83, 0])
        caption = label("从同一个起点出发。先看第一步。", 26).move_to([0, -3.4, 0])
        self.add(optimum, optimum_label, legend, caption)
        self.wait(2)

        def say(text):
            nonlocal caption
            new = label(text, 25).move_to([0, -3.4, 0])
            self.remove(caption)
            caption = new
            self.add(caption)

        for k in range(STEPS):
            arrows, distances = [], []
            for i, (eta, y, color) in enumerate(zip(ETAS, ys, COLORS)):
                a, b = rows[i][k], rows[i][k+1]
                arrow = Arrow([axis_x(a['w']), y+.22, 0], [axis_x(b['w']), y+.22, 0],
                              buff=0, color=color, stroke_width=3, tip_length=.11,
                              max_tip_length_to_length_ratio=.18)
                arrows.append(arrow)
                # This line's geometric endpoints, not the dot diameter, encode distance.
                distance = Line([axis_x(2), y-.33, 0], [axis_x(a['w']), y-.33, 0], color=color, stroke_width=4)
                distances.append(distance)
                new = label(f"k={k} → {k+1}   w: {fmt(a['w'])} → {fmt(b['w'])}   Δw={fmt(a['delta'])}   步长={fmt(a['length'])}", 18, color).move_to([.95, y-.53, 0])
                self.remove(readouts[i]); readouts[i] = new; self.add(new)
            say(f"第 {k+1} 次更新：箭头的方向与长度由 Δw=−2ηe 决定。")
            self.play(*[GrowArrow(a) for a in arrows], *[Create(d) for d in distances], run_time=.6)
            self.wait(1.4 if k == 0 else .6)
            # All lanes share transition progress. Pause exactly at each crossing.
            milestones = [(1/(2*ETAS[2]), 2), (1/(2*ETAS[1]), 1), (1.0, None)]
            previous = 0.0
            for progress, crossing in milestones:
                targets = []
                for i, y in enumerate(ys):
                    a, b = rows[i][k], rows[i][k+1]
                    w = a['w'] + (b['w']-a['w']) * progress
                    targets.append(dots[i].animate.move_to([axis_x(w), y, 0]))
                self.play(*targets, run_time=(progress-previous)*(4.0 if k == 0 else 2.4), rate_func=linear)
                previous = progress
                event = dict(time=float(self.time), k=k, progress=progress,
                             crossing_eta=None if crossing is None else ETAS[crossing],
                             dots=[d.get_center().tolist() for d in dots],
                             arrows=[dict(start=a.get_start().tolist(), end=a.get_end().tolist()) for a in arrows])
                self.events.append(event)
                if crossing is not None:
                    sign_from = "负" if rows[crossing][k]['e'] < 0 else "正"
                    sign_to = "正" if sign_from == "负" else "负"
                    say(f"η={ETAS[crossing]} 穿过 2：下一端点误差由{sign_from}变{sign_to}，对应负的乘数。")
                    box = SurroundingRectangle(coefficients[crossing], color=COLORS[crossing], buff=.1, stroke_width=2)
                    ring = Circle(radius=.17, color=COLORS[crossing], stroke_width=2).move_to(dots[crossing])
                    self.add(box, ring)
                    self.wait(2.4 if k == 0 else .9)
                    self.remove(box, ring)
                    say("停在 2 只是过渡画面；本次计算直接从起点更新到箭头终点。")
            # Commit the real iteration only after every moving point has arrived.
            for i, (y, color) in enumerate(zip(ys, COLORS)):
                a, b = rows[i][k], rows[i][k+1]
                self.add(Circle(radius=.08, color=color, stroke_width=2).move_to(dots[i]).set_opacity(.4))
                self.remove(readouts[i])
                readouts[i] = label(f"真实端点 k={k+1}   w={fmt(b['w'])}   e={fmt(b['e'])}   L={fmt(b['loss'])}", 18, color).move_to([.95, y-.53, 0])
                self.add(readouts[i])
            say("到达真实端点：蓝、橙距离都乘 0.6；红色距离乘 1.2。")
            for i, (d, y) in enumerate(zip(distances, ys)):
                d.put_start_and_end_on([axis_x(2), y-.33, 0], [axis_x(rows[i][k+1]['w']), y-.33, 0])
            self.play(*[Indicate(c, scale_factor=1.08) for c in coefficients], run_time=.7)
            self.events.append(dict(time=float(self.time), k=k+1, endpoint=True,
                                    distances=[dict(start=d.get_start().tolist(), end=d.get_end().tolist()) for d in distances],
                                    rows=[r[k+1] for r in rows], readouts=[r.text for r in readouts]))
            self.wait(2 if k == 0 else 1.0)
            self.play(*[FadeOut(a) for a in arrows], *[FadeOut(d) for d in distances], run_time=.3)

        say("η=0.2 同侧接近；η=0.8 交替接近；η=1.1 交替远离。")
        self.wait(3)
        self.play(*[FadeOut(m) for m in list(self.mobjects)], run_time=.7)
        summary = VGroup(
            label("符号决定换不换边，绝对值决定近还是远。", 33),
            math(r"e_{k+1}=\underbrace{(1-2\eta)}_{\text{multiplier}}e_k", 40),
            label("0.6：同侧、缩短      −0.6：换边、缩短      −1.2：换边、放大", 25),
            label("η=0.2 与 0.8 的距离、损失相同，带符号的位置不同。", 26),
            label("结论限于 L(w)=(w−2)²；运动插值不是额外的梯度迭代。", 21, MUTED),
        ).arrange(DOWN, buff=.5).move_to(ORIGIN)
        self.play(FadeIn(summary), run_time=.8)
        self.wait(5)
        path = Path(os.environ.get("EX9A_EVIDENCE", "scene-evidence.json"))
        path.write_text(json.dumps(dict(duration=float(self.time), fps=config.frame_rate,
                                       resolution=[config.pixel_width, config.pixel_height],
                                       events=self.events), ensure_ascii=False, indent=2), encoding="utf-8")

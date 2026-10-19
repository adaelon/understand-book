# MIT License
# 
# Copyright (c) 2026 Deeki-Sora
# 
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
# 
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
# 
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""切线候选站位与短时追击估算；所有长度使用同一自选单位。"""
from dataclasses import dataclass
from math import hypot, sqrt, inf, isfinite
from typing import Optional

EPS = 1e-12
# 默认值用于粗略估算。反应参考公开点击测试均值，前摇参考历史普通普攻样本。
DEFAULT_REACTION = 0.28
DEFAULT_WINDUP = 0.30
DEFAULT_WINDUP_RATIO = 0.20


def reference_windup(attack_speed=None):
    if attack_speed is None: return DEFAULT_WINDUP
    if not isfinite(attack_speed) or attack_speed <= 0:
        raise ValueError("攻速需要大于0。")
    return DEFAULT_WINDUP_RATIO / attack_speed


def nonnegative(name, value):
    if not isfinite(value) or value < 0:
        raise ValueError(f"{name}必须是有限的非负数。")


@dataclass(frozen=True)
class V:
    x: float
    y: float
    def __add__(self, o): return V(self.x + o.x, self.y + o.y)
    def __sub__(self, o): return V(self.x - o.x, self.y - o.y)
    def __mul__(self, k): return V(self.x * k, self.y * k)
    __rmul__ = __mul__
    def dot(self, o): return self.x * o.x + self.y * o.y
    def norm(self): return hypot(self.x, self.y)
    def unit(self):
        n = self.norm()
        if n <= EPS: raise ValueError("敌方英雄与己方关键小兵不能重合。")
        return V(self.x / n, self.y / n)
    def dist(self, o): return (self - o).norm()


@dataclass(frozen=True)
class I:
    lo: float
    hi: float
    @property
    def empty(self): return self.lo > self.hi
    def inter(self, o): return I(max(self.lo, o.lo), min(self.hi, o.hi))
    def clamp(self, x): return min(max(x, self.lo), self.hi)


def sideI(b):
    if b > EPS: return I(0, inf)
    if b < -EPS: return I(-inf, 0)
    return I(-inf, inf)


def reachI(a, b, r):
    if abs(a) > r: return None
    h = sqrt(max(0, (r - abs(a)) * (r + abs(a))))
    return I(b - h, b + h)


def fmtI(z):
    if z is None or z.empty: return "无"
    def f(x): return "+∞" if x == inf else "-∞" if x == -inf else f"{x:.3f}"
    return f"[{f(z.lo)}, {f(z.hi)}]"


@dataclass(frozen=True)
class Skill:
    """targeted: 中心有效距离；circle: 圆心距离+半径；line: 长度+全宽。"""
    shape: str
    range: float
    size: float

    def __post_init__(self):
        if self.shape not in ("targeted", "circle", "line"):
            raise ValueError("技能类型必须是 targeted（指向性）、circle（圆形）或 line（直线）。")
        nonnegative("技能距离", self.range)
        nonnegative("圆形作用半径／直线全宽", self.size)

    def target_reach(self, target_radius):
        # 直接瞄准静止目标；直线全宽不能加到纵向射程。
        if self.shape == "targeted": return self.range
        return self.range + target_radius + (self.size if self.shape == "circle" else 0)

    def aimed_at_q_hits(self, enemy, q, point, target_radius):
        if self.shape == "targeted": return enemy.dist(point) <= self.range
        if self.shape == "circle":
            if enemy.dist(q) > self.range: return False
            return point.dist(q) <= self.size + target_radius
        if enemy.dist(q) <= EPS: return None  # 零构造半径无法定义E→Q瞄准方向。
        direction = (q - enemy).unit()
        normal = V(-direction.y, direction.x)
        local = point - enemy
        along, across = local.dot(direction), abs(local.dot(normal))
        dx = max(-along, 0, along - self.range)
        dy = max(across - self.size / 2, 0)
        return hypot(dx, dy) <= target_radius


@dataclass
class Geo:
    E: V; F: V; T: V
    enemy_aa: float; self_aa: float; self_cast: float
    enemy_cast: Optional[float]
    u: V; n: V; Q: V; a: float; b: float
    aaI: Optional[I]; castI: Optional[I]; sideI: I; legalI: I
    skill_range: Optional[float]; skillI: Optional[I]; skillSafeI: Optional[I]
    safety_margin: float = 0
    self_radius: float = 0
    target_radius: float = 0
    own_skill: Optional[Skill] = None
    enemy_skill: Optional[Skill] = None
    aa_mode: str = "effective"
    pressure_range: Optional[float] = None

    @property
    def safety_radius(self): return self.enemy_aa + self.safety_margin
    def point(self, s): return self.Q + self.n * s
    @property
    def safeAaI(self):
        """放弃施压后的普攻区间，仍保留所设安全边界与目标侧约束。"""
        return I(1, 0) if self.aaI is None else self.aaI.inter(self.sideI)
    @property
    def skillPressureI(self):
        return I(1, 0) if self.skillSafeI is None or self.castI is None else self.skillSafeI.inter(self.castI)
    @property
    def recommendedI(self):
        if not self.legalI.empty: return self.legalI
        if not self.safeAaI.empty: return self.safeAaI
        return I(1, 0) if self.skillSafeI is None else self.skillSafeI
    def stagingI(self):
        # 短时补刀先选边界线上最近的等待点，不强求仍可施压。
        return self.sideI
    def staging_point(self):
        z = self.stagingI()
        return None if z.empty else self.point(z.clamp(self.b))
    def aggressive_point(self):
        z = self.recommendedI
        return None if z.empty else self.point(z.clamp(self.b))
    def midpoint(self):
        z = self.recommendedI
        return None if z.empty else self.point((z.lo + z.hi) / 2)
    def conservative_point(self):
        z = self.recommendedI
        return None if z.empty else self.point(max((z.lo, z.hi), key=abs))


def solve_geometry(enemy, own_minion, target_minion, enemy_aa, self_aa, self_cast,
                   enemy_cast=None, skill_range=None, *, safety_margin=0,
                   aa_mode="effective", enemy_radius=0, self_radius=0,
                   target_radius=0, own_skill=None, enemy_skill=None, pressure_range=None):
    """effective: 普攻已换算到中心；edge: 普攻射程加双方玩法半径。
    self_cast/enemy_cast 是全部可用技能的最大施法范围（统一中心口径）。
    pressure_range 是具体消耗技能的中心命中距离，与最大施法范围独立。
    skill_range 是旧调用的补刀中心有效距离，不能与 own_skill 同时传入。
    """
    for name, value in (("敌方普攻距离", enemy_aa), ("己方普攻距离", self_aa),
                        ("己方最大施法范围", self_cast), ("安全裕量", safety_margin),
                        ("敌方玩法半径", enemy_radius), ("己方玩法半径", self_radius),
                        ("目标兵玩法半径", target_radius)):
        nonnegative(name, value)
    for name, value in (("敌方最大施法范围", enemy_cast), ("补刀有效距离", skill_range),
                        ("消耗技能有效距离", pressure_range)):
        if value is not None: nonnegative(name, value)
    if aa_mode not in ("effective", "edge"): raise ValueError("普攻口径必须是 effective 或 edge。")
    if skill_range is not None and own_skill is not None:
        raise ValueError("补刀有效距离与技能形状参数只能选一种，避免重复扩大距离。")
    E, F, T = (V(*map(float, p)) for p in (enemy, own_minion, target_minion))
    if not all(isfinite(c) for p in (E, F, T) for c in (p.x, p.y)):
        raise ValueError("坐标必须是有限数字。")
    if aa_mode == "edge":
        enemy_aa += enemy_radius + self_radius
        self_aa += self_radius + target_radius
    rho = enemy_aa + safety_margin
    for name, value in (("有效敌方普攻距离", enemy_aa), ("有效己方普攻距离", self_aa),
                        ("构造切线半径", rho)):
        nonnegative(name, value)
    u = (F - E).unit(); n = V(-u.y, u.x); Q = E + rho * u
    d = T - Q; a = d.dot(u); b = d.dot(n)
    side = sideI(b); aa = reachI(a, b, self_aa); cast = reachI(rho, 0, self_cast)
    legal = I(1, 0) if aa is None or cast is None else aa.inter(cast).inter(side)
    if own_skill is not None:
        skill_range = own_skill.target_reach(target_radius)
        nonnegative("技能有效补兵距离", skill_range)
    sk = None if skill_range is None else reachI(a, b, skill_range)
    sk_safe = None if sk is None else sk.inter(side)
    return Geo(E, F, T, enemy_aa, self_aa, self_cast, enemy_cast, u, n, Q, a, b,
               aa, cast, side, legal, skill_range, sk, sk_safe, safety_margin,
               self_radius, target_radius, own_skill, enemy_skill, aa_mode, pressure_range)


def move_toward(p, target, speed, dt):
    d = target - p; length = d.norm()
    if length <= EPS: return p
    step = speed * dt
    return target if step >= length else p + d * (step / length)


def move_away(p, threat, speed, dt):
    d = p - threat; length = d.norm()
    return p if length <= EPS else p + d * (speed * dt / length)


def minimum_hit(staging, target, aa):
    distance = staging.dist(target)
    if distance <= aa: return staging, 0.0
    entry = distance - aa
    return staging + (target - staging).unit() * entry, entry


@dataclass
class Sim:
    ok: bool; reason: str
    staging: Optional[V] = None; hit: Optional[V] = None
    entry_dist: Optional[float] = None; entry_time: Optional[float] = None
    period: Optional[float] = None; returned: Optional[bool] = None
    return_time: Optional[float] = None; dist_ready: Optional[float] = None
    margin: Optional[float] = None; classification: Optional[str] = None
    attack_completed: bool = False
    attack_time: Optional[float] = None
    window_valid: bool = False
    ready_time: Optional[float] = None
    danger_margin: Optional[float] = None
    distance_tolerance: Optional[float] = None
    reaction_time: Optional[float] = None
    windup_time: Optional[float] = None


def simulate_window(g, self_ms, enemy_ms, enemy_as=None, reaction=DEFAULT_REACTION, windup=DEFAULT_WINDUP,
                    dt=0.002, *, enemy_windup=None, remaining_cooldown=None):
    """t=0 为敌方攻击出手；输入实测剩余冷却或 1/攻速-敌方前摇。
    己方攻击假设已就绪；完成前摇只证明攻击出手，不证明击杀小兵。
    己方阶段时刻精确拆分，敌方采用离散纯追击近似。
    """
    if reaction is None: reaction = DEFAULT_REACTION
    if windup is None: windup = DEFAULT_WINDUP
    values = (self_ms, enemy_ms, reaction, windup, dt)
    if self_ms == 0:
        return Sim(False, "没有我方移速，暂时算不了上前和回撤的时间；请补充移速。")
    if not all(isfinite(v) and v >= 0 for v in values) or self_ms <= 0 or dt <= 0:
        return Sim(False, "己方移速与步长须大于0；敌方移速和时间须有限且非负。")
    period = None
    if remaining_cooldown is not None:
        if not isfinite(remaining_cooldown) or remaining_cooldown <= 0:
            return Sim(False, "出手后剩余冷却必须是有限正数。")
        ready = remaining_cooldown
    else:
        if enemy_as is None or not isfinite(enemy_as) or enemy_as <= 0:
            return Sim(False, "请输入有效敌方攻速，或直接估计出手后剩余冷却。")
        if enemy_windup is None: enemy_windup = reference_windup(enemy_as)
        if not isfinite(enemy_windup) or enemy_windup < 0:
            return Sim(False, "敌方普攻前摇需要是不小于0的秒数。")
        period = 1 / enemy_as
        ready = period - enemy_windup
        if ready <= 0: return Sim(False, "敌方前摇须小于完整攻击周期。")
    if ready / dt > 2_000_000:
        return Sim(False, "输入不属于短时窗口，或步长过小；请缩短窗口或增大步长。")
    st = g.staging_point()
    if st is None: return Sim(False, "找不到这次补刀路线的等待位置。")
    hit, entry = minimum_hit(st, g.T, g.self_aa)
    entry_time = entry / self_ms
    entry_end = reaction + entry_time
    attack_time = entry_end + windup
    return_time = attack_time + entry_time
    A, B, t = st, g.E, 0.0
    def own_position(time):
        if time <= reaction: return st
        if time < entry_end: return move_toward(st, hit, self_ms, time - reaction)
        if time <= attack_time: return hit
        if time < return_time: return move_toward(hit, st, self_ms, time - attack_time)
        return st
    while t < ready:
        boundaries = [x for x in (reaction, entry_end, attack_time, return_time) if x > t]
        end = min(t + dt, ready, *boundaries)
        step = end - t
        next_B = move_toward(B, A, enemy_ms, step)
        A = move_away(A, B, self_ms, step) if t >= return_time else own_position(end)
        B = next_B; t = end
    distance = A.dist(B); danger_margin = distance - g.enemy_aa
    margin = distance - g.safety_radius
    # 相对容差随长度缩放，步长位移量用于标记临界结果，不是严格全局误差界。
    tolerance = max(g.safety_radius, g.self_aa, st.dist(g.T), (self_ms + enemy_ms) * ready) * 1e-9
    tolerance += (self_ms + enemy_ms) * min(dt, ready)
    completed = attack_time <= ready
    returned = return_time <= ready
    valid = completed and returned and margin > tolerance
    if not completed: classification = "来不及：对面下一次普攻转好时，你还没A出这一刀"
    elif not returned: classification = "来不及：A出了这一刀，但还没退回等待位置"
    elif margin < -tolerance: classification = "不够安全：虽然退回去了，但对面也追上来了"
    elif margin <= tolerance: classification = "时间很紧：结果接近边界，建议留更多余量"
    else: classification = "可以尝试：来得及A出这一刀并退回，距离余量也够"
    return Sim(True, "完成估算", st, hit, entry, entry_time, period, returned,
               return_time if returned else None, distance, margin, classification,
               completed, attack_time if completed else None, valid, ready,
               danger_margin, tolerance, reaction, windup)



if __name__ == "__main__":
    g = solve_geometry(
        enemy=(0, 0), own_minion=(4, 0), target_minion=(3, 10),
        enemy_aa=6, self_aa=2, self_cast=8, skill_range=5,
        safety_margin=0.5, aa_mode="effective",
    )
    sim = simulate_window(
        g, self_ms=5, enemy_ms=3, reaction=0.2, windup=0.2,
        remaining_cooldown=1.2, dt=0.002,
    )
    assert sim.ok and sim.window_valid
    assert abs(sim.attack_time - 0.7) < 1e-9
    assert abs(sim.return_time - 1.0) < 1e-9
    print("W1:", round(sim.dist_ready, 6), sim.window_valid)

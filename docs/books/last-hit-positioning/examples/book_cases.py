"""本书的练习参数与可复算结果。运行只需要 Python 3 标准库。"""
from dataclasses import asdict
from pathlib import Path
import argparse
import json
import math
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "upstream"))
from last_hit_positioning_v3 import (Skill, V, solve_geometry, simulate_window,
                                     minimum_hit, move_toward, move_away)

BASE = dict(enemy=(0, 0), own_minion=(4, 0), enemy_aa=6,
            self_aa=3, self_cast=8, safety_margin=0.5, aa_mode="effective")

STATIC = {
    "G1": dict(target_minion=(6, 4), pressure_range=7),
    "G2": dict(target_minion=(6, 9), pressure_range=7),
    "G3": dict(target_minion=(2, 4), skill_range=5, pressure_range=5),
    "G4": dict(target_minion=(0, 2), skill_range=5, pressure_range=5),
}

WINDOW = {
    "W1": dict(target_minion=(3, 10), self_ms=5, enemy_ms=3,
               reaction=0.2, windup=0.2, remaining_cooldown=1.2),
    "W2": dict(target_minion=(3, 5), self_ms=5, enemy_ms=3,
               reaction=0.2, windup=0.2, remaining_cooldown=1.2),
    "W3": dict(target_minion=(3, 10), self_ms=2.5, enemy_ms=3,
               reaction=0.2, windup=0.2, remaining_cooldown=1.2),
    "W4": dict(target_minion=(3, 10), self_ms=2, enemy_ms=3,
               reaction=0.4, windup=0.3, remaining_cooldown=1.2),
}


def interval(z):
    return None if z is None or z.empty else [z.lo, z.hi]


def point(p):
    return None if p is None else [p.x, p.y]


def geometry(case_id):
    return solve_geometry(**(BASE | STATIC[case_id]))


def window(case_id, dt=0.002):
    values = WINDOW[case_id].copy()
    target = values.pop("target_minion")
    g = solve_geometry(**(BASE | dict(target_minion=target, self_aa=2, skill_range=5)))
    return g, simulate_window(g, **values, dt=dt)


def window_trace(case_id, dt=0.002):
    """按上游相同的分段更新记录轨迹，供静态插图使用。"""
    g, sim = window(case_id, dt)
    params = WINDOW[case_id]
    st = sim.staging
    hit, entry = minimum_hit(st, g.T, g.self_aa)
    entry_end = params["reaction"] + entry / params["self_ms"]
    attack_time = entry_end + params["windup"]
    return_time = attack_time + entry / params["self_ms"]
    a, b, t = st, g.E, 0.0
    trace = [(t, a.x, a.y, b.x, b.y)]
    while t < sim.ready_time:
        boundaries = [v for v in (params["reaction"], entry_end, attack_time, return_time) if v > t]
        end = min(t + dt, sim.ready_time, *boundaries)
        step = end - t
        next_b = move_toward(b, a, params["enemy_ms"], step)
        if t >= return_time:
            a = move_away(a, b, params["self_ms"], step)
        elif end <= params["reaction"]:
            a = st
        elif end < entry_end:
            a = move_toward(st, hit, params["self_ms"], end - params["reaction"])
        elif end <= attack_time:
            a = hit
        elif end < return_time:
            a = move_toward(hit, st, params["self_ms"], end - attack_time)
        else:
            a = st
        b, t = next_b, end
        trace.append((t, a.x, a.y, b.x, b.y))
    if not math.isclose(a.dist(b), sim.dist_ready, abs_tol=1e-10):
        raise AssertionError("配图轨迹终点与上游模拟不一致")
    return trace


def collect():
    output = {"units": {"length": "练习格（中心有效距离）", "speed": "格/秒", "time": "秒"},
              "source_commit": "50db387dd1218f5a0578183bdfd031cdb9375573",
              "static": {}, "window": {}}
    for name in STATIC:
        g = geometry(name)
        output["static"][name] = {
            "input": BASE | STATIC[name], "Q": point(g.Q),
            "aa_interval": interval(g.safeAaI), "cast_interval": interval(g.castI),
            "combined_interval": interval(g.legalI),
            "skill_interval": interval(g.skillSafeI),
            "near": point(g.aggressive_point()), "middle": point(g.midpoint()),
            "far": point(g.conservative_point()),
        }
    g = geometry("G1")
    output["points"] = {
        name: {"position": [x, y], "enemy_distance": V(x, y).dist(g.E),
               "minion_distance": V(x, y).dist(g.T)}
        for name, x, y in (("A", 6.5, 1), ("B", 6.5, 4), ("C", 6.5, 7),
                           ("D", 3, 4), ("P", 3, 6.2))
    }
    for name in WINDOW:
        g, sim = window(name)
        values = asdict(sim)
        initial = sim.staging.dist(g.E)
        params = WINDOW[name]
        full_return = params["reaction"] + 2 * sim.entry_dist / params["self_ms"] + params["windup"]
        bound = None
        if sim.returned:
            bound = initial - params["enemy_ms"] * full_return + (
                params["self_ms"] - params["enemy_ms"]) * (sim.ready_time - full_return)
        output["window"][name] = dict(input=BASE | dict(self_aa=2, skill_range=5) | params | {"dt": 0.002},
                                        result=values, initial_distance=initial,
                                        planned_return_time=full_return,
                                        conservative_end_distance=bound)
    output["skill_example"] = {
        "circle": {"cast_center": 4, "radius": 1.5, "target_radius": 0.5,
                   "effective_reach": Skill("circle", 4, 1.5).target_reach(0.5)},
        "line": {"length": 4, "full_width": 2, "target_radius": 0.5,
                 "longitudinal_reach": Skill("line", 4, 2).target_reach(0.5)},
    }
    short = solve_geometry(**(BASE | dict(target_minion=(6, 10), self_aa=2, skill_range=5)))
    output["short_static"] = {"input": BASE | dict(target_minion=(6, 10), self_aa=2, skill_range=5),
                              "aa_interval": interval(short.safeAaI),
                              "wait": point(short.staging_point()),
                              "target_distance_at_wait": short.staging_point().dist(short.T)}
    return output


def verify(data):
    """用手算端点、直接距离和时间账核对本书依赖的结论。"""
    checks = []

    def require(ok, label):
        if not ok:
            raise AssertionError(label)
        checks.append(label)

    g = geometry("G1")
    require(math.isclose(g.safeAaI.lo, 4 - math.sqrt(8.75)) and
            math.isclose(g.safeAaI.hi, 4 + math.sqrt(8.75)), "G1 普攻端点与勾股手算一致")
    require(math.isclose(g.legalI.hi, math.sqrt(21.75)), "G1 施法参考截断位置一致")
    require(V(6.5, 4).dist(g.T) == 0.5 and V(3, 4).dist(g.E) == 5, "B 能补刀与 D 在普攻范围内")
    require(V(6.5, 1).dist(g.T) > 3 and V(6.5, 7).dist(g.T) > 3, "A 和 C 均未进入补兵范围")
    g2 = geometry("G2")
    require(g2.legalI.empty and not g2.safeAaI.empty, "G2 保留普攻但超出最大施法参考范围")
    g3 = geometry("G3")
    require(g3.safeAaI.empty and not g3.skillSafeI.empty, "G3 候选线上的技能方案存在")
    require(V(3, 6.2).dist(g3.E) > 6.5 and V(3, 6.2).dist(g3.T) < 3,
            "G3 线外 P 点反例证明切线上无解不代表平面无解")
    require(geometry("G4").recommendedI.empty, "G4 当前候选线上没有直接方案")
    w1 = data["window"]["W1"]
    require(math.isclose(w1["planned_return_time"], 1.0) and
            math.isclose(w1["result"]["attack_time"], 0.7), "W1 手算出手与返回时刻一致")
    require(w1["result"]["window_valid"] and w1["conservative_end_distance"] > 6.5,
            "W1 数值结论有保守距离下界支持")
    w2 = data["window"]["W2"]
    require(w2["result"]["returned"] and w2["result"]["dist_ready"] < 6,
            "W2 虽已返回但对手下一次普攻就绪时已够得到")
    require(data["window"]["W3"]["result"]["attack_completed"] and
            not data["window"]["W3"]["result"]["returned"], "W3 出手了但没返回")
    require(not data["window"]["W4"]["result"]["attack_completed"], "W4 尚未出手")
    require(data["skill_example"]["circle"]["effective_reach"] == 6 and
            data["skill_example"]["line"]["longitudinal_reach"] == 4.5, "技能作用范围与全宽口径一致")
    require(data["short_static"]["target_distance_at_wait"] == 0.5 and
            data["short_static"]["aa_interval"] is not None, "第六章短手静态补刀场景成立")
    for case_id in ("W1", "W2"):
        window_trace(case_id)
        checks.append(case_id + " 配图轨迹终点与模拟一致")
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    data = collect()
    data["verified_claims"] = verify(data)
    body = json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    if args.output:
        args.output.write_text(body, encoding="utf-8")
        print(f"已核对 {len(data['verified_claims'])} 项书中结论；结果写入 {args.output}")
    else:
        print(body)


if __name__ == "__main__":
    main()

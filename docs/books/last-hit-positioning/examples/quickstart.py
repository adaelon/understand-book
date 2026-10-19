from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "upstream"))
from last_hit_positioning_v3 import solve_geometry, simulate_window

g = solve_geometry(
    enemy=(0, 0),
    own_minion=(4, 0),
    target_minion=(3, 10),
    enemy_aa=6,
    self_aa=2,
    self_cast=8,
    skill_range=5,
    safety_margin=0.5,
    aa_mode="effective",
)
result = simulate_window(
    g,
    self_ms=5,
    enemy_ms=3,
    reaction=0.2,
    windup=0.2,
    remaining_cooldown=1.2,
    dt=0.002,
)
print("普攻候选为空：", g.safeAaI.empty)
print("计算完成：", result.ok)
print("出手时刻：", result.attack_time)
print("已完成返回：", result.returned)
print("截止距离：", round(result.dist_ready, 3))
print("本计划可行：", result.window_valid)

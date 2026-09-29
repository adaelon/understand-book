"""Freeze inputs and implementation before a separately accounted EX11.7 batch."""
import json
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
root = repo / 'docs/performance/ex11-local-demonstrations/ex11.7' / sys.argv[1]
root.mkdir(parents=True, exist_ok=False)
(root / 'inputs').mkdir()
(root / 'runs').mkdir()
learning = '学习率大一点，为什么会来回跳，甚至越来越远？我想自己拖动学习率，马上看见每一步会走到哪里，也能播放、暂停和回到前一步。这里的简化材料是 L(w)=(w-2)^2，更新规则 w_{t+1}=w_t-2η(w_t-2)，初值 w_0=0；只在这个一维模型内解释。请制作并交付可以操作的解释，把同侧接近、交替接近、发散放在可直接比较的关系里。'
mechanism = '我看不懂“结合之后才开门，随后才通过”这三个动作之间的关系。请制作并交付一个以正文为主的解释，在对应段落里放一小段可播放、暂停、重播并停在各阶段的预先渲染动画，让我逐步观察。材料：这是人为规定的膜通道教学模型，不代表所有真实膜蛋白。阶段0：门关闭，紫色配体在门外、青色粒子在膜左侧；阶段1：配体移到门上结合，门仍关闭，粒子留在左侧；阶段2：配体保持结合，门打开，粒子还未通过；阶段3：配体和开门状态保持，粒子从左侧穿过通道到右侧。只解释这个顺序，不增加能量、速度或生理功能主张。默认停在首帧，隐藏系统视频进度条，用就近按钮控制，让我能保存中途画面、重开并继续问。'
inputs = [
    ('learning-1', learning, 'learning-rate'),
    ('disk-1', '同样转过一个角度，为什么大圆盘边缘走得更远？请制作并交付可以让我拖动转角和半径的解释。同图比较两个圆盘的转角、对应圆弧和展开后的路程。材料仅为理想圆周运动：θ以弧度计，s=rθ；两盘共用θ，半径r1=1、r2可在1到3之间改变，θ从0到2π。请让我能播放、暂停、回退并保存当前位置，区分角度相同和路程不同。', 'disk'),
    ('mechanism-1', mechanism, 'mechanism'),
    ('system-1', '缓存命中和未命中，为什么会改变总等待时间？请制作并交付一页可操作解释，两处就近演示各有自己的位置并能同时保存、重开和追问。材料：简化串行系统，固定未命中事件依次为请求到缓存、缓存查找结束且未命中、请求到后端、后端返回结果、缓存写入并回复用户；请用一小段预先渲染的局部动画观察这条固定流程。另一处让我连续改命中率h，立即比较平均延迟：查缓存2ms，未命中额外访问后端20ms，忽略写入开销与网络波动，因此T(h)=2+(1-h)*20ms，h∈[0,1]。动画位置不代表h；默认静止、可播放暂停和阶段回退。', 'system'),
    ('learning-2', learning, 'learning-rate'),
    ('mechanism-2', mechanism, 'mechanism'),
    ('plain-1', '只用两句话解释：圆周运动的s=rθ里，为什么相同转角下半径越大路程越长？θ用弧度。', 'plain'),
]
for name, message, family in inputs:
    (root / 'inputs' / f'{name}.json').write_text(json.dumps({'id': name, 'family': family, 'message': message}, ensure_ascii=False, indent=2), encoding='utf-8')
for source in ['crates/runtime/src', 'crates/server/src', 'packages/web/src', 'skills/presentation', 'assets/presentation']:
    shutil.copytree(repo / source, root / 'frozen-source' / source)
for source in ['Cargo.toml', 'Cargo.lock', 'scripts/presentation-animation-requirements.txt', 'packages/web/package.json', 'pnpm-lock.yaml']:
    target = root / 'frozen-source' / source
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(repo / source, target)
for filename, args in [('git-status.txt', ['status', '--short']), ('tracked-diff.patch', ['diff', '--binary'])]:
    (root / filename).write_bytes(subprocess.check_output(['git', *args], cwd=repo))
(root / 'frozen.json').write_text(json.dumps({
    'frozen_at': datetime.now().astimezone().isoformat(),
    'head': subprocess.check_output(['git', 'log', '-1', '--oneline'], cwd=repo, text=True).strip(),
    'method': (repo / 'skills/presentation/SKILL.md').read_text(encoding='utf-8').split('Presentation method (')[1].split('):')[0], 'model': 'deepseek-v4-flash', 'provider': 'native', 'temperature': 0,
    'libraries': {'konva': '10.7.0', 'manim': '0.21.0', 'renderer': 'Cairo', 'fps': 30},
    'run_order': [x[0] for x in inputs], 'generation_count': 6, 'plain_count': 1,
    'entry': 'server::tests::presentation_author_tests::ex11::ex11_agent_diagnostic -> prepare_agent_chat -> execute_prepared',
    'limits': 'Production turn limit unchanged. Catalog Flash requests with presentation.author use max_tokens=131072 and matching output reserve; input compaction threshold stays approximately 84000. Other requests retain provider defaults. No added cumulative token ceiling. Render timeout 180s.',
    'provider_note': 'The configured deepseek-v4-flash alias is retained; official documentation maps it to current Flash service, so the served weights are not an immutable snapshot. https://api-docs.deepseek.com/quick_start/pricing/',
    'acceptance': {
        'all': 'Untouched drafts; actual method/schema in requests; final delivery and completed goal; three viewports; real controls; save/reopen/follow-up; errors and raw requests retained.',
        'learning-rate': 'Independent w_k=2-2(1-2η)^k at η=0,.2,.5,.8,1,1.1,1.2; actual geometry, drag hit area, pause/backward/repeat.',
        'disk': 'Independent s=rθ; shared angle, arc/endpoints/unrolled lengths; radius/angle actual drag and resize.',
        'mechanism': 'Actual decoded frames: binding before opening, particle crosses only after opening; paused/repeat/backward seeks; no native controls.',
        'system': 'Fixed five-event order and independent T=22-20h; preserve separate animation position and parameter; multiple demos restore.',
        'plain': 'Two-sentence answer without authoring.',
    },
    'failure_policy': 'Retain failed sample and diagnose. Stop affected subsequent runs; any method/engineering repair requires a new frozen batch.',
    'revision_stability': '../../ex11.2-run3/revision-resize-resume1/source-diff.txt remains the original scope-deviation evidence.',
    'learning_effects': 'Not evaluated; no learner task data.',
}, ensure_ascii=False, indent=2), encoding='utf-8')
print(root)

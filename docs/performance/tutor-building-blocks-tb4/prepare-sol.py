"""Freeze each authorized input once, from actual requests and teaching facts."""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        raise SystemExit(f"Frozen input already exists: {path}")
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


phase = sys.argv[1]
if phase == "author":
    prior = ROOT.parent / "tutor-building-blocks-tb3"
    context = read(prior / "contexts/independent-step/reopen.json")
    source_packet = read(prior / "sol/reopen-repair/independent-step/input.json")
    snapshots = {name: read(ROOT / f"requests/{name}.json") for name in ("Global", "Local", "Review")}
    tools = {tool["name"]: tool for tool in snapshots["Local"]["tools"]}
    tutor = (REPO / "crates/runtime/src/tutor.rs").read_text(encoding="utf-8")
    guidance = {}
    for name in ("COMMON", "SELECT", "UNDERSTANDING"):
        guidance[name] = re.search(rf'const {name}: &str = "(.*?)";', tutor, re.S)[1]
    for name in snapshots:
        guidance[name] = re.search(rf'PresentationPhase::{name} => "(.*?)",', tutor, re.S)[1]
    guidance["presentation_engineering"] = (REPO / "skills/presentation/engineering.md").read_text(encoding="utf-8")
    guidance["page_state"] = (REPO / "skills/presentation/references/state.md").read_text(encoding="utf-8")
    save(ROOT / "sol/author/input.json", {
        "model": "gpt-6.1-sol", "fork_turns": "none", "reasoning": "model default; override omitted",
        "identity": "Scripted request after actual TB3 delivery; no human learning observations.",
        "guidance_revision": "tb4.v1", "guidance": guidance,
        "tools": {name: tools[name] for name in ("presentation.author", "tutor.step")},
        "source": (ROOT.parent / "tutor-building-blocks-tb1/source.txt").read_text(encoding="utf-8"),
        "presented_sources": source_packet["presented_sources"],
        "prior_context": context,
        "last_delivered_answer": (prior / "sol/reopen-repair/independent-step/answer.md").read_text(encoding="utf-8"),
        "user_request": "接着上次那道 Copy 与共享引用的题，给我一个能切换条件、查看对应代码和结果的小教具。我想自己选一组对照来检验 b.0 = 8 的判断；先别直接讲出结论，等我操作后再继续。",
        "output_contract": {
            "framework": "brief whole design", "focus": "relationship and learner activity",
            "write": "one exact presentation.author write argument object, including self-contained html and page-state contract",
            "select": "one exact tutor.step select argument object; omit presentation, which replay binds only after real delivery",
            "answer": "actual final Chinese teaching text; do not claim preview or delivery has happened",
            "preview_actions": "real preview actions to operate the key relationship; do not invent receipts",
        },
        "execution": "Generate actual content, not a plan. This response is captured before local replay. Host will perform real previews, verify code and inspect screenshots before accepting delivery. Do not fabricate learner responses or evidence. Only read this input; write output.json and answer.md beside it. Do not run external model calls. No access to other groups or expected outputs.",
    })
elif phase == "followup":
    current = read(ROOT / "browser/followup-context.json")
    author = read(ROOT / "sol/author/input.json")
    save(ROOT / "sol/followup/input.json", {
        "model": "gpt-6.1-sol", "fork_turns": "none", "reasoning": "model default; override omitted",
        "identity": "Scripted browser interaction and learner expression; no human learning observations.",
        "guidance_revision": "tb4.v1",
        "guidance": {key: value for key, value in author["guidance"].items() if key in ("COMMON", "SELECT", "UNDERSTANDING")},
        "tools": {"tutor.step": author["tools"]["tutor.step"]},
        "source": author["source"], "presented_sources": author["presented_sources"],
        "current": current,
        "output_contract": {"evidence": "exact tutor.step hypothesis arguments only if informative learner words warrant it, otherwise null", "select": "exact tutor.step select args", "answer": "actual Chinese next teaching response"},
        "execution": "Continue this same teaching conversation from its actual saved version, scene, learner words and prior support. Do not author another page. Only read this input; write output.json and answer.md beside it. Preserve exact fact/interpretation IDs; do not invent receipts or learner replies. Do not run external model calls.",
    })
else:
    raise SystemExit("author or followup")

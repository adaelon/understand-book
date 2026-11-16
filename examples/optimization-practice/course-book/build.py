"""Build the combined revision book, exam index, and optional print PDF."""
from pathlib import Path
import argparse
import json
import os
import re
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
WORK = ROOT / "tmp/pdfs/optimization-course-book"
NAME = "最优化方法I-复习讲义与习题解答"


def slug(title):
    return re.sub(r"\s+", "-", re.sub(r"[^\w\s-]", "", title.lower()).strip())


def normalize(text):
    text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
    text = re.sub(r"\$\$(.*?)\$\$|(?<!\$)\$([^$\n]+)\$(?!\$)",
                  lambda m: "\n\n$$\n" + m[1].strip() + "\n$$\n\n" if m[1] is not None else "$" + m[2].strip() + "$",
                  text, flags=re.S)
    lines = []
    for line in text.splitlines():
        if line.startswith("|"):
            line = re.sub(r"\$([^$]+)\$", lambda m: "$" + m[1].replace(r"\|", r"\Vert ").replace("|", r"\vert ") + "$", line)
        lines.append(line.rstrip())
    return re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()


def sections(text, level=3):
    matches = list(re.finditer(r"^" + "#" * level + r" ([^\n]+)\n", text, re.M))
    return {m[1]: text[m.end():matches[i+1].start() if i+1 < len(matches) else len(text)].split("\n## ")[0].strip()
            for i, m in enumerate(matches)}


def question_index(manifest):
    prompts = sections((HERE / "past-exams.md").read_text(encoding="utf-8"))
    answers = sections((HERE / "past-exam-answers.md").read_text(encoding="utf-8"))
    questions = []
    for item in manifest["questions"]:
        q = {k: v for k, v in item.items() if k != "prompt_markdown"}
        q["kind"] = "exam"
        q["source_path"] = manifest["source_path"]
        q["source_title"] = q["year"] + " 学年试题"
        q["chapter_references"] = [dict(chapter=int(ch), part=ref["part"], topic=ref["topic"], exercise=ref["exercise"])
                                   for ch, refs in manifest["chapter_exercises"].items()
                                   for ref in refs if ref["question_id"] == q["id"]]
        prompt = re.sub(r"^\*\*（\d+分）\*\*\s*", "", prompts[q["title"]])
        prompt = re.sub(r"\n\[参考解答\]\([^\n]+\)\s*$", "", prompt)
        answer = re.sub(r"^\[返回题目\]\([^\n]+\)\s*", "", answers[q["title"] + "解答"])
        q["prompt_markdown"] = normalize(prompt)
        q["answer_markdown"] = normalize(answer)
        questions.append(q)
    known = {q["id"] for q in questions}
    for refs in manifest["chapter_exercises"].values():
        for ref in refs:
            if ref["question_id"] not in known:
                raise ValueError(f"Unknown exam question: {ref['question_id']}")
    result = {"title": "最优化方法 I · 历年考试题",
              "source_policy": "课后习题精选历年考题并列出完整题干，标注学年、题号和小问；书末保留完整试题与参考解答。",
              "counts": {"exam": len(questions)}, "questions": questions}
    (HERE / "questions.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return result


def textbook_index():
    sources = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))
    lines = ["# 教材阅读索引", "", "| 章 | 内容 | 教材对应 |", "| ------ | -------------------------------- | ---------------------------------------------------- |"]
    seen = set()
    for source in sources["slides"]:
        if source["number"] not in seen:
            seen.add(source["number"])
            lines.append(f'| {source["number"]} | {source["title"].split("（")[0]} | {source["textbook_sections"]} |')
    lines += ["", "教材节号与《最优化：建模、算法与理论》正文对应。"]
    return "\n".join(lines)


def assemble():
    chapters = sorted((HERE / "chapters").glob("*.md"))
    if len(chapters) != 19:
        raise ValueError("Expected 19 chapters")
    paths = [HERE / "preface.md", HERE / "exam-guide.md", *chapters, HERE / "notes.md"]
    parts = [p.read_text(encoding="utf-8") for p in paths]
    parts += [textbook_index(), (HERE / "past-exams.md").read_text(encoding="utf-8"),
              (HERE / "past-exam-answers.md").read_text(encoding="utf-8")]
    text = "\n\n".join(parts)
    old_ids = {}
    def clean_heading(m):
        title = re.sub(r"\s*\{#[^}]+\}\s*$", "", m[2]).strip()
        ident = re.search(r"\{#([^}]+)\}", m[2])
        if ident:
            old_ids[ident[1]] = slug(title)
        return "#" + m[1] + " " + title
    text = re.sub(r"^(#{1,5}) (.+)$", clean_heading, text, flags=re.M)
    text = re.sub(r"\]\(#([^\)]+)\)", lambda m: "](#" + old_ids.get(m[1], m[1]) + ")", text)
    text = normalize(text)
    if re.search(r"作业|课件|lecture|<!--|<a\b", text, re.I):
        raise ValueError("Retired exercise text or HTML remains in reader output")
    ids = [slug(h) for h in re.findall(r"^#{2,6} (.+)$", text, re.M)]
    if len(ids) != len(set(ids)):
        raise ValueError("Duplicate heading anchors")
    broken = set(re.findall(r"\]\(#([^\)]+)\)", text)) - set(ids)
    if broken:
        raise ValueError(f"Broken internal references: {broken}")
    contents = ["## 目录", ""]
    for title in re.findall(r"^## (.+)$", text, re.M):
        contents.append(f"- [{title}](#{slug(title)})")
    markdown = "# 最优化方法 I：复习讲义与历年考试题\n\n" + "\n".join(contents) + "\n\n" + text + "\n"
    return markdown, text


def build_pdf(body):
    WORK.mkdir(parents=True, exist_ok=True)
    output = ROOT / "output/pdf"
    output.mkdir(parents=True, exist_ok=True)
    pandoc, xelatex = shutil.which("pandoc"), shutil.which("xelatex")
    if not pandoc or not xelatex:
        raise SystemExit("Pandoc and XeLaTeX are required for --pdf.")
    cover = r"""\begin{titlepage}
\thispagestyle{empty}
\vspace*{24mm}
{\Large\sffamily\color{chapterblue} 课程学习参考\par}
\vspace{12mm}
{\fontsize{32}{44}\selectfont\sffamily\bfseries 最优化方法 I\par}
\vspace{8mm}
{\LARGE\sffamily 复习讲义与历年考试题\par}
\vspace{16mm}
{\large 19 章内容讲解 · 按章练习 · 5 套试题与参考解答\par}
\vfill
{\normalsize 教材参考：《最优化：建模、算法与理论》\par}
{\normalsize 刘浩洋、户将、李勇锋、文再文\par}
\vspace{10mm}
{\small 2026 年 11 月\par}
\end{titlepage}
\fancyhead[R]{\small\sffamily 复习讲义与历年考试题}
\pagenumbering{roman}
\setcounter{tocdepth}{1}
\hypersetup{bookmarksdepth=3}
\tableofcontents
\clearpage
\pagenumbering{arabic}
"""
    seen_year_groups = set()
    def pdf_heading(m):
        level, title = m[1], m[2]
        year = re.match(r"20\d\d-20\d\d 学年(试题|解答)$", title)
        new_page = len(level) == 2 or bool(year and year[1] in seen_year_groups)
        if year:
            seen_year_groups.add(year[1])
        pagebreak = "\\clearpage\n\n" if new_page else ""
        if len(level) == 4 and re.match(r"(?:20\d\d-20\d\d 学年第|习题 \d+\.\d+)", title):
            pagebreak += "\\Needspace{8\\baselineskip}\n\n"
        return pagebreak + level[1:] + " " + title + " {#s-" + slug(title) + "}"
    pdf_body = re.sub(r"\]\(#", "](#s-", body)
    pdf_body = re.sub(r"^(#{2,6}) (.+)$", pdf_heading, pdf_body, flags=re.M)
    manuscript = cover + "\n\n" + pdf_body
    source, tex = WORK / "combined.md", WORK / "combined.tex"
    source.write_text(manuscript, encoding="utf-8")
    env = dict(os.environ, max_print_line="1000")
    subprocess.run([
        pandoc, str(source), "--standalone", "--to=latex",
        "--from=markdown+tex_math_dollars+raw_tex", "--wrap=none",
        "-V", "documentclass=ctexart", "-V", "classoption=UTF8,fontset=windows",
        "-V", "fontsize=11pt", "-V", "papersize=a4",
        "-V", "geometry=top=22mm,bottom=23mm,left=22mm,right=22mm",
        "-V", "mainfont=Times New Roman", "-V", "sansfont=Microsoft YaHei",
        "-V", "mathfont=Latin Modern Math", "-V", "colorlinks=true",
        "-V", "linkcolor=chapterblue", "-V", "title=",
        "--include-in-header", str(HERE / "typeset.tex"), "-o", str(tex),
    ], check=True, cwd=WORK, env=env)
    for run in (1, 2, 3):
        result = subprocess.run([xelatex, "-interaction=nonstopmode", "-halt-on-error", "-file-line-error", tex.name],
                                cwd=WORK, env=env, text=True, encoding="utf-8", errors="replace", capture_output=True)
        log = WORK / f"combined-pass-{run}.txt"
        log.write_text(result.stdout + result.stderr, encoding="utf-8")
        if result.returncode:
            raise SystemExit(f"Typesetting failed: {log}")
    dest = output / (NAME + ".pdf")
    shutil.copy2(WORK / "combined.pdf", dest)
    print(dest, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pdf", action="store_true", help="Also build the combined print PDF")
    args = parser.parse_args()
    manifest = json.loads((HERE / "exam-exercises.json").read_text(encoding="utf-8"))
    index = question_index(manifest)
    markdown, body = assemble()
    output = ROOT / "output/markdown"
    output.mkdir(parents=True, exist_ok=True)
    dest = output / (NAME + ".md")
    dest.write_text(markdown, encoding="utf-8")
    print(f"19 chapters; {len(index['questions'])} exam questions and answers; {sum(len(v) for v in manifest['chapter_exercises'].values())} selected chapter exercises.", flush=True)
    print(dest, flush=True)
    if args.pdf:
        build_pdf(body)


if __name__ == "__main__":
    main()

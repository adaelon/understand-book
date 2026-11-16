"""Typeset the two chapter manuscripts with Pandoc and XeLaTeX."""

from pathlib import Path
import os
import shutil
import subprocess


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
WORK = ROOT / "tmp" / "pdfs" / "optimization-duality"
OUTPUT = ROOT / "output" / "pdf"
MANUSCRIPTS = (
    ("chapter", "最优化方法I-对偶理论样章.pdf"),
    ("answers", "最优化方法I-对偶理论习题解答.pdf"),
)


def main():
    pandoc = shutil.which("pandoc")
    xelatex = shutil.which("xelatex")
    if not pandoc or not xelatex:
        raise SystemExit("Typesetting requires Pandoc and XeLaTeX on PATH.")
    WORK.mkdir(parents=True, exist_ok=True)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, max_print_line="1000")
    for stem, filename in MANUSCRIPTS:
        manuscript = (HERE / f"{stem}.md").read_text(encoding="utf-8")
        source = WORK / f"{stem}.md"
        source.write_text(manuscript.replace("<!-- pagebreak -->", r"\newpage"), encoding="utf-8")
        tex = WORK / f"{stem}.tex"
        subprocess.run([
            pandoc, str(source), "--standalone", "--to=latex",
            "--from=markdown+tex_math_dollars+raw_tex", "--wrap=none",
            "-V", "documentclass=ctexart", "-V", "classoption=UTF8,fontset=windows",
            "-V", "fontsize=11pt", "-V", "papersize=a4",
            "-V", "geometry=top=22mm,bottom=23mm,left=22mm,right=22mm",
            "-V", "mainfont=Times New Roman", "-V", "sansfont=Microsoft YaHei",
            "-V", "mathfont=Latin Modern Math", "-V", "colorlinks=true",
            "-V", "linkcolor=chapterblue", "--include-in-header", str(HERE / "typeset.tex"),
            "-o", str(tex),
        ], check=True, cwd=WORK, env=env)
        for run in (1, 2):
            result = subprocess.run([
                xelatex, "-interaction=nonstopmode", "-halt-on-error", "-file-line-error",
                tex.name,
            ], cwd=WORK, env=env, text=True, encoding="utf-8", errors="replace", capture_output=True)
            (WORK / f"{stem}-pass-{run}.txt").write_text(result.stdout + result.stderr, encoding="utf-8")
            if result.returncode:
                raise SystemExit(f"Typesetting failed; see {WORK / f'{stem}-pass-{run}.txt'}")
        shutil.copy2(WORK / f"{stem}.pdf", OUTPUT / filename)
        print(OUTPUT / filename)


if __name__ == "__main__":
    main()

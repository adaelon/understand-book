"""将已完成的书稿、配图和代码装入一个可搬迁的 ZIP。"""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

book = Path(__file__).resolve().parents[1]
destination = book.parents[2] / "output" / "last-hit-positioning-book" / "这一刀怎么补-书稿与代码.zip"
destination.parent.mkdir(parents=True, exist_ok=True)
count = 0
with ZipFile(destination, "w", ZIP_DEFLATED) as archive:
    for source in sorted(book.rglob("*")):
        if source.is_file() and "__pycache__" not in source.parts and source.suffix != ".pyc":
            archive.write(source, Path("这一刀怎么补") / source.relative_to(book))
            count += 1
print(f"Packaged {count} files; {destination.stat().st_size} bytes: {destination}")

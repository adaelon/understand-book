"""Create original document conversion fixtures. No course material is bundled."""
from pathlib import Path
import sys
from pptx import Presentation
from pptx.util import Inches, Pt

out = Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
deck = Presentation()
deck.slide_width, deck.slide_height = Inches(10), Inches(6)

def page(title, body):
    slide = deck.slides.add_slide(deck.slide_layouts[6])
    box = slide.shapes.add_textbox(Inches(.6), Inches(.4), Inches(8.8), Inches(.7))
    box.text_frame.paragraphs[0].text = title
    box.text_frame.paragraphs[0].font.size = Pt(28)
    box = slide.shapes.add_textbox(Inches(.6), Inches(1.4), Inches(8.8), Inches(3.5))
    for index, text in enumerate(body):
        paragraph = box.text_frame.paragraphs[0] if index == 0 else box.text_frame.add_paragraph()
        paragraph.text = text
        paragraph.font.size = Pt(22)
    return slide

page("Original exam preparation fixture", ["This is original test content, not a published course.", "Goal: explain a function and read its value table."])
slide = page("Function and values", ["f(x) = x\u00b2 + 1", "The PDF page must preserve the superscript and table."])
table = slide.shapes.add_table(4, 2, Inches(1), Inches(3.1), Inches(6), Inches(2)).table
for row, values in enumerate([("x", "f(x)"), ("0", "1"), ("1", "2"), ("2", "5")]):
    for column, value in enumerate(values):
        table.cell(row, column).text = value
slide = page("Hidden review slide", ["Hidden slides must retain their original position.", "Check: f(3) = 10."])
slide._element.set("show", "0")
deck.save(out / "original-exam.pptx")
print(out / "original-exam.pptx")

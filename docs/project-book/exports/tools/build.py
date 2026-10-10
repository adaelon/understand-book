"""Build the continuous reading editions; only writes under exports/."""
from pathlib import Path
from urllib.parse import unquote, urlsplit, quote
import copy
import json
import os
import re
import subprocess
import zipfile

HERE = Path(__file__).resolve().parent
OUT = HERE.parent
BOOK = OUT.parent
ROOT = BOOK.parents[1]
PANDOC = os.environ.get('BOOK_PANDOC', r'C:\Program Files (x86)\Pandoc\pandoc.exe')
NODE = os.environ.get('BOOK_NODE', 'node')
TITLE = '深入 Understand Book：从知识构建到阅读 Agent 的架构与实现'
STEM = '深入UnderstandBook-连续阅读版'
READER = 'markdown+tex_math_single_backslash-smart-citations-raw_html'
FILES = sorted((BOOK/'chapters').glob('*.md')) + sorted((BOOK/'appendices').glob('*.md'))
assert len(FILES) == 31
ASSETS = OUT/'assets'
ASSETS.mkdir(exist_ok=True)
WORK = HERE/'work'
WORK.mkdir(exist_ok=True)

def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')

def node(t, c):
    return {'t': t, 'c': c}

def string(s):
    return [node('Str', s)]

def slug(s):
    s = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', s)
    return re.sub(r'[^\w\s-]', '', s.lower()).replace(' ', '-')

docs = []
lookup = {}
for index, path in enumerate(FILES):
    original = path.read_bytes()
    text = original.decode('utf-8-sig')
    ast = json.loads(subprocess.check_output([PANDOC, '-f', READER, '-t', 'json', str(path)]))
    prefix = f'ch{index:02d}' if index < 25 else f'app-{path.name[0].lower()}'
    headers = [b for b in ast['blocks'] if b['t'] == 'Header']
    mapping = {'': prefix}
    seen = {}
    for hindex, header in enumerate(headers):
        old = header['c'][1][0]
        new = prefix if hindex == 0 else f'{prefix}-s{hindex}'
        mapping[old] = new
        header['c'][1][0] = new
    # The original Markdown uses GitHub anchors, which differ from Pandoc IDs.
    unfenced = re.sub(r'^```[^\n]*\n.*?^```\s*$', '', text, flags=re.M|re.S)
    raw_headers = re.findall(r'^#{1,6}\s+(.+?)\s*$', unfenced, re.M)
    assert len(raw_headers) == len(headers), path
    for title, header in zip(raw_headers, headers):
        base = slug(title)
        count = seen.get(base, 0)
        seen[base] = count + 1
        mapping[base if count == 0 else f'{base}-{count}'] = header['c'][1][0]
    doc = dict(path=path, original=original, text=text, ast=ast, prefix=prefix,
               title=raw_headers[0], headers=headers)
    docs.append(doc)
    lookup[path.resolve()] = mapping

references = {}
media = []
code_before = []
counts = dict(chapters=25, appendices=6, diagrams=0, formulas=0, source_links=0, internal_links=0)

def link(destination, origin, epub):
    if urlsplit(destination).scheme:
        return destination
    location, _, fragment = destination.partition('#')
    target = (origin.parent/unquote(location)).resolve() if location else origin.resolve()
    fragment = unquote(fragment)
    assert target.exists(), (origin, destination)
    if target in lookup:
        assert fragment in lookup[target], (origin, destination, fragment)
        if epub:
            counts['internal_links'] += 1
        return '#'+lookup[target][fragment]
    if epub:
        counts['source_links'] += 1
        key = (target, fragment)
        if key not in references:
            references[key] = f'ref-{len(references)+1:03d}'
        return '#'+references[key]
    relative = os.path.relpath(target, OUT).replace('\\', '/')
    return quote(relative, safe='/.-_') + ('#'+quote(fragment) if fragment else '')

def image_inline(name, alt, classes=None):
    return node('Image', [['', classes or [], []], string(alt), [f'assets/{name}.png', '']])

def transform(value, origin):
    if isinstance(value, list):
        return [transform(x, origin) for x in value]
    if not isinstance(value, dict):
        return value
    t, c = value.get('t'), value.get('c')
    if t == 'Link':
        result = copy.deepcopy(value)
        result['c'][2][0] = link(c[2][0], origin, True)
        return result
    if t == 'CodeBlock':
        if 'mermaid' not in c[0][1]:
            code_before.append(c[1])
            return value
        counts['diagrams'] += 1
        name = f'diagram-{counts["diagrams"]:02d}'
        media.append(dict(name=name, kind='mermaid', source=c[1], chapter=origin.name))
        return node('Para', [image_inline(name, f'{origin.stem}：流程图')])
    if t == 'Math':
        counts['formulas'] += 1
        name = f'formula-{counts["formulas"]:02d}'
        media.append(dict(name=name, kind='math', source=c[1], display=c[0]['t']=='DisplayMath', chapter=origin.name))
        return image_inline(name, c[1].strip(), ['math-display' if c[0]['t']=='DisplayMath' else 'math-inline'])
    return {key: transform(v, origin) for key, v in value.items()}

blocks = []
for doc in docs:
    blocks.extend(transform(doc['ast']['blocks'], doc['path']))

note = ('本连续阅读版编排于2026年10月8日，收录全书导读、24章正文与6份附录。'
        '各章的历史实验、源码读取日期与验证范围沿用分章书稿。图表与公式随书内嵌。'
        '跨章引用可在书内跳转；源码与原始记录的链接通向书末出处索引，索引中的路径相对于项目根目录。')
blocks.insert(0, node('Para', string(note)))
blocks.append(node('Header', [1, ['references', [], []], string('出处索引')]))
blocks.append(node('Para', string('以下路径相对于 understand-book 项目根目录。电子书保留文件位置与段落锚点，源码和原始验证材料在项目中查阅。')))
for (target, fragment), ident in references.items():
    relative = target.relative_to(ROOT).as_posix()
    blocks.append(node('Para', [node('Span', [[ident, [], []], string(ident.upper()+'　')]),
                               node('Code', [['', [], []], relative]),
                               node('Str', ('　段落：'+fragment) if fragment else '')]))

ast = dict(docs[0]['ast'])
ast['blocks'] = blocks
ast['meta'] = {
    'title': node('MetaString', TITLE),
    'lang': node('MetaString', 'zh-CN'),
    'date': node('MetaString', '2026-10-08'),
    'toc-title': node('MetaString', '目录'),
}
dump(WORK/'book.json', ast)
dump(WORK/'media.json', media)

# Keep original prose, code and mathematical notation in the editable Markdown.
# Render diagrams as adjacent assets; escape generic type brackets in prose.
merged = [f'# {TITLE}\n\n{note.replace("书末出处索引，索引中的路径相对于项目根目录", "项目中的对应文件")}\n\n'
          '本文件的流程图位于同目录的 `assets/` 文件夹；移动Markdown时请一并携带。公式保留TeX标记。\n\n## 目录\n']
merged.extend(f'- [{d["title"]}](#{d["prefix"]})\n' for d in docs)
diagram_index = 0
for doc in docs:
    lines = doc['text'].splitlines(keepends=True)
    output = []
    fenced = False
    diagram = False
    hindex = 0
    for line in lines:
        if line.startswith('```'):
            if not fenced:
                fenced = True
                diagram = line.strip() == '```mermaid'
                if diagram:
                    diagram_index += 1
                    output.append(f'![{doc["title"]}：流程图](assets/diagram-{diagram_index:02d}.png)\n')
                else:
                    output.append(line)
            else:
                if not diagram:
                    output.append(line)
                fenced = diagram = False
            continue
        if fenced:
            if not diagram:
                output.append(line)
            continue
        heading = re.match(r'^(#{1,6}) (.*)', line)
        if heading:
            ident = doc['headers'][hindex]['c'][1][0]
            hindex += 1
            line = f'<a id="{ident}"></a>\n\n'+('#'*min(6,len(heading[1])+1))+' '+heading[2]+'\n'
        line = re.sub(r'(\[[^\]\n]+\])\(([^)\n]+)\)',
                      lambda m:m[1]+'('+link(m[2],doc['path'],False)+')', line)
        # Literal generic types in plain prose otherwise become invisible HTML.
        parts = re.split(r'(`+[^`]*`+)', line)
        line = ''.join(part if i % 2 else re.sub(r'<(AppState|AtomicBool|UserRuntime|Book)>', r'&lt;\1&gt;', part)
                       for i, part in enumerate(parts))
        output.append(line)
    merged.append('\n\n---\n\n'+''.join(output))
assert diagram_index == counts['diagrams'] == 26
(OUT/(STEM+'.md')).write_text(''.join(merged), encoding='utf-8')

subprocess.run([NODE, str(HERE/'render-assets.cjs')], check=True)
subprocess.run([PANDOC, '-f', 'json', '-t', 'epub3', str(WORK/'book.json'),
                '--toc', '--toc-depth=1', '--epub-chapter-level=1', '--no-highlight',
                '--css='+str(HERE/'reading.css'),
                '-o', str(OUT/(STEM+'.epub'))], cwd=OUT, check=True)
for doc in docs:
    assert doc['path'].read_bytes() == doc['original'], 'Source changed while exporting: '+str(doc['path'])
counts['source_reference_entries'] = len(references)
counts['code_excerpts'] = len(code_before)
dump(WORK/'build-record.json', counts)
with zipfile.ZipFile(OUT/(STEM+'-Markdown含插图.zip'), 'w', zipfile.ZIP_DEFLATED) as archive:
    archive.write(OUT/(STEM+'.md'), STEM+'.md')
    for path in sorted(ASSETS.glob('diagram-*.png')):
        archive.write(path, 'assets/'+path.name)
print(json.dumps(counts, ensure_ascii=False))

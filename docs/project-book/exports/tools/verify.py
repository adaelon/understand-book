"""Verify exported content and every packaged link; no product execution."""
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlsplit
from collections import Counter
import json
import posixpath
import re
import subprocess
import xml.etree.ElementTree as ET
import zipfile
from PIL import Image

HERE = Path(__file__).resolve().parent
OUT = HERE.parent
BOOK = OUT.parent
WORK = HERE/'work'
STEM = '深入UnderstandBook-连续阅读版'
PANDOC = r'C:\Program Files (x86)\Pandoc\pandoc.exe'
NS = {'x':'http://www.w3.org/1999/xhtml', 'o':'http://www.idpf.org/2007/opf'}
failures = []
checks = Counter()

def check(condition, message):
    if not condition:
        failures.append(message)

def walk(value):
    if isinstance(value, dict):
        yield value
        for item in value.values():
            yield from walk(item)
    elif isinstance(value, list):
        for item in value:
            yield from walk(item)

def compact(s):
    return re.sub(r'\s+', '', s)

with zipfile.ZipFile(OUT/(STEM+'.epub')) as z:
    members = z.namelist()
    check(members[0]=='mimetype' and z.getinfo('mimetype').compress_type==0, 'mimetype must be first and uncompressed')
    check(z.read('mimetype')==b'application/epub+zip', 'mimetype content')
    trees = {name:ET.fromstring(z.read(name)) for name in members if name.endswith(('.xhtml','.opf','.ncx','.xml'))}
    xhtml = {name:tree for name,tree in trees.items() if name.endswith('.xhtml')}
    ids = {}
    for name, tree in xhtml.items():
        values = [e.get('id') for e in tree.iter() if e.get('id')]
        check(len(values)==len(set(values)), 'Duplicate ID: '+name)
        ids[name] = set(values)
    for name, tree in trees.items():
        for e in tree.iter():
            for attr in ['href','src']:
                destination = e.get(attr)
                if not destination or urlsplit(destination).scheme:
                    continue
                url = urlsplit(destination)
                target = posixpath.normpath(posixpath.join(posixpath.dirname(name),unquote(url.path))) if url.path else name
                check(target in members, f'Missing EPUB target: {name}: {destination}')
                if url.fragment:
                    check(unquote(url.fragment) in ids.get(target,set()), f'Missing EPUB anchor: {name}: {destination}')
                checks['epub_local_references'] += 1
    opf = trees['EPUB/content.opf']
    check(opf.find('.//{http://purl.org/dc/elements/1.1/}language').text=='zh-CN','Language metadata')
    spine = opf.findall('o:spine/o:itemref',NS)
    checks['spine_items'] = len(spine)
    checks['xhtml_files'] = len(xhtml)
    checks['epub_images'] = len([n for n in members if n.endswith('.png')])
    check(checks['epub_images']==49,'Expected 26 diagrams and 23 formulas')
    check(sum(len(t.findall('.//x:table',NS)) for t in xhtml.values())==140,'Expected 140 tables')
    check(len(xhtml['EPUB/nav.xhtml'].findall('.//x:nav[@id="toc"]//x:a',NS))==33,'Navigation should contain introduction, 31 source documents and references')
    z.extractall(WORK/'preview')

files = sorted((BOOK/'chapters').glob('*.md'))+sorted((BOOK/'appendices').glob('*.md'))
original_code = []
original_math = []
for index, path in enumerate(files):
    prefix = f'ch{index:02d}' if index<25 else f'app-{path.name[0].lower()}'
    matched = [tree for name,tree in xhtml.items() if prefix in ids[name]]
    check(len(matched)==1,'Chapter missing or duplicated: '+path.name)
    tree = matched[0]
    actual = compact(''.join(tree.itertext()))
    ast = json.loads(subprocess.check_output([PANDOC,'-f','markdown+tex_math_single_backslash-smart-citations-raw_html','-t','json',str(path)]))
    expected_code = []
    for item in walk(ast['blocks']):
        kind,c = item.get('t'),item.get('c')
        if kind in ('Str','Code'):
            value = c if kind=='Str' else c[1]
            check(compact(value) in actual, f'Lost prose/inline code in {path.name}: {value[:100]}')
            checks['prose_and_inline_code_fragments'] += 1
        if kind=='Math':
            original_math.append(c)
        if kind=='CodeBlock' and 'mermaid' not in c[0][1]:
            expected_code.append(c[1].strip())
    actual_code = [''.join(e.itertext()).strip() for e in tree.findall('.//x:pre',NS)]
    check(expected_code==actual_code,'Code excerpt mismatch in EPUB: '+path.name)
    original_code.extend(expected_code)
    checks['source_documents'] += 1

md = (OUT/(STEM+'.md')).read_text(encoding='utf-8')
md_ast = json.loads(subprocess.check_output([PANDOC,'-f','markdown+tex_math_single_backslash-smart-citations','-t','json',str(OUT/(STEM+'.md'))]))
md_code = [n['c'][1].strip() for n in walk(md_ast['blocks']) if n.get('t')=='CodeBlock']
md_math = [n['c'] for n in walk(md_ast['blocks']) if n.get('t')=='Math']
check(md_code==original_code,'Markdown code excerpts changed')
check(md_math==original_math,'Markdown formulas changed')
anchors = set(re.findall(r'<a id="([^"]+)"', md))
for n in walk(md_ast['blocks']):
    if n.get('t') not in ('Link','Image'):
        continue
    destination=n['c'][2][0]
    if urlsplit(destination).scheme:
        continue
    location,_,fragment=destination.partition('#')
    if location:
        target=(OUT/unquote(location)).resolve()
        check(target.exists(),'Missing Markdown target: '+destination)
        if fragment and target.suffix=='.md':
            text=re.sub(r'^```[^\n]*\n.*?^```\s*$','',target.read_text(encoding='utf-8-sig'),flags=re.M|re.S)
            headings=set()
            seen=Counter()
            for title in re.findall(r'^#{1,6}\s+(.+?)\s*$',text,re.M):
                title=re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',title)
                slug=re.sub(r'[^\w\s-]','',title.lower()).replace(' ','-')
                count=seen[slug];seen[slug]+=1
                headings.add(slug if count==0 else f'{slug}-{count}')
            check(unquote(fragment) in headings,'Missing external Markdown anchor: '+destination)
    else:
        check(unquote(fragment) in anchors,'Missing Markdown anchor: '+destination)
    checks['markdown_local_references']+=1
for p in (OUT/'assets').glob('*.png'):
    with Image.open(p) as im:
        im.verify()
    checks['valid_images']+=1
check(len(original_code)==150,'Expected 150 code excerpts')
check(len(original_math)==23,'Expected 23 formulas')
check(len(anchors)==522,'Expected 522 section anchors')
checks['code_excerpts_compared_in_both_formats']=len(original_code)
checks['formulas_compared_in_markdown']=len(original_math)
checks['section_anchors']=len(anchors)
record=dict(status='passed' if not failures else 'failed',counts=dict(checks),failures=failures)
(WORK/'verification.json').write_text(json.dumps(record,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(record,ensure_ascii=False,indent=2))
raise SystemExit(bool(failures))

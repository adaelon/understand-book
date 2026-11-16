"""Engrave score-only measures omitted by the archived performance route."""
import copy
from pathlib import Path
import xml.etree.ElementTree as ET

SVG = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG)
ET.register_namespace('xlink', 'http://www.w3.org/1999/xlink')


def supplement_unperformed_score(xml_path, output, pages, objects, parts):
    import verovio

    original = ET.parse(xml_path).getroot()
    source_parts = original.findall('part')
    represented = {int(obj['measure']) for obj in objects}
    missing = [i for i in range(1, len(source_parts[0].findall('measure')) + 1) if i not in represented]
    batches = []
    for index in missing:
        if not batches or index != batches[-1][-1] + 1 or len(batches[-1]) == 6:
            batches.append([])
        batches[-1].append(index)
    added = []
    for batch in batches:
        first, last = batch[0], batch[-1]
        page_id = f'unperformed-{first}-{last}'
        excerpt = ET.Element('score-partwise', original.attrib)
        part_list = copy.deepcopy(original.find('part-list'))
        for part in part_list.findall('score-part'):
            part.find('part-name').text = parts[part.get('id')]['name']
        excerpt.append(part_list)
        page_objects = []
        for part_index, source in enumerate(source_parts, 1):
            part_id = source.get('id')
            part = ET.SubElement(excerpt, 'part', source.attrib)
            state = {}
            for measure_index, measure in enumerate(source.findall('measure'), 1):
                if measure_index > last:
                    break
                before = dict(state)
                for attributes in measure.findall('attributes'):
                    for child in attributes:
                        state[(child.tag, child.get('number'))] = copy.deepcopy(child)
                if measure_index < first:
                    continue
                clone = copy.deepcopy(measure)
                if measure_index == first:
                    inherited = ET.Element('attributes')
                    for child in before.values():
                        inherited.append(copy.deepcopy(child))
                    clone.insert(0, inherited)
                divisions = float(state[('divisions', None)].text)
                transposition = state.get(('transpose', None))
                transpose = 0 if transposition is None else int(transposition.findtext('chromatic', '0')) + 12 * int(transposition.findtext('octave-change', '0'))
                cursor = onset = 0
                note_index = 0
                for child in clone:
                    if child.tag in ('backup', 'forward'):
                        cursor += float(child.findtext('duration', '0')) / divisions * (-1 if child.tag == 'backup' else 1)
                    if child.tag != 'note':
                        continue
                    note_index += 1
                    sid = f's-p{part_index}-m{measure_index}-n{note_index}'
                    child.set('id', sid)
                    duration = float(child.findtext('duration', '0')) / divisions
                    if child.find('chord') is None:
                        onset = cursor
                        if child.find('grace') is None:
                            cursor += duration
                    staff, voice = child.findtext('staff', '1'), child.findtext('voice', '1')
                    voice_id = f'{staff}:{voice}'
                    if voice_id not in parts[part_id]['voices']:
                        parts[part_id]['voices'].append(voice_id)
                    pitch = child.find('pitch')
                    midi = None
                    if pitch is not None:
                        midi = (int(pitch.findtext('octave')) + 1) * 12 + {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}[pitch.findtext('step')] + int(pitch.findtext('alter', '0')) + transpose
                    ties = {tie.get('type') for tie in child.findall('tie')}
                    rest = child.find('rest') is not None
                    obj = {'source_id': sid, 'part_id': part_id, 'measure': str(measure_index), 'beat': onset,
                        'duration_beats': duration, 'voice': voice_id, 'midi_pitch': midi, 'rest': rest,
                        'tie': 'none' if rest else 'continue' if ties == {'start', 'stop'} else 'start' if 'start' in ties else 'stop' if 'stop' in ties else 'none',
                        'page_id': page_id, 'svg_element_id': sid}
                    if child.find('unpitched') is not None:
                        obj['unpitched'] = True
                    page_objects.append(obj)
                part.append(clone)
        toolkit = verovio.toolkit()
        toolkit.setOptions({'breaks': 'none', 'pageWidth': 2200, 'adjustPageHeight': True,
            'header': 'none', 'footer': 'none', 'svgViewBox': True})
        if not toolkit.loadData(ET.tostring(excerpt, encoding='unicode')):
            raise ValueError(f'Cannot engrave unperformed measures {first}–{last}')
        if toolkit.getPageCount() != 1:
            raise ValueError('Unperformed excerpt must fit one score page')
        svg = ET.fromstring(toolkit.renderToSVG(1))
        for label_group in svg.iter():
            if label_group.get('class') != 'label':
                continue
            text = label_group.find(f'{{{SVG}}}text')
            label = ''.join(text.itertext()).strip()
            y = text.get('y')
            text.clear()
            text.attrib.update({'x': '200', 'y': y, 'text-anchor': 'start', 'font-size': '360px', 'font-family': 'Noto Serif SC, SimSun, serif'})
            name, separator, tuning = label.partition('（')
            text.text = name
            if separator:
                ET.SubElement(text, f'{{{SVG}}}tspan', {'x': '200', 'dy': '320', 'font-size': '280px'}).text = '（' + tuning
        drawn = {node.get('id') for node in svg.iter() if len(node)}
        absent = [obj['source_id'] for obj in page_objects if obj['source_id'] not in drawn]
        if absent:
            raise ValueError(f'Unperformed notation was not engraved: {absent[:5]}')
        _, _, width, height = map(float, svg.get('viewBox').split())
        svg.set('viewBox', f'0 0 {width} {height + 44}')
        content = ET.Element(f'{{{SVG}}}g', {'transform': 'translate(0, 44)'})
        for child in list(svg):
            if child.tag.rsplit('}', 1)[-1] not in ('defs', 'style', 'desc'):
                svg.remove(child)
                content.append(child)
        label = f'第二乐章 · 源小节 {first}–{last} · 本次录音未演奏'
        ET.SubElement(svg, f'{{{SVG}}}title').text = label
        ET.SubElement(svg, f'{{{SVG}}}text', {'x': '16', 'y': '26', 'font-size': '14', 'font-family': 'Noto Serif SC, SimSun, serif', 'fill': '#687364'}).text = label
        svg.append(content)
        relative = f'music/score/{page_id}.svg'
        Path(output, relative).write_text(ET.tostring(svg, encoding='unicode'), encoding='utf8')
        pages.append({'page_id': page_id, 'path': relative, 'width': width, 'height': height + 44})
        objects.extend(page_objects)
        added.extend(page_objects)
    return {'measures': missing, 'engraver_version': toolkit.getVersion() if batches else None, 'pitched_notes': sum(obj['midi_pitch'] is not None for obj in added),
        'rests': sum(obj['rest'] for obj in added), 'pages': len(batches)}

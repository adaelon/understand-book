"""Decode the whole delivered movie and extract semantic keyframes for visual QA."""
import json
import sys
from pathlib import Path

import av
from PIL import Image, ImageDraw

movie = Path(sys.argv[1] if len(sys.argv) > 1 else "learning-rate-final.mp4")
evidence = json.loads(Path("scene-evidence.json").read_text(encoding="utf-8"))
events = evidence['events']
red = next(e['time'] for e in events if e.get('crossing_eta') == 1.1)
orange = next(e['time'] for e in events if e.get('crossing_eta') == .8)
first = next(e['time'] for e in events if e.get('endpoint') and e['k'] == 1)
last = next(e['time'] for e in events if e.get('endpoint') and e['k'] == 5)
targets = [('intro', 3), ('moving', red-.5), ('red-cross', red+.3),
           ('orange-cross', orange+.3), ('first-end', first+.3),
           ('fifth-end', last+.3), ('summary', evidence['duration']-2)]
folder = Path('frames') / movie.stem
folder.mkdir(parents=True, exist_ok=True)
captures = []
with av.open(str(movie)) as container:
    stream = container.streams.video[0]
    fps = float(stream.average_rate)
    previous = -1
    count = 0
    next_target = 0
    for frame in container.decode(stream):
        time = float(frame.time)
        assert time > previous, (time, previous)
        previous = time
        count += 1
        if next_target < len(targets) and time >= targets[next_target][1]:
            name, target = targets[next_target]
            path = folder / f'{name}.png'
            frame.to_image().save(path)
            captures.append(dict(name=name, target=target, actual=time, path=str(path)))
            next_target += 1
    duration = count / fps
    assert next_target == len(targets), 'Missing end of movie'
    # Manim rounds each animation duration to complete frames.
    assert abs(duration - evidence['duration']) < .15, (duration, evidence['duration'])
    result = dict(movie=str(movie), width=stream.width, height=stream.height,
                  fps=fps, frames=count, duration=duration, decoded=True, captures=captures)
Path(f'{movie.stem}-media-check.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
sheet = Image.new('RGB', (960, 4*294), '#e4e9ed')
draw = ImageDraw.Draw(sheet)
for i, capture in enumerate(captures):
    picture = Image.open(capture['path']).convert('RGB')
    picture.thumbnail((480, 270))
    x, y = (i%2)*480, (i//2)*294
    sheet.paste(picture, (x, y+24))
    draw.text((x+8, y+4), f"{capture['name']}  {capture['actual']:.2f}s", fill='#1c2938')
sheet.save(folder / 'contact-sheet.jpg')
print(json.dumps(result, ensure_ascii=False, indent=2))

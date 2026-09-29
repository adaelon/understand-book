"""Fixed Cairo entrypoint; validate and sample the actual encoded media with PyAV."""
import json
import shutil
from pathlib import Path

import av
import manim
from manim import Scene, tempconfig

if manim.__version__ != "0.21.0":
    raise RuntimeError("Manim 0.21.0 is required; configure UNDERSTAND_BOOK_ANIMATION_PYTHON")
args = json.loads(Path("input.json").read_text(encoding="utf-8"))
namespace = dict(vars(manim), data=args["data"], __name__="presentation_animation")
exec(compile(Path("scene.py").read_text(encoding="utf-8"), "scene.py", "exec"), namespace)
scene_class = namespace.get("PresentationAnimation")
if not isinstance(scene_class, type) or not issubclass(scene_class, Scene):
    raise RuntimeError("Define class PresentationAnimation(Scene)")
with tempconfig({"renderer": "cairo", "pixel_width": args["width"], "pixel_height": args["height"],
                 "frame_rate": 30, "frame_width": 8 * args["width"] / args["height"], "frame_height": 8,
                 "media_dir": "media", "output_file": "animation", "format": "mp4",
                 "write_to_movie": True, "disable_caching": True, "progress_bar": "none"}):
    scene = scene_class()
    scene.render()
    shutil.copyfile(scene.renderer.file_writer.movie_file_path, "animation.mp4")
if Path("animation.mp4").stat().st_size > 8 * 1024 * 1024:
    raise RuntimeError("MP4 exceeds 8 MiB; shorten the clip or reduce size")
with av.open("animation.mp4") as container:
    if len(container.streams.video) != 1 or container.streams.audio:
        raise RuntimeError("Expected one video stream without audio")
    stream = container.streams.video[0]
    fps = float(stream.average_rate)
    duration = float(stream.duration * stream.time_base)
    if duration <= 0 or abs(fps - 30) > 0.01:
        raise RuntimeError("Expected a nonempty 30fps animation")
    if any(cue["at_seconds"] > duration for cue in args["cues"]):
        raise RuntimeError(f"Cue is beyond actual duration {duration:.6f}s")
    targets = sorted(set([0, duration / 2, max(0, duration - 1 / fps)] +
                         [cue["at_seconds"] for cue in args["cues"][:1]]))
    frames = []
    target = 0
    last = None
    for frame in container.decode(stream):
        last = frame
        time = float(frame.time)
        while target < len(targets) and time + 0.5 / fps >= targets[target]:
            name = f"frame-{target}.png"
            frame.to_image().save(name)
            frames.append({"file": name, "at_seconds": time})
            target += 1
    if last is None:
        raise RuntimeError("No video frame decoded")
    if target < len(targets):
        name = f"frame-{target}.png"
        last.to_image().save(name)
        frames.append({"file": name, "at_seconds": float(last.time)})
    Path("metadata.json").write_text(json.dumps({"width": stream.width, "height": stream.height,
        "duration_seconds": duration, "fps": fps, "frames": frames}), encoding="utf-8")

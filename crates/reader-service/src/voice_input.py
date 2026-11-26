"""Decode one browser recording and return literal dictation, without a book prompt."""
import json
import os
import sys
from pathlib import Path

import numpy as np
from faster_whisper import WhisperModel
from faster_whisper.audio import decode_audio


def transcribe(audio_file, model_dir, language):
    audio = decode_audio(audio_file, sampling_rate=16000)
    duration = len(audio) / 16000
    if duration > 92:
        raise ValueError("Please record at most 90 seconds.")
    if not len(audio) or np.max(np.abs(audio)) < 0.001:
        return {"text": "", "duration_ms": round(duration * 1000)}
    model = WhisperModel(model_dir, device="cpu", compute_type="int8", cpu_threads=4, local_files_only=True)
    segments, _ = model.transcribe(
        audio, language=language, task="transcribe", beam_size=5,
        temperature=0, vad_filter=True, condition_on_previous_text=False,
        vad_parameters={"min_silence_duration_ms": 400},
    )
    text = "".join(segment.text for segment in segments).strip()
    return {"text": text, "duration_ms": round(duration * 1000)}


if __name__ == "__main__":
    audio_file, model_dir, language, output_file = sys.argv[1:]
    audio_path = os.path.abspath(audio_file)
    output_path = Path(output_file).absolute()
    # Windows app resources may use a verbatim path that CTranslate2 cannot open.
    # This one-request subprocess can load the model relative to its own directory.
    os.chdir(model_dir)
    result = transcribe(audio_path, ".", language)
    output_path.write_text(json.dumps(result, ensure_ascii=False), encoding="utf-8")

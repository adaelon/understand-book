import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { musicCapabilities, validateMusicMaterialSuccessor, validateMusicQuestionContext, validateMusicSourceManifest } from "./music-contract";
import type { MusicSourceManifest, MusicQuestionContext } from "./music-contract";

function fixture(): MusicSourceManifest {
  return JSON.parse(readFileSync(new URL("../../../crates/base-schema/tests/fixtures/music.json", import.meta.url), "utf8"));
}

describe("curated music source contracts", () => {
  it('preserves score screenshots even without indexed score objects and rejects oversized images', () => {
    const m = fixture(), publication = { book_id: m.book_id, publication_id: 'release-1' };
    const q: MusicQuestionContext = { draft_id: 'staff-lines', publication,
      audio: { recording: m.recording.source, start_ms: 0, end_ms: 1000 }, score: null, occurrence_id: null, focused_part_ids: [],
      image: { png_base64: 'score-pixels', width: 800, height: 240, origin: 'selection' } };
    expect(validateMusicQuestionContext(q, m, publication).image).toEqual(q.image);
    expect(() => validateMusicQuestionContext({ ...q, image: { ...q.image, width: 2049 } }, m, publication)).toThrow();
  });
  it("supports repeated occurrences and simultaneous chord notes without a transcript", () => {
    const m = validateMusicSourceManifest(fixture());
    expect(m.note_alignments).toHaveLength(4);
    expect(musicCapabilities(m)).toEqual(["music", "music_score", "music_events", "music_note_alignment"]);
    expect(m).not.toHaveProperty("transcript");
  });
  it("requires every played note for note coverage, including the second repetition", () => {
    const m = fixture(); m.note_alignments.pop();
    expect(() => validateMusicSourceManifest(m)).toThrow("note coverage gap");
    m.coverage[0].granularity = "measure";
    expect(musicCapabilities(validateMusicSourceManifest(m))).not.toContain("music_note_alignment");
    m.occurrences[1].score_object_ids.pop();
    expect(() => validateMusicSourceManifest(m)).toThrow("occurrence");
  });
  it("rejects missing references and spurious attacks on tied continuations", () => {
    const missing = fixture(); missing.note_alignments[0].occurrence_id = "missing";
    expect(() => validateMusicSourceManifest(missing)).toThrow("note alignment");
    const tied = fixture(); tied.score_objects[0].tie = "stop";
    expect(() => validateMusicSourceManifest(tied)).toThrow("note alignment");
    tied.note_alignments[0].attack = tied.note_alignments[2].attack = false;
    expect(() => validateMusicSourceManifest(tied)).not.toThrow();
    tied.events[0].relations[0].target_event_id = "missing";
    expect(() => validateMusicSourceManifest(tied)).toThrow("event relation");
  });
  it("distinguishes synthesized timing and rejects contradictory coverage claims", () => {
    const m = fixture(); m.provenance[1].kind = "score_synthesis"; m.coverage[0].method = "score_synthesis";
    expect(() => validateMusicSourceManifest(m)).toThrow();
    m.recording.kind = "score_synthesis";
    expect(() => validateMusicSourceManifest(m)).not.toThrow();
    m.coverage.push({ ...m.coverage[0] });
    expect(() => validateMusicSourceManifest(m)).toThrow("coverage overlap");
  });
  it("retains reviewed unobserved tones without inventing attacks or dropping coverage", () => {
    const m = fixture(); m.note_alignments[0].unobserved = true;
    expect(() => validateMusicSourceManifest(m)).toThrow("note alignment");
    m.note_alignments[0].attack = false;
    expect(validateMusicSourceManifest(m).note_alignments[0].unobserved).toBe(true);
    expect(musicCapabilities(m)).toContain("music_note_alignment");
  });
  it("permits layout and semantic revisions while preserving source anchors", () => {
    const old = fixture(), next = fixture();
    next.pages[0].width = 800; next.score_objects[0].svg_element_id = "new-render-id";
    next.score.musicxml_path = "repacked/score.musicxml"; next.recording.media_path = "repacked/audio.flac";
    next.note_alignments[2].start_ms = 2100; next.events[1].explanation = "Updated teaching annotation.";
    expect(() => validateMusicMaterialSuccessor(next, old)).not.toThrow();
    next.score_objects[0].midi_pitch = 61;
    expect(() => validateMusicMaterialSuccessor(next, old)).toThrow("immutable score location");
    const changed = fixture(); changed.work.work_id = "different-work";
    expect(() => validateMusicMaterialSuccessor(changed, old)).toThrow("immutable material source");
  });
  it("keeps a question at the original publication and repeated occurrence", () => {
    const m = fixture(), publication = { book_id: m.book_id, publication_id: "release-1" };
    const q: MusicQuestionContext = { draft_id: "question-1", publication, audio: { recording: m.recording.source, start_ms: 2100, end_ms: 2600 },
      score: { score: m.score.source, source_ids: ["score/note/c"] }, occurrence_id: "repeat", focused_part_ids: ["piano"] };
    expect(validateMusicQuestionContext(q, m, publication)).toEqual(q);
    expect(() => validateMusicQuestionContext(q, m, { ...publication, publication_id: "release-2" })).toThrow("question publication");
    expect(() => validateMusicQuestionContext({ ...q, occurrence_id: "first" }, m, publication)).toThrow("question focus");
  });
  it("assigns melody and accompaniment to actual voices within one instrument", () => {
    const m = fixture();
    expect(m.parts).toHaveLength(1);
    expect(m.parts[0].voices).toEqual(["1", "2"]);
    expect(validateMusicSourceManifest(m).events[0].roles[0]).not.toHaveProperty("voice_ids");
    m.events[0].roles = [{ part_id: "piano", role: "melody", voice_ids: ["1"] },
      { part_id: "piano", role: "accompaniment", voice_ids: ["2"] }];
    expect(() => validateMusicSourceManifest(m)).not.toThrow();
    m.events[0].roles[1].voice_ids = ["3"];
    expect(() => validateMusicSourceManifest(m)).toThrow("event role");
    m.events[0].roles[1].voice_ids = ["2", "2"];
    expect(() => validateMusicSourceManifest(m)).toThrow("event role");
    m.events[0].roles[1].voice_ids = [];
    expect(() => validateMusicSourceManifest(m)).not.toThrow();
  });
  it("accepts legacy questions and binds focused voices to their instrument", () => {
    const m = fixture(), publication = { book_id: m.book_id, publication_id: "release-1" };
    const q: MusicQuestionContext = { draft_id: "question-1", publication, audio: { recording: m.recording.source, start_ms: 2100, end_ms: 2600 },
      score: null, occurrence_id: "repeat", focused_part_ids: [] };
    expect(validateMusicQuestionContext(q, m, publication)).not.toHaveProperty("focused_voices");
    q.focused_voices = [{ part_id: "piano", voice: "1" }];
    expect(() => validateMusicQuestionContext(q, m, publication)).not.toThrow();
    q.focused_voices = [{ part_id: "piano", voice: "3" }];
    expect(() => validateMusicQuestionContext(q, m, publication)).toThrow("voice focus");
    q.focused_voices = [{ part_id: "flute", voice: "1" }];
    expect(() => validateMusicQuestionContext(q, m, publication)).toThrow("voice focus");
    q.focused_voices = [{ part_id: "piano", voice: "2" }, { part_id: "piano", voice: "2" }];
    expect(() => validateMusicQuestionContext(q, m, publication)).toThrow("question voice focus");
  });
  it("binds optional event playback occurrences without rebinding audio evidence", () => {
    const m = fixture(), evidence = structuredClone(m.events[1].evidence[0].audio);
    expect(validateMusicSourceManifest(m).events[1]).not.toHaveProperty("occurrence_ids");
    m.events[1].occurrence_ids = ["repeat"];
    expect(validateMusicSourceManifest(m).events[1].evidence[0].audio).toEqual(evidence);
    m.events[1].occurrence_ids = ["missing"];
    expect(() => validateMusicSourceManifest(m)).toThrow("event occurrence");
    m.events[1].occurrence_ids = ["repeat", "repeat"];
    expect(() => validateMusicSourceManifest(m)).toThrow("event occurrence");
  });
  it("keeps explicit theme identity across roles while accepting absent or null theme", () => {
    const m = fixture();
    expect(validateMusicSourceManifest(m).events[0].roles[0]).not.toHaveProperty("theme_id");
    m.events[0].roles[0].theme_id = "main-theme";
    m.events[1].roles = [{ part_id: "piano", role: "returning theme", voice_ids: ["2"], theme_id: "main-theme" }];
    expect(validateMusicSourceManifest(m).events.map(e => e.roles[0].theme_id)).toEqual(["main-theme", "main-theme"]);
    m.events[0].roles[0].theme_id = null;
    expect(() => validateMusicSourceManifest(m)).not.toThrow();
    m.events[1].roles[0].theme_id = " ";
    expect(() => validateMusicSourceManifest(m)).toThrow();
  });
});

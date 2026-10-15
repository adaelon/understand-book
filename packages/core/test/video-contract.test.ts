import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { z } from "zod";
import { ReadOnlyBaseZ } from "../src/zod";
import { buildVideoSourceManifest } from "../src/source-manifest";
import {
  VideoSourceManifestZ, TranscriptMapZ, VideoQuestionContextZ, EvidenceTargetZ, MediaObservationZ, TranscriptTranslationZ,
  validateTranscriptMap, validateVideoQuestionContext, validateEvidenceTarget, validateMediaObservation,
  validateTranscriptTranslation, videoCoverageGaps,
  validateMediaLocation,
  validateVideoMaterialSuccessor,
} from "../src/video-contract";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const fixturePath = new URL("../../../crates/base-schema/fixtures/video-contract.json", import.meta.url);
const CaseZ = z.object({ manifest: VideoSourceManifestZ, map: TranscriptMapZ, base: ReadOnlyBaseZ, source_text: z.string(),
  questions: z.array(VideoQuestionContextZ), targets: z.array(EvidenceTargetZ), observations: z.array(MediaObservationZ), translations: z.array(TranscriptTranslationZ) });
const FixtureZ = z.object({ provenance: z.unknown(), cases: z.array(CaseZ) });
type Case = z.infer<typeof CaseZ>;
function fixture() { return FixtureZ.parse(JSON.parse(readFileSync(fixturePath, "utf8"))); }
function lecture() { return fixture().cases[0]; }
function validate(c: Case): Case {
  const manifest = buildVideoSourceManifest(c.manifest);
  return { ...c, manifest, map: validateTranscriptMap(c.map, manifest, c.base, c.source_text),
    questions: c.questions.map(q => validateVideoQuestionContext(q, manifest)),
    targets: c.targets.map(t => validateEvidenceTarget(t, manifest, c.base, c.source_text)),
    observations: c.observations.map(o => validateMediaObservation(o, manifest)),
    translations: c.translations.map(t => validateTranscriptTranslation(t, c.map)) };
}

describe("VI1 video source contracts", () => {
  it("writes TS data, validates and serializes in Rust, then reads back without loss", () => {
    const raw = JSON.parse(readFileSync(fixturePath, "utf8"));
    const f = fixture(); const sent = { ...f, cases: f.cases.map(validate) };
    expect(sent).toEqual(raw);
    const received = execFileSync("cargo", ["run", "--quiet", "-p", "base-schema", "--example", "video_contract_roundtrip"],
      { cwd: root, input: JSON.stringify(sent), encoding: "utf8", timeout: 120000 });
    const back = FixtureZ.parse(JSON.parse(received));
    back.cases.forEach(validate);
    expect(back).toEqual(sent);
  }, 130000);

  it("retains the actual PTS, original crop dimensions and independent repeated positions", () => {
    const f = fixture(); f.cases.forEach(validate);
    const o = f.cases[0].observations[0]; expect(o.kind).toBe("visual");
    if (o.kind !== "visual") throw new Error("fixture");
    expect(o.frames[0].requested_ms).toBe(555123);
    expect(o.frames[0].frame.actual_pts_ms).toBe(555133.3125);
    expect(o.regions[0].frame).toMatchObject({ width: 1920, height: 1080 });
    const e = f.cases[1].targets[0]; if (e.kind !== "visual") throw new Error("fixture");
    expect(e.frames.map(f => f.actual_pts_ms)).toEqual([1000, 5000]);
    expect(f.cases[1].base.lid_nodes).toEqual([]);
  });

  it("rejects missing identity and nonfinite times", () => {
    const c = lecture();
    const { media_id: _, ...noIdentity } = c.manifest.source.video;
    expect(() => buildVideoSourceManifest({ ...c.manifest, source: { ...c.manifest.source, video: noIdentity } })).toThrow();
    c.questions[0].frame.actual_pts_ms = NaN;
    expect(() => validate(c)).toThrow();
  });

  const mutations: [string, (c: Case) => void][] = [
    ["transcript revision", c => { c.map.transcript.transcript_revision = "other"; }],
    ["missing LID", c => { c.map.segments[0].text_range.lid = "missing"; }],
    ["wrong UTF-16 mapping", c => { c.map.segments[0].text_range.range.end++; }],
    ["invented text", c => { c.map.segments[0].text = "invented transcript"; }],
    ["out of film", c => { c.map.segments[0].location.end_ms = c.manifest.source.duration_ms+1; }],
    ["edited film", c => { c.map.segments[0].location.video.media_revision = "edited"; }],
    ["duplicate segment", c => { c.map.segments[1].segment_id = c.map.segments[0].segment_id; }],
    ["segment order", c => { c.map.segments.reverse(); }],
    ["word text offset", c => { c.map.segments[0].words[0].range.end = 1000; }],
    ["word outside segment", c => { c.map.segments[0].words[0].end_ms = 590000; }],
    ["frame at exclusive end", c => { c.questions[0].frame.actual_pts_ms = c.manifest.source.duration_ms; }],
    ["crop overflow", c => { c.questions[0].region!.rect_normalized = [0.9, 0, 0.2, 1]; }],
    ["crop dimensions substituted for original", c => { c.questions[0].region!.frame.width = 1120; }],
    ["different crop frame", c => { c.questions[0].region!.frame.actual_pts_ms = 650000; }],
    ["wrong publication book", c => { c.questions[0].publication.book_id = "new"; }],
    ["question text version", c => { c.questions[0].material.transcript.transcript_revision = "new"; }],
    ["translation text version", c => { c.translations[0].transcript.transcript_revision = "old"; }],
    ["translation segment", c => { c.translations[0].segment_id = "missing"; }],
    ["imported file must be relative", c => { c.manifest.source.imported_path = "C:/external.mp4"; }],
    ["material revision requires different book", c => { c.manifest.previous_book_id = c.manifest.material.book_id; }],
    ["duplicate coverage", c => { c.manifest.coverage.push(c.manifest.coverage[0]); }],
  ];
  it.each(mutations)("rejects %s", (_, mutate) => { const c = lecture(); mutate(c); expect(() => validate(c)).toThrow(); });

  it("keeps segment precision without words and treats failed intervals as gaps", () => {
    const c = lecture(); c.map.segments[1].words = []; validate(c);
    const silent = fixture().cases[1];
    expect(silent.map.segments).toEqual([]);
    expect(videoCoverageGaps(silent.manifest.coverage[0], silent.manifest.source)).toEqual([]);
    const p = (start_ms: number, end_ms: number) => ({ video: silent.manifest.source.video, start_ms, end_ms });
    const coverage = { capability: "transcript" as const, accepted: [p(4000,6000),p(0,2000)], pending: [], failed: [p(2000,4000)] };
    expect(videoCoverageGaps(coverage, silent.manifest.source)).toEqual([p(2000,4000)]);
    coverage.accepted.push(p(2500,3000));
    expect(() => videoCoverageGaps(coverage, silent.manifest.source)).toThrow("overlap");
  });

  it("requires processing references and per-member support in observation/evidence contracts", () => {
    const c = lecture();
    for (const o of c.observations) {
      const broken = o.kind === "visual" ? { ...o, model_request_id: "" } : { ...o, tool_call_id: "" };
      expect(() => validateMediaObservation(broken, c.manifest)).toThrow();
    }
    const target = c.targets[3]; if (target.kind !== "joint") throw new Error("fixture");
    target.members[0].support_role = "";
    expect(() => validate(c)).toThrow();
    target.members[0].support_role = "transcript";
    const m = target.members[1].target; if (m.kind === "joint") throw new Error("fixture");
    m.publication.publication_id = "later";
    expect(() => validate(c)).toThrow("joint publication");
  });

  it("retains a real zero-duration Whisper word without making it a playable interval", () => {
    const c = fixture().cases[2]; validate(c);
    const w = c.map.segments[0].words[0];
    expect([w.start_ms, w.end_ms]).toEqual([1180000,1180000]);
    expect(() => validateMediaLocation({ video: c.manifest.source.video, start_ms: w.start_ms, end_ms: w.end_ms }, c.manifest.source)).toThrow();
  });

  it("keeps historical locations when workspace paths and VU grouping change", () => {
    const c = lecture(); const old = structuredClone(c.targets);
    c.manifest.source.original_path = "D:/moved/original.mp4";
    c.manifest.source.imported_path = "materials/original.mp4";
    validate(c); expect(c.targets).toEqual(old);
    expect(JSON.stringify(old)).not.toContain("visual_unit");
    const next = structuredClone(c.manifest); next.material.book_id = "revised"; next.previous_book_id = c.manifest.material.book_id;
    next.material.transcript.transcript_revision = "corrected";
    buildVideoSourceManifest(next);
    validateVideoMaterialSuccessor(next, c.manifest);
    expect(() => validateEvidenceTarget(old[0], next, c.base, c.source_text)).toThrow();
    expect(validateEvidenceTarget(old[0], c.manifest, c.base, c.source_text)).toEqual(old[0]);
  });

  it("requires a new book and predecessor for source revisions, preserves book for enrichment", () => {
    const c = lecture(); const next = structuredClone(c.manifest);
    next.coverage = []; validateVideoMaterialSuccessor(next, c.manifest);
    next.material.transcript.transcript_revision = "corrected";
    expect(() => validateVideoMaterialSuccessor(next, c.manifest)).toThrow();
    next.material.book_id = "new-book";
    expect(() => validateVideoMaterialSuccessor(next, c.manifest)).toThrow();
    next.previous_book_id = c.manifest.material.book_id;
    validateVideoMaterialSuccessor(next, c.manifest);
    next.material.transcript = structuredClone(c.manifest.material.transcript);
    expect(() => validateVideoMaterialSuccessor(next, c.manifest)).toThrow();
  });
});

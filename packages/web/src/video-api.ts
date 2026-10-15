import type { VideoSourceRef } from "../../core/src/generated/VideoSourceRef";
import type { VideoImportStage } from "../../core/src/generated/VideoImportStage";
export type PreparingVideo = { video: VideoSourceRef; title: string; stage: VideoImportStage;
  copied_bytes: number; total_bytes: number; duration_ms: number | null;
  playable: boolean; qa_ready: false; failure: string | null; english_ready?: boolean;
  publication?: { book_id: string; publication_id: string } | null;
  transcript_progress?: { stage: string; failure: string | null; chunks: { id: string; status: string; start_ms: number; end_ms: number; failure: string | null }[] } | null };
export function videoUrl(video: VideoSourceRef, leaf: string): string {
  return `/api/videos/${encodeURIComponent(video.media_id)}/${encodeURIComponent(video.media_revision)}/${leaf}`;
}
export async function videoGet<T>(url: string): Promise<T> {
  const response = await fetch(url, { credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new Error(response.status === 401 ? "请重新登录后打开视频" : "视频暂不可读取，请检查材料是否仍可用");
  return response.json();
}
export const videoCatalog = () => videoGet<{ videos: PreparingVideo[]; active?: VideoSourceRef | null }>("/api/videos");

export type PublishedVideo = { time_base: [number, number]; publication: import('../../core/src/generated/PublishedBookRef').PublishedBookRef;
  material: import('../../core/src/generated/VideoMaterial').VideoMaterial;
  source: import('../../core/src/generated/VideoSourceAsset').VideoSourceAsset;
  map: import('../../core/src/generated/TranscriptMap').TranscriptMap;
  coverage?: import('../../core/src/generated/VideoCoverage').VideoCoverage[];
  translations?: import('../../core/src/generated/TranscriptTranslation').TranscriptTranslation[];
  translation_progress?: { tasks: { id: string; segment_ids: string[]; status: 'pending' | 'running' | 'accepted' | 'failed'; attempts: number; failure: string | null }[] } | null };

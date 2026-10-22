import type { BookRef } from './types';
export interface ReleaseMaterial { published_book_ref: BookRef; required_capabilities: string[] }
export interface ReleasePart { title: string; description: string; materials: BookRef[] }
export interface ExternalResource { title: string; url: string; description: string }
export interface ReleaseManifest {
  name: string; publisher: string; public_goal: string; applicability: string; prerequisites: string;
  materials: ReleaseMaterial[]; topic_links: ReleasePart[]; suggested_route: ReleasePart[];
  external_resources?: ExternalResource[];
}
export interface ReleaseReadiness {
  status: 'ready' | 'incomplete';
  materials: { published_book_ref: BookRef; available_capabilities: string[]; missing_capabilities: string[]; publication_available: boolean }[];
  audience: { user_id: string; ready: boolean; missing_grants: BookRef[] }[];
}
export interface WorkspaceRelease {
  archived?: boolean;
  release_id: string; version: number; latest_version: number; manifest: ReleaseManifest; created_at: number; publisher_user_id: string;
  allowed_tiers?: string[]; visible_to_users: string[]; readiness_at_publication: ReleaseReadiness; availability: ReleaseReadiness;
}
export const bookKey = (book: BookRef) => JSON.stringify([book.book_id, book.publication_id]);
export const bookLabel = (book: BookRef) => `${book.book_id} · ${book.publication_id}`;
export function releaseBody(data: FormData, materials: ReleaseMaterial[], topics: ReleasePart[], route: ReleasePart[], existing: WorkspaceRelease | null, externalResources: ExternalResource[] = []) {
  if (!materials.length) throw new Error('请至少选择一份确切版本的资料。');
  if (materials.some(material => !material.required_capabilities.length)) throw new Error('请为每份资料选择至少一种必要能力。');
  if ([...topics, ...route].some(part => !part.materials.length)) throw new Error('请为每条主题联系和建议路线选择关联资料。');
  const visible = String(data.get('visible_to_users') ?? '').split(/[\s,，]+/).filter(Boolean);
  const tiers = data.getAll('allowed_tiers').map(String);
  if (!visible.length && !tiers.length) throw new Error('请选择套餐开放范围或填写可见账号。');
  return {
    release_id: existing?.release_id ?? String(data.get('release_id') ?? '').trim(),
    expected_version: existing?.latest_version ?? 0,
    visible_to_users: [...new Set(visible)],
    allowed_tiers: [...new Set(tiers)],
    manifest: {
      name: String(data.get('name') ?? '').trim(), publisher: String(data.get('publisher') ?? '').trim(),
      public_goal: String(data.get('public_goal') ?? '').trim(), applicability: String(data.get('applicability') ?? '').trim(), prerequisites: String(data.get('prerequisites') ?? '').trim(),
      materials, topic_links: topics, suggested_route: route,
      external_resources: externalResources.map(resource => ({title:resource.title.trim(),url:resource.url.trim(),description:resource.description.trim()})),
    },
  };
}

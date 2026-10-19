import type { LearningWorkspace, PublishedBookRef } from './network-context';
import { bookTitle } from './book-title';

export interface SharedMaterial { published_book_ref: PublishedBookRef; required_capabilities: string[] }
export interface SharedPart { title: string; description: string; materials: PublishedBookRef[] }
export interface SharedManifest {
  name: string; publisher: string; public_goal: string; applicability: string; prerequisites: string;
  materials: SharedMaterial[]; topic_links: SharedPart[]; suggested_route: SharedPart[];
}
export interface SharedReadiness {
  status: 'ready' | 'incomplete';
  materials: { published_book_ref: PublishedBookRef; available_capabilities: string[]; missing_capabilities: string[]; publication_available: boolean }[];
  audience: { user_id: string; ready: boolean; missing_grants: PublishedBookRef[] }[];
}
export interface SharedRelease {
  release_id: string; version: number; latest_version: number; manifest: SharedManifest; created_at: number;
  archived: boolean; readiness_at_publication: SharedReadiness; availability: SharedReadiness;
}
export interface SharedReleasePage { items: SharedRelease[]; total: number; limit: number; offset: number }
export interface SharedInstance {
  release_id: string; version: number; latest_version: number; release: SharedRelease;
  history: { version: number; selected_at: number }[];
}
export interface SharedUseResult { workspace: LearningWorkspace; instance: SharedInstance }
export interface SharedUpdatePreview {
  instance: SharedInstance; release: SharedRelease;
  changes: { added_materials: PublishedBookRef[]; removed_materials: PublishedBookRef[]; public_goal_changed: boolean; composition_changed: boolean; required_capabilities_changed?: boolean; description_changed?: boolean };
  preserved_private_materials: PublishedBookRef[];
}
export interface SharedBook { published_book_ref: PublishedBookRef; title?: string | null }
export const sharedBookKey = (reference: PublishedBookRef) => JSON.stringify([reference.book_id, reference.publication_id]);
export function sharedBookLabel(reference: PublishedBookRef, books: SharedBook[], materials: SharedMaterial[]) {
  const book = books.find(book => sharedBookKey(book.published_book_ref) === sharedBookKey(reference));
  const index = materials.findIndex(material => sharedBookKey(material.published_book_ref) === sharedBookKey(reference));
  return book ? bookTitle({ title: book.title, book_id: reference.book_id }) : (index >= 0 ? `资料 ${index + 1}` : '原版本资料');
}
export const sharedCapability = (value: string) => ({ text: '阅读原文', tutor: '资料理解', teaching: '教学材料' }[value] ?? '资料内容');

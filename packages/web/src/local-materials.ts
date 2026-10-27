import { t as uiText } from './i18n';
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { network } from './network-context';
import { accountStorage, preparations, preparationUpload, type PreparationUpload } from './personal-preparations';

export interface MaterialScope { account_id: string; workspace_id: string }
export interface LocalMaterial {
  material_id: string; filename: string; bytes: number; source_path: string;
  scope: MaterialScope; workspace_name: string; imported_at: number;
  cloud_copy: null | { scope: MaterialScope; upload_id: string; task_id: string; operation_id: string; complete: boolean };
}
export interface PdfAnnotation {
  id: string; page_index: number; quote: string; rects: [number, number, number, number][]; color: string; note: string;
}
export interface PdfPageText { page_index: number; text: string; ocr_lines: import('../../core/src/pdf-ocr').PdfOcrLine[] }
export interface PdfReading { annotations: PdfAnnotation[]; page_index: number; pages: PdfPageText[] }
export type PdfReadingUpdate = { kind: 'page'; page: PdfPageText } | { kind: 'annotation'; annotation: PdfAnnotation }
  | { kind: 'delete_annotation'; id: string } | { kind: 'position'; page_index: number } | { kind: 'source_complete' };
declare global { interface Window { __UNDERSTAND_BOOK_DESKTOP__?: { localMaterials: number } } }
export const localMaterialsSupported = () => isTauri() && window.__UNDERSTAND_BOOK_DESKTOP__?.localMaterials === 1;
export const localMaterialUrl = (item: LocalMaterial) => convertFileSrc(item.material_id, 'local-material');
export const materialSize = (size: number) => size < 1024 ? `${size} B` : size < 1024 ** 2 ? `${(size / 1024).toFixed(1)} KB` : size < 1024 ** 3 ? `${(size / 1024 ** 2).toFixed(1)} MB` : `${(size / 1024 ** 3).toFixed(2)} GB`;
export const localMaterials = {
  list: (scope: MaterialScope | null = null) => invoke<LocalMaterial[]>('desktop_local_materials', { scope }),
  import: (path: string, scope: MaterialScope, workspaceName: string) => invoke<LocalMaterial>('desktop_import_local_material', { path, scope, workspaceName }),
  async choose(scope: MaterialScope, workspaceName: string) {
    const path = await open({ multiple: false, directory: false, title: uiText('导入到本机'), filters: [{ name: uiText('学习资料'), extensions: ['epub', 'md', 'txt', 'pdf', 'mp4', 'ppt', 'pptx'] }] });
    return typeof path === 'string' ? this.import(path, scope, workspaceName) : null;
  },
  remove: (materialId: string) => invoke<void>('desktop_remove_local_material', { materialId }),
  openOriginal: (materialId: string) => invoke<void>('desktop_open_local_material', { materialId }),
  openLibrary: () => invoke<void>('desktop_open_local_library'),
  pdfReading: (materialId: string) => invoke<PdfReading>('desktop_pdf_reading', { materialId }),
  pdfUpdate: (materialId: string, update: PdfReadingUpdate) => invoke<void>('desktop_pdf_update', { materialId, update }),
};

export class CloudSyncUnavailable extends Error {}

/** Only called by the explicit cloud-copy action. Local import never calls this. */
export async function syncLocalMaterial(item: LocalMaterial, scope: MaterialScope, progress: (bytes: number) => void,
  onSession: (session: PreparationUpload) => void) {
  const active = () => {
    if (network.value.identity?.user_id !== scope.account_id) throw new Error(uiText('账号已切换，请回到原账号继续同步。'));
  };
  active();
  const storage = await accountStorage.read(); active();
  const copy = item.cloud_copy?.scope.account_id === scope.account_id && item.cloud_copy.scope.workspace_id === scope.workspace_id ? item.cloud_copy : null;
  const saved = copy && storage.items.find(entry => entry.storage_id === copy.upload_id && entry.workspace_id === scope.workspace_id);
  if (saved?.task_id) return; // A previous response may have been lost after the cloud copy was committed.
  if (!saved && !storage.can_upload) throw new CloudSyncUnavailable(uiText('同步到云端需要有效套餐和足够的云端空间。本机资料仍可正常打开。'));
  if (!saved && (item.bytes > storage.available_bytes || storage.max_file_bytes !== null && item.bytes > storage.max_file_bytes)) {
    throw new CloudSyncUnavailable(uiText('这份资料超过了当前云端剩余空间或单文件上限。本机副本已保留。'));
  }
  const updated = await invoke<LocalMaterial>('desktop_local_material_cloud_copy', {
    materialId: item.material_id, scope, complete: false, restart: !!copy && !saved,
  });
  active();
  const cloud = updated.cloud_copy!;
  const session = { ...preparationUpload(cloud.upload_id), task_id: cloud.task_id, operation_id: cloud.operation_id };
  onSession(session);
  // Read bounded native chunks, so a long video is never loaded into JS memory in full.
  const source = {
    name: item.filename, size: item.bytes,
    slice(start: number, end: number) { return { async arrayBuffer() {
      active();
      const encoded = await invoke<string>('desktop_read_local_material_chunk', { materialId: item.material_id, offset: start, length: end - start });
      active();
      return Uint8Array.from(atob(encoded), character => character.charCodeAt(0)).buffer;
    } }; },
  };
  await preparations.upload(scope.workspace_id, source, progress, session);
  active();
  await invoke('desktop_local_material_cloud_copy', { materialId: item.material_id, scope, complete: true, restart: false });
}

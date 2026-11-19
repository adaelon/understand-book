import { hostFetch as networkFetch } from './reader-host';
import type { CourseOverview } from './generated/CourseOverview';
import type { CourseProgress } from './generated/CourseProgress';
import type { CourseLessonView } from './generated/CourseLessonView';
import type { CourseMutation } from './generated/CourseMutation';
import type { BoardSourceReadResponse, SourceWindowRequest } from './api';

export interface CourseCatalog {
  release_id: string; release_version: number; course: CourseOverview | null; progress: CourseProgress[];
}
const path = (workspace: string) => workspace.startsWith('local-course:')
  ? `/book/course/${encodeURIComponent(workspace.slice('local-course:'.length))}`
  : `/learning-workspaces/${encodeURIComponent(workspace)}/shared-release/course`;
export const inquiryCourse = {
  catalog: (workspace: string) => networkFetch<CourseCatalog>('GET', path(workspace)),
  read: (workspace: string, run: string) => networkFetch<CourseLessonView>('GET', `${path(workspace)}?run_id=${encodeURIComponent(run)}`),
  readSource: (workspace: string, run: string, options: SourceWindowRequest) => {
    const query = new URLSearchParams({ run_id: run, source: '1' });
    for (const [key, value] of Object.entries(options)) {
      if (value !== undefined) query.set(key, String(value));
    }
    return networkFetch<BoardSourceReadResponse>('GET', `${path(workspace)}?${query}`);
  },
  mutate: (workspace: string, request: CourseMutation) => networkFetch<CourseLessonView>('POST', path(workspace), request),
};

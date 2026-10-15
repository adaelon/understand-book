import type { PresentationRef } from './generated/PresentationRef';

export interface PresentationWorkspace {
  turnId: string;
  reference: PresentationRef;
  suspended?: boolean;
}

export function presentationKey(turnId: string, reference: PresentationRef): string {
  return `${turnId}:${reference.presentation_id}:${reference.revision}`;
}

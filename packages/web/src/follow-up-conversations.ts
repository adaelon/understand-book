/** Half-open UTF-16 offsets in the delivered Markdown parts, before rendering. */
export interface FollowUpAnchor {
  start_part: number;
  start_offset: number;
  end_part: number;
  end_offset: number;
  excerpt: string;
}

export interface FollowUpStart {
  parent_session_id: string;
  parent_turn_id: string;
  anchor: FollowUpAnchor;
  question: string;
  client_request_id: string;
}

export interface FollowUpOrigin extends FollowUpStart {
  parent_question: string;
  publication?: import('./reader-context').PublishedBookRef | null;
}

export interface FollowUpNode {
  session_id: string;
  title: string;
  origin?: FollowUpOrigin | null;
  question: string;
  answer?: string | null;
  status: 'ready' | 'pending' | 'failed' | 'unsent';
}

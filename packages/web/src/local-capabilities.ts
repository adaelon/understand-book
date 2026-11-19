export interface LocalCapabilities {
  noteDrafts: boolean;
  managedBuildEngine: boolean;
}

declare global {
  interface Window { __UNDERSTAND_BOOK_LOCAL__?: LocalCapabilities }
}

/** The native host declares implemented commands; a browser has no native store. */
export function localCapabilities(): Readonly<LocalCapabilities> | undefined {
  return window.__UNDERSTAND_BOOK_LOCAL__;
}

// Large pending requests (notably score screenshots) cannot fit in sessionStorage.
// Keep their body in IndexedDB and their small, tab-scoped recovery key in sessionStorage.
function bodyRequest<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    const opening = indexedDB.open('understand-book-submissions', 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore('bodies');
    opening.onerror = () => reject(opening.error);
    opening.onsuccess = () => {
      const db = opening.result;
      const transaction = db.transaction('bodies', mode);
      let request: IDBRequest<T>;
      try { request = operation(transaction.objectStore('bodies')); }
      catch (error) { transaction.abort(); db.close(); reject(error); return; }
      transaction.oncomplete = () => { db.close(); resolve(request.result); };
      transaction.onabort = () => { db.close(); reject(transaction.error ?? request.error); };
    };
  });
}

export function saveSubmissionBody(key: string, body: Record<string, unknown>) {
  return bodyRequest('readwrite', store => store.put(body, key));
}
export function readSubmissionBody(key: string): Promise<Record<string, unknown> | undefined> {
  return bodyRequest('readonly', store => store.get(key));
}
export function discardSubmissionBody(key: string) {
  // Cleanup cannot turn a successfully accepted question into a failed submission.
  void bodyRequest('readwrite', store => store.delete(key)).catch(() => {});
}

import { ApiError } from "./api";

export interface Invite {
  invite_id: string;
  code: string;
  actor: string;
  operation_id: string;
  state: "unused" | "used" | "disabled";
  created_at: number;
  used_by: string | null;
  used_email: string | null;
  used_at: number | null;
  disabled_at: number | null;
}
export interface InviteBatch {
  operation_id: string;
  actor: string;
  count: number;
  created_at: number;
  invites: Invite[];
}
interface PendingBatch { operation_id: string; count: number }
type Request = <T>(path: string, method?: string, body?: unknown) => Promise<T>;
export const formatInvite = (code: string) => code.match(/.{1,5}/g)?.join("-") ?? code;

// Batch writes precede any target reader: keep them separate from account operations.
export class InviteJournal {
  private key: string;
  constructor(actor: string, private storage: Storage, private request: Request) {
    this.key = `understand-book:admin-invite-batch:${actor}`;
  }
  pending(): PendingBatch | null {
    const raw = this.storage.getItem(this.key);
    return raw ? JSON.parse(raw) : null;
  }
  prepare(count: number) {
    if (this.pending()) throw new Error("请先核对上一批内测码。");
    if (!Number.isInteger(count) || count < 1 || count > 100)
      throw new Error("每批可生成 1–100 个内测码。");
    this.storage.setItem(this.key, JSON.stringify({ operation_id: crypto.randomUUID(), count }));
  }
  async lookup(): Promise<InviteBatch | null> {
    const pending = this.pending();
    if (!pending) return null;
    try {
      const result = await this.request<InviteBatch>(`/admin/invite-batches/${encodeURIComponent(pending.operation_id)}`);
      this.storage.removeItem(this.key);
      return result;
    } catch (failure) {
      if (failure instanceof ApiError && failure.status === 404) return null;
      throw failure;
    }
  }
  async send(): Promise<InviteBatch> {
    const pending = this.pending();
    if (!pending) throw new Error("没有待提交批次。");
    try {
      const result = await this.request<InviteBatch>("/admin/invite-batches", "POST", pending);
      this.storage.removeItem(this.key);
      return result;
    } catch (failure) {
      // Only invalid input proves this new batch was rejected. Conflict/unknown results need lookup.
      if (failure instanceof ApiError && failure.status === 400)
        this.storage.removeItem(this.key);
      throw failure;
    }
  }
  async recover(retry: boolean): Promise<InviteBatch | null> {
    const result = await this.lookup();
    return result ?? (retry ? this.send() : null);
  }
}

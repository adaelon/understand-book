import { ApiError } from "./api";

export interface Operation {
  operation_id: string;
  path: string;
  body: Record<string, unknown>;
  title: string;
  passwordRequired: boolean;
}
export interface Receipt {
  operation_id: string;
  actor: string;
  user_id: string;
  kind: string;
  created_at: number;
  ok: boolean;
  [key: string]: unknown;
}
type Request = <T>(path: string, method?: string, body?: unknown) => Promise<T>;
// One unresolved operation per account and browser tab. Reload keeps its original identity.
export class OperationJournal {
  private key: string;
  constructor(
    owner: string,
    private storage: Storage,
    private request: Request,
  ) {
    this.key = `understand-book:admin-operation:${owner}`;
  }
  pending(): Operation | null {
    const raw = this.storage.getItem(this.key);
    return raw ? JSON.parse(raw) : null;
  }
  prepare(
    title: string,
    path: string,
    body: Record<string, unknown>,
  ): Operation {
    if (this.pending()) throw new Error("请先核对上一笔操作。");
    const { password, ...safe } = body;
    const operation = {
      title,
      path,
      body: safe,
      operation_id: crypto.randomUUID(),
      passwordRequired: typeof password === "string",
    };
    // If storage is unavailable, stop before any write can reach the server.
    this.storage.setItem(this.key, JSON.stringify(operation));
    return operation;
  }
  async lookup(): Promise<Receipt | null> {
    const operation = this.pending();
    if (!operation) return null;
    try {
      const receipt = await this.request<Receipt>(
        `/admin/operations/${encodeURIComponent(operation.operation_id)}`,
      );
      this.storage.removeItem(this.key);
      return receipt;
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) return null;
      throw error;
    }
  }
  async send(password?: string): Promise<Receipt> {
    const operation = this.pending();
    if (!operation) throw new Error("没有待提交操作。");
    if (operation.passwordRequired && !password)
      throw new Error("请重新输入本次设置的密码；密码不会保存到浏览器。");
    try {
      const receipt = await this.request<Receipt>(operation.path, "POST", {
        ...operation.body,
        ...(operation.passwordRequired ? { password } : {}),
        operation_id: operation.operation_id,
      });
      this.storage.removeItem(this.key);
      return receipt;
    } catch (error) {
      // These responses prove the transaction was rejected. Unknown outcomes keep the key.
      if (
        error instanceof ApiError &&
        [400, 404, 409].includes(error.status) &&
        error.code !== "CLIENT_IDENTITY_STALE"
      )
        this.storage.removeItem(this.key);
      throw error;
    }
  }
}

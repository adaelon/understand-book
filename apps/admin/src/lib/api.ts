import { QueryClient } from "@tanstack/react-query";

export interface Identity {
  user_id: string;
  email?: string | null;
  csrf_token: string;
  capabilities: { admin: boolean };
}
export interface AuthState {
  identity: Identity | null;
  epoch: number;
  status: "checking" | "ready" | "error";
  error: string;
}
export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}
const messages: Record<string, string> = {
  BUILD_BUDGET_EXHAUSTED: "本任务剩余预算不足。请核对待核算费用，或提高预算后继续。",
  WORKSPACE_RELEASE_INVALID: "请填写完整公开编排，选择确切资料版本、必要能力和可见账号。",
  WORKSPACE_RELEASE_VERSION_CONFLICT: "已有更新版本，请刷新发布列表后重新确认公开编排。",
  WORKSPACE_RELEASE_READER_UNAVAILABLE: "所选可见账号不可用，请核对账号状态。",
  WORKSPACE_RELEASE_STORAGE_FAILED: "发布结果暂未确认，请核对原操作回执后重试。",
  MCP_PACK_REVISION_CONFLICT: "调用包已被更正，请重新读取后确认金额与次数。",
  MCP_PACK_ALREADY_CONSUMED: "只能扣回尚未消费的次数，请刷新调用包后重新填写。",
  MCP_USAGE_INPUT_INVALID: "赠送次数须为正整数，并填写原因。",
  MCP_USAGE_STORAGE_UNAVAILABLE: "MCP 操作结果尚未确认，请核对原操作回执后重试。",
  RECEIPT_CORRECTION_EXCEEDS_AMOUNT: "收款更正后的金额不能小于零。",
  USER_UNAVAILABLE: "账号已停用或不可用，请先核对账号状态。",
  INVITE_BATCH_CONFLICT: "原操作号的生成数量不同，请核对原批次。",
  INVITE_ALREADY_USED: "内测码已被使用，不能停用。请刷新列表查看使用账号。",
  INVITE_STORAGE_UNAVAILABLE: "内测码操作暂未确认，请核对原批次后重试。",
  ALLOWANCE_BASIS_UNBOUND: "请先为该额度期配置百分比基准。",
  ALLOWANCE_BASIS_ALREADY_BOUND: "该额度期已绑定固定基准，不能更改。",
  ALLOWANCE_BASIS_VERSION_CONFLICT: "保存的额度换算配置发生冲突。请刷新页面后重新提交预算金额。",
  CLOUD_PLAN_PERIOD_IMMUTABLE: "已开通套餐的期限不能直接修改。需要续期时，请选择套餐并登记续费。",
  CLOUD_PLAN_UNLIMITED: "账号仍有内测不限额度，请先结束内测有效期，再开通收费套餐。",
  ALLOWANCE_PERCENT_NOT_REPRESENTABLE: "该百分比不能换算为完整内部额度，请调整输入。",
  ALLOWANCE_REVISION_CONFLICT: "额度期已改变，请重新读取后确认。",
  CHARGE_REVISION_CONFLICT: "费用记录已改变，请重新读取后确认。",
  ALLOWANCE_PERIOD_OVERLAP: "有效期与已有额度期重叠，请调整起止时间。",
  ALLOWANCE_ALREADY_COMMITTED: "下调金额超过未被扣减或占用的额度。",
  RECEIPT_EXTERNAL_REF_CONFLICT: "该渠道的外部交易编号已登记，请核对原收款。",
  ADMIN_OPERATION_CONFLICT: "原操作号的参数不一致，请核对原回执。",
  USER_ID_INVALID:
    "账号只能包含英文字母、数字、短横线和下划线，最多 128 个字符。",
  INVALID_REQUEST: "填写内容不符合要求，请核对金额、有效期和必填字段。",
};
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false },
    mutations: { retry: false },
  },
});

// The epoch belongs to the Cookie session, not to a ReaderWorkspace or a target reader.
export class AdminSession {
  private state: AuthState = {
    identity: null,
    epoch: 0,
    status: "checking",
    error: "",
  };
  private listeners = new Set<() => void>();
  private check = 0;
  constructor(
    private transport: typeof fetch,
    private clear: () => void,
  ) {}
  snapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(value: AuthState) {
    this.state = value;
    this.listeners.forEach((listener) => listener());
  }
  invalidate(status: AuthState["status"] = "checking") {
    this.check++;
    this.clear();
    this.publish({
      identity: null,
      epoch: this.state.epoch + 1,
      status,
      error: "",
    });
  }
  async refresh() {
    const ticket = ++this.check;
    try {
      const response = await this.transport("/api/auth/me", {
        credentials: "same-origin",
        cache: "no-store",
      });
      const value = await response.json();
      if (ticket !== this.check) return;
      if (!response.ok && response.status !== 401)
        throw new ApiError(response.status, value.error_code, value.message);
      const identity: Identity | null = response.ok ? value : null;
      const old = this.state.identity;
      const changed =
        old?.user_id !== identity?.user_id ||
        old?.csrf_token !== identity?.csrf_token ||
        old?.capabilities.admin !== identity?.capabilities.admin;
      if (changed) this.clear();
      this.publish({
        identity,
        epoch: this.state.epoch + Number(changed),
        status: "ready",
        error: "",
      });
    } catch (error) {
      if (ticket === this.check) {
        this.clear();
        this.publish({
          identity: null,
          epoch: this.state.epoch + 1,
          status: "error",
          error: "无法确认登录状态，请检查连接后重试。",
        });
      }
    }
  }
  async request<T>(
    path: string,
    method = "GET",
    body?: unknown,
    signal?: AbortSignal,
  ): Promise<T> {
    const { epoch, identity } = this.state;
    const response = await this.transport(`/api${path}`, {
      method,
      credentials: "same-origin",
      cache: "no-store",
      signal,
      headers: {
        "Content-Type": "application/json",
        ...(identity && method !== "GET"
          ? { "X-CSRF-Token": identity.csrf_token }
          : {}),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    const value = await response.json();
    if (epoch !== this.state.epoch)
      throw new ApiError(
        409,
        "CLIENT_IDENTITY_STALE",
        "登录身份已改变，请重新确认。",
      );
    if (!response.ok) {
      if (response.status === 401) this.invalidate("ready");
      else if (value.error_code === "CSRF_REJECTED") await this.refresh();
      else if (response.status === 403) {
        this.invalidate();
        await this.refresh();
      }
      throw new ApiError(
        response.status,
        value.error_code ?? `HTTP_${response.status}`,
        value.error_code === "CSRF_REJECTED"
          ? "登录凭证已更新，请重新确认操作；尚未自动重试。"
          : (messages[value.error_code] ??
              `${value.message ?? "请求未完成"} (${value.error_code ?? response.status})`),
      );
    }
    return value;
  }
}
export const session = new AdminSession(
  (...args) => fetch(...args),
  () => {
    void queryClient.cancelQueries();
    queryClient.clear();
  },
);
let channel: BroadcastChannel | null = null;
export function startIdentitySync() {
  channel =
    typeof BroadcastChannel === "undefined"
      ? null
      : new BroadcastChannel("understand-book:authentication");
  const changed = (event: MessageEvent) => {
    if (event.data === "profile-changed") { void session.refresh(); void queryClient.invalidateQueries(); return; }
    session.invalidate();
    void session.refresh();
  };
  const visible = () => {
    if (document.visibilityState !== "hidden") void session.refresh();
  };
  channel?.addEventListener("message", changed);
  window.addEventListener("focus", visible);
  document.addEventListener("visibilitychange", visible);
  void session.refresh();
  return () => {
    channel?.close();
    channel = null;
    window.removeEventListener("focus", visible);
    document.removeEventListener("visibilitychange", visible);
  };
}
export async function login(username: string, password: string) {
  session.invalidate();
  await session.request("/auth/login", "POST", { username, password });
  channel?.postMessage("changed");
  await session.refresh();
}
export async function logout() {
  // Capture the old CSRF before clearing the private UI.
  const token = session.snapshot().identity?.csrf_token;
  session.invalidate("ready");
  try {
    const response = await fetch("/api/auth/logout", {
      method: "POST",
      credentials: "same-origin",
      cache: "no-store",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": token ?? "",
      },
      body: "{}",
    });
    if (!response.ok) throw new Error("logout");
  } catch {
    throw new Error("退出请求未确认，请检查连接后重新登录。");
  } finally {
    channel?.postMessage("changed");
  }
}

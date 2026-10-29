import {
  createContext,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { ApiError, queryClient, session } from "@/lib/api";
import { OperationJournal, type Receipt } from "@/lib/operations";
import { percentage, milliPercentage } from "@/lib/allowance";
import type { Period } from "@/lib/types";
import { date, money } from "@/lib/format";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

interface Operations {
  busy: boolean;
  blocked: boolean;
  submit: (
    title: string,
    path: string,
    body: Record<string, unknown>,
  ) => Promise<boolean>;
}
const Context = createContext<Operations | null>(null);
const fieldNames: Record<string, string> = {
  user_id: "账号",
  period_id: "额度期",
  receipt_id: "原收款",
  pack_count: "购买包数",
  units: "赠送次数（次）",
  delta_units: "次数差额（次）",
  amount_fen: "实际收款（元）",
  delta_fen: "收款差额（元）",
  delta_micro_cny: "内部额度差额（元）",
  delta_milli_percent: "增加 / 调整额度",
  basis_version: "固定基准版本",
  title: "方案名称",
  instructions: "收款说明",
  enabled: "启用方案",
  provider_cost_micro_cny: "供应商成本（元）",
  starts_at: "开始时间",
  expires_at: "到期时间",
  paid_at: "付款时间",
  channel: "渠道",
  external_ref: "外部交易编号",
  note: "备注",
  reason: "原因",
  evidence: "核算依据",
  disabled: "停用账号",
  waive_account: "账号免扣",
  published_book_ref: "材料发布",
  release_id: "共享发布标识",
  expected_version: "提交时的最新版本",
  visible_to_users: "可见账号",
  archived: "归档共享入口",
  task_id: "运营任务",
  name: "名称",
  max_micro_cny: "平台预算上限（元）",
  reserved_per_execution_micro_cny: "每次执行预计占用（元）",
  expected_revision: "提交时的任务版本",
  product_id: "套餐商品",
  product_revision: "购买商品版本",
  tier: "套餐档位",
  duration: "套餐时长",
  test_only: "仅供测试",
  grant_milli_percent: "每期套餐额度",
  storage_bytes: "个人空间（字节）",
  max_file_bytes: "单文件上限（字节）",
  retention_seconds: "到期保留（秒）",
};
function SubmissionSummary({ body }: { body: Record<string, unknown> }) {
  return (
    <dl className="my-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
      {typeof body.pack_count === "number" && <>
        <dt>确认收款 / 开通次数</dt>
        <dd>{money(body.pack_count * 1000, 2)} 元 / {body.pack_count * 1000} 次</dd>
      </>}
      {Object.entries(body)
        .filter(([key, value]) => fieldNames[key] && !(key === "max_micro_cny" && value == null))
        .map(([key, value]) => {
          const text = value == null ? '未填写' : key === "tier" ? ({ basic: '基础版', plus: '进阶版', pro: '专业版' }[String(value)] ?? String(value))
            : key === "duration" ? ({ day1: '1 天冲刺', day3: '3 天冲刺', day7: '7 天冲刺', month: '按月学习' }[String(value)] ?? String(value))
            : key === "delta_milli_percent" || key === "grant_milli_percent" ? milliPercentage(value as number) : key.endsWith("_fen")
            ? money(value as number, 2)
            : key.endsWith("_micro_cny")
              ? money(value as number | null)
              : key.endsWith("_at")
                ? date(value as number)
                : typeof value === "boolean"
                  ? value
                    ? "是"
                    : "否"
                  : key === "published_book_ref"
                    ? `${(value as { book_id: string }).book_id} · ${(value as { publication_id: string }).publication_id}`
                    : String(value ?? "未填写");
          return (
            <div className="contents" key={key}>
              <dt>{fieldNames[key]}</dt>
              <dd className="break-all">{text}</dd>
            </div>
          );
        })}
      {body.manifest != null && (() => {
        const manifest = body.manifest as { name: string; public_goal: string; materials: { published_book_ref: { book_id: string; publication_id: string }; required_capabilities: string[] }[] };
        return <><dt>共享工作区名称</dt><dd>{manifest.name}</dd><dt>公开目标</dt><dd>{manifest.public_goal}</dd><dt>资料版本</dt><dd>{manifest.materials.map(material => `${material.published_book_ref.book_id} · ${material.published_book_ref.publication_id}（${material.required_capabilities.join('、')}）`).join('；')}</dd></>;
      })()}
    </dl>
  );
}
export const useOperations = () => useContext(Context)!;
export function OperationsProvider({ children }: { children: ReactNode }) {
  const { identity, epoch } = session.snapshot();
  const journal = useMemo(
    () =>
      new OperationJournal(
        identity!.user_id,
        sessionStorage,
        <T,>(path: string, method?: string, body?: unknown) => {
          if (session.snapshot().epoch !== epoch)
            throw new ApiError(409, "CLIENT_IDENTITY_STALE", "身份已改变");
          return session.request<T>(path, method, body);
        },
      ),
    [identity!.user_id, epoch],
  );
  const [pending, setPending] = useState(() => journal.pending());
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [password, setPassword] = useState("");
  const running = useRef(false);
  function success(result: Receipt) {
    setReceipt(result);
    setPending(null);
    setError("");
    void queryClient.invalidateQueries();
  }
  async function execute(action: () => Promise<Receipt | null>) {
    if (running.current) return false;
    running.current = true;
    setBusy(true);
    setError("");
    try {
      const result = await action();
      if (result) {
        success(result);
        return true;
      }
      setError("暂未查到回执。可按原操作号重试，系统不会重复提交业务。");
      return false;
    } catch (failure) {
      setError(
        failure instanceof Error
          ? failure.message
          : "提交结果尚未确认，请核对原操作。",
      );
      setPending(journal.pending());
      if (failure instanceof ApiError && failure.status === 409)
        void queryClient.invalidateQueries();
      return false;
    } finally {
      running.current = false;
      setBusy(false);
      setPassword("");
    }
  }
  const submit = (title: string, path: string, body: Record<string, unknown>) =>
    execute(async () => {
      setReceipt(null);
      const op = journal.prepare(title, path, body);
      setPending(op);
      try {
        return await journal.send(body.password as string | undefined);
      } catch (failure) {
        if (journal.pending()) {
          const found = await journal.lookup();
          if (found) return found;
        }
        throw failure;
      }
    });
  return (
    <Context.Provider value={{ busy, blocked: !!pending, submit }}>
      {pending && (
        <section
          className="mb-6 rounded-lg border border-amber-500 bg-amber-50 p-4 text-slate-900"
          aria-label="待核对操作"
        >
          <h2>有一笔操作待核对：{pending.title}</h2>
          <p className="my-2 break-all text-sm">
            操作号：{pending.operation_id}
          </p>
          <p className="text-sm">
            刷新后保留原操作；请先查询回执，再决定是否重试。
          </p>
          <SubmissionSummary body={pending.body} />
          {pending.passwordRequired && (
            <Field label="本次设置的密码">
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="new-password"
              />
            </Field>
          )}
          <div className="mt-3 flex flex-wrap gap-2">
            <Button
              disabled={busy}
              onClick={() => void execute(() => journal.lookup())}
            >
              核对原操作
            </Button>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() =>
                void execute(
                  async () =>
                    (await journal.lookup()) ?? (await journal.send(password)),
                )
              }
            >
              按原操作重试
            </Button>
          </div>
        </section>
      )}
      {error && (
        <p
          role="alert"
          className="mb-4 rounded-md border border-destructive p-3 text-destructive"
        >
          {error}
        </p>
      )}
      {receipt && (
        <section
          role="status"
          className="mb-5 rounded-md border bg-muted/40 p-4"
        >
          <strong>操作已完成 · {receipt.user_id}</strong>
          <p className="break-all text-sm">
            回执 {receipt.operation_id} · {date(receipt.created_at)}
          </p>
          {receipt.allowance_period != null && (
            <p>
              提交后可用额度：
              {(receipt.allowance_period as Period).balance.unlimited ? '内测不限额度' : percentage((receipt.allowance_period as Period).balance.percentages?.available)}

            </p>
          )}
          {receipt.parameters && !receipt.kind.startsWith("mcp-") && <SubmissionSummary body={receipt.parameters} />}
          {receipt.receipt_id != null && (
            <p className="break-all text-sm">
              收款记录：{String(receipt.receipt_id)}
            </p>
          )}
          {receipt.workspace_release != null && (() => { const release = receipt.workspace_release as {release_id: string; version: number; readiness_at_publication:{status:string}}; return <p>共享发布 {release.release_id} · v{release.version}：{release.readiness_at_publication.status === 'ready' ? '完整可用' : '尚未完整就绪，请查看版本中的资料与授权情况。'}</p>; })()}
          {receipt.operator_build != null && <p>运营任务已保存，可打开任务查看最新准备状态与费用。</p>}
          {receipt.entitlement != null && (() => {
            const plan = receipt.entitlement as { title: string; starts_at: number; expires_at: number };
            return <p>已开通 {plan.title}：{date(plan.starts_at)} — {date(plan.expires_at)}（香港时间）。</p>;
          })()}
          {receipt.kind.startsWith("mcp-") && <>
            {receipt.parameters && <SubmissionSummary body={receipt.parameters} />}
            {receipt.call_pack && <p>
              原购 {receipt.call_pack.pack_count} 包 · {money(receipt.call_pack.price_fen, 2)} 元 · {receipt.call_pack.total_units} 次；
              累计次数调整 {receipt.call_pack.adjusted_units} 次，已用 {receipt.call_pack.used_units} 次，
              操作完成时未用 {receipt.call_pack.remaining_units} 次 / 可用 {receipt.call_pack.available_units} 次。<br />
              到期：{date(Date.parse(receipt.call_pack.expires_at) / 1000)}（香港时间）
            </p>}
            {receipt.usage && <p>
              赠送月份：{receipt.month_hk}；操作完成时免费剩余 {receipt.usage.free_remaining} 次，已购剩余 {receipt.usage.paid_remaining} 次。
            </p>}
            <p className="text-sm text-muted-foreground">以上为原操作回执，当前余额以刷新后的 MCP 次数面板为准。</p>
          </>}
          {receipt.account_delta_micro_cny != null && (
            <p>
              本次账号扣减差额：{money(Number(receipt.account_delta_micro_cny))}{" "}
              元
            </p>
          )}
          <Button variant="ghost" onClick={() => setReceipt(null)}>
            收起回执
          </Button>
        </section>
      )}
      {children}
    </Context.Provider>
  );
}
export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <Label className="grid gap-2 py-2 text-sm">
      {label}
      {children}
    </Label>
  );
}
export function OperationDialog({
  title,
  description,
  children,
  onSubmit,
  open,
  onOpenChange,
  submitLabel = "确认提交",
}: {
  title: string;
  description: string;
  children?: ReactNode;
  onSubmit: (data: FormData) => Promise<boolean>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  submitLabel?: string;
}) {
  const ops = useOperations();
  const [error, setError] = useState("");
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (!ops.busy) {
          setError("");
          onOpenChange(value);
        }
      }}
    >
      <DialogContent className="max-h-[90dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <form
          onSubmit={async (event) => {
            event.preventDefault();
            setError("");
            const data = new FormData(event.currentTarget);
            try {
              await onSubmit(data);
              onOpenChange(false);
            } catch (failure) {
              setError(
                failure instanceof Error ? failure.message : String(failure),
              );
            }
          }}
        >
          <fieldset disabled={ops.busy || ops.blocked}>
            {children}
            <p role="alert" className="my-2 text-destructive">
              {error}
            </p>
            <Button className="mt-3 w-full" type="submit">
              {ops.busy ? "正在提交…" : submitLabel}
            </Button>
          </fieldset>
        </form>
      </DialogContent>
    </Dialog>
  );
}

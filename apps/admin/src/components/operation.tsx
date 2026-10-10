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
  amount_fen: "实际收款（元）",
  delta_fen: "收款差额（元）",
  delta_micro_cny: "授予 / 调整额度（元）",
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
};
function SubmissionSummary({ body }: { body: Record<string, unknown> }) {
  return (
    <dl className="my-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
      {Object.entries(body)
        .filter(([key]) => fieldNames[key])
        .map(([key, value]) => {
          const text = key.endsWith("_fen")
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
              {money(
                (
                  receipt.allowance_period as {
                    balance: { available_micro_cny: number };
                  }
                ).balance.available_micro_cny,
              )}{" "}
              元
            </p>
          )}
          {receipt.receipt_id != null && (
            <p className="break-all text-sm">
              收款记录：{String(receipt.receipt_id)}
            </p>
          )}
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
}: {
  title: string;
  description: string;
  children?: ReactNode;
  onSubmit: (data: FormData) => Promise<boolean>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
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
              {ops.busy ? "正在提交…" : "确认提交"}
            </Button>
          </fieldset>
        </form>
      </DialogContent>
    </Dialog>
  );
}

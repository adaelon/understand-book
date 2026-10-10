import { useState } from "react";
import { type Charge, type Page } from "@/lib/types";
import { date, money, moneyInput } from "@/lib/format";
import { DataState, DataTable, Pagination, useData } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const states: Record<string, string> = {
  reserved: "已预占",
  sent: "发送中",
  pending: "待核算",
  settled: "已结算",
  released: "已释放",
};
export function ChargesPanel({ userId }: { userId: string }) {
  const [offset, setOffset] = useState(0),
    [pending, setPending] = useState(false),
    [call, setCall] = useState<string | null>(null);
  const query = useData<Page<Charge>>(
    `/admin/charges?user_id=${encodeURIComponent(userId)}&pending=${pending}&limit=20&offset=${offset}`,
  );
  return (
    <section className="space-y-3 rounded-lg border p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>调用费用与待核算</h2>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={pending}
            onChange={(e) => {
              setPending(e.target.checked);
              setOffset(0);
            }}
          />
          仅看待核算
        </label>
      </div>
      <DataState query={query} />
      {query.data && (
        <>
          <DataTable
            rows={query.data.items}
            columns={[
              {
                header: "时间 / 模型",
                cell: ({ row }) => (
                  <>
                    {date(row.original.created_at)}
                    <br />
                    <span className="text-muted-foreground">
                      {row.original.model}
                    </span>
                  </>
                ),
              },
              {
                header: "状态",
                cell: ({ row }) => (
                  <>
                    {states[row.original.state] ?? row.original.state}
                    {row.original.needs_reconciliation ? " · 回执冲突" : ""}
                    {row.original.provider_cost_status === "pending"
                      ? " · 供应商成本未知"
                      : ""}
                  </>
                ),
              },
              {
                header: "预占（元）",
                cell: ({ row }) => money(row.original.reserved_micro_cny),
              },
              {
                header: "账号扣减（元）",
                cell: ({ row }) => money(row.original.account_debit_micro_cny),
              },
              {
                header: "供应商成本（元）",
                cell: ({ row }) => money(row.original.provider_cost_micro_cny),
              },
              {
                header: "操作",
                cell: ({ row }) => (
                  <Button
                    variant="outline"
                    onClick={() => setCall(row.original.call_id)}
                  >
                    查看 / 核算
                  </Button>
                ),
              },
            ]}
          />
          <Pagination
            offset={offset}
            total={query.data.total}
            change={setOffset}
          />
        </>
      )}
      {call && <ChargeDetail call={call} close={() => setCall(null)} />}
    </section>
  );
}
function ChargeDetail({ call, close }: { call: string; close: () => void }) {
  const [offset, setOffset] = useState(0),
    [reconcile, setReconcile] = useState<Charge | null>(null),
    [waive, setWaive] = useState(false);
  const query = useData<Charge>(
      `/admin/charges/${encodeURIComponent(call)}?limit=20&offset=${offset}`,
    ),
    ops = useOperations(),
    charge = query.data;
  return (
    <>
      <Dialog
        open={!reconcile}
        onOpenChange={(open) => {
          if (!open) close();
        }}
      >
        <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-3xl">
          <DialogHeader>
            <DialogTitle>调用费用详情</DialogTitle>
            <DialogDescription className="break-all">{call}</DialogDescription>
          </DialogHeader>
          <DataState query={query} />
          {charge && (
            <>
              <p>
                {charge.model} · {charge.purpose} · {date(charge.created_at)}
              </p>
              <p>
                {states[charge.state]} · 供应商成本{" "}
                {money(charge.provider_cost_micro_cny)} 元 · 账号扣减{" "}
                {money(charge.account_debit_micro_cny)} 元
              </p>
              <Button
                disabled={
                  ops.blocked || ["reserved", "sent"].includes(charge.state)
                }
                onClick={() => {
                  setWaive(false);
                  setReconcile(charge);
                }}
              >
                人工核算 / 追加更正
              </Button>
              <p className="text-sm text-muted-foreground">
                发送中的调用须等待结束。账号免扣可释放占用，供应商成本未知时仍保留未知。
              </p>
              {(
                [
                  ["计价用量", charge.provider_usage],
                  ["费率快照", charge.rate_snapshot],
                  ["预占估算依据", charge.reservation_estimate],
                ] as const
              ).map(([label, value]) => (
                <details key={label}>
                  <summary className="cursor-pointer font-medium">
                    {label}
                  </summary>
                  <pre className="mt-2 overflow-auto rounded bg-muted p-3 text-xs">
                    {value == null
                      ? "暂无回执"
                      : JSON.stringify(value, null, 2)}
                  </pre>
                </details>
              ))}
              <h2>已有发送回执</h2>
              {charge.reports.items.map((item, index) => (
                <pre
                  className="overflow-auto rounded bg-muted p-3 text-xs"
                  key={index}
                >
                  {JSON.stringify(item, null, 2)}
                </pre>
              ))}
              {!charge.reports.items.length && <p>暂无回执</p>}
              <h2>人工核算记录</h2>
              {charge.reconciliations.items.map((item, index) => (
                <details key={index}>
                  <summary>
                    {date(Number(item.created_at))} · {String(item.actor)} ·{" "}
                    {String(item.reason)}
                  </summary>
                  <p className="py-2 text-sm">依据：{String(item.evidence)}</p>
                  <pre className="overflow-auto rounded bg-muted p-3 text-xs">
                    {JSON.stringify(item, null, 2)}
                  </pre>
                </details>
              ))}
              {!charge.reconciliations.items.length && <p>暂无核算记录</p>}
              <Pagination
                offset={offset}
                total={Math.max(
                  charge.reports.total,
                  charge.reconciliations.total,
                )}
                change={setOffset}
              />
            </>
          )}
        </DialogContent>
      </Dialog>
      <OperationDialog
        title="人工核算 / 追加更正"
        description="按本次供应商账单或其他可核对依据确认费用，保留原记录和更正差额。"
        open={!!reconcile}
        onOpenChange={(open) => {
          if (!open) setReconcile(null);
        }}
        onSubmit={async (data) => {
          const result = await ops.submit(
            "人工核算费用",
            `/admin/charges/${encodeURIComponent(call)}/reconcile`,
            {
              revision: reconcile!.revision,
              provider_cost_micro_cny: String(data.get("cost")).trim()
                ? moneyInput(String(data.get("cost")))
                : null,
              waive_account: waive,
              reason: String(data.get("reason")),
              evidence: String(data.get("evidence")),
            },
          );
          close();
          return result;
        }}
      >
        <Field label="供应商成本（元）">
          <Input name="cost" inputMode="decimal" required={!waive} />
        </Field>
        <label className="my-3 flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={waive}
            onChange={(e) => setWaive(e.target.checked)}
          />
          账号免扣（释放占用）
        </label>
        {waive && (
          <p className="text-sm text-muted-foreground">
            不填供应商成本时保留原值；未知成本仍未知。
          </p>
        )}
        <Field label="处理原因">
          <Input name="reason" required />
        </Field>
        <Field label="核算依据">
          <textarea name="evidence" required rows={3} />
        </Field>
      </OperationDialog>
    </>
  );
}

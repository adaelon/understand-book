import { useState } from "react";
import { type Charge, type Page } from "@/lib/types";
import { date, money, moneyInput } from "@/lib/format";
import { DataState, DataTable, Pagination, useData } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Amount, Disclosure, Metric, StatusBadge } from '@/components/analytics';
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
function chargeService(charge: Charge): string {
  if (charge.entry_kind === 'web_search') return '搜索';
  if (charge.entry_kind === 'web_fetch') return '网页读取';
  if (charge.entry_kind === 'service') return charge.purpose === 'video_audio_review' ? '原声复核' : '视频转写';
  return charge.model ?? '模型';
}
export function ChargesPanel({ userId }: { userId: string }) {
  const [offset, setOffset] = useState(0),
    [pending, setPending] = useState(false),
    [call, setCall] = useState<string | null>(null);
  const query = useData<Page<Charge>>(
    `/admin/charges?user_id=${encodeURIComponent(userId)}&pending=${pending}&limit=20&offset=${offset}`,
  );
  return (
    <section className="analytics-panel">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>服务费用与待核算</h2>
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
                header: "时间 / 服务",
                cell: ({ row }) => (
                  <>
                    {date(row.original.created_at)}
                    <br />
                    <span className="text-muted-foreground">
                      {chargeService(row.original)}
                    </span>
                  </>
                ),
              },
              {
                header: "状态",
                cell: ({ row }) => (
                  <div className="max-w-48 space-y-1 whitespace-normal">
                    <StatusBadge tone={row.original.state === 'pending' || row.original.needs_reconciliation ? 'amber' : row.original.state === 'settled' ? 'green' : 'blue'}>{states[row.original.state] ?? row.original.state}</StatusBadge>
                    {row.original.needs_reconciliation && <p className="text-xs text-amber-600 dark:text-amber-300">回执冲突</p>}
                    {row.original.provider_cost_status === 'pending' && <p className="text-xs text-muted-foreground">供应商成本未知</p>}
                    {row.original.provider_cost_status === 'estimated' && <p className="text-xs text-muted-foreground">处理成本为估计</p>}
                  </div>
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
                header: "成本（元）",
                cell: ({ row }) => `${money(row.original.provider_cost_micro_cny)}${row.original.provider_cost_status === "estimated" ? "（估计）" : ""}`,
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
            <DialogTitle>服务费用详情</DialogTitle>
            <DialogDescription className="break-all">{call}</DialogDescription>
          </DialogHeader>
          <DataState query={query} />
          {charge && (
            <>
              <p>
                {chargeService(charge)} · {charge.purpose} · {date(charge.created_at)}
                {charge.entry_kind === "service" && <> · 实际处理 {charge.quantity_ms == null ? "待确认" : `${charge.quantity_ms / 1000} 秒`}</>}
              </p>
              <StatusBadge tone={charge.state === 'pending' ? 'amber' : charge.state === 'settled' ? 'green' : 'blue'}>{states[charge.state]}</StatusBadge>
              <dl className="grid gap-3 sm:grid-cols-2"><Metric label={charge.provider_cost_status === 'estimated' ? '处理成本估计' : charge.provider_cost_status === 'confirmed' ? '已确认成本' : '成本待核算'} value={<Amount value={charge.provider_cost_micro_cny} />} detail={charge.provider_cost_micro_cny == null ? '等待成本凭据确认' : `精确金额 ${money(charge.provider_cost_micro_cny)} 元`} /><Metric label="账号扣减" value={<Amount value={charge.account_debit_micro_cny} />} detail={charge.account_debit_micro_cny == null ? '扣减金额尚未确定' : `精确金额 ${money(charge.account_debit_micro_cny)} 元`} tone="green" /></dl>
              <Button
                disabled={
                  ops.blocked || ["reserved", "sent"].includes(charge.state) || charge.entry_kind === "service" && charge.state !== "pending"
                }
                onClick={() => {
                  setWaive(false);
                  setReconcile(charge);
                }}
              >
                {charge.entry_kind === "service" ? "核对实际处理用量" : "人工核算 / 追加更正"}
              </Button>
              <p className="text-sm text-muted-foreground">
                {charge.entry_kind === "service" ? "只根据处理日志确认实际音频时长或确认尚未启动。服务扣减沿原费率计算；没有实际成本凭据时仍保留未知或估计。" : "发送中的调用须等待结束。账号免扣可释放占用，供应商成本未知时仍保留未知。"}
              </p>
              {(
                [
                  ["计价用量", charge.entry_kind === "service" ? {quantity_ms:charge.quantity_ms,admitted_quantity_ms:charge.admitted_quantity_ms} : charge.provider_usage],
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
              <Disclosure title={`已有发送回执 · ${charge.reports.total} 条`}>
              {charge.reports.items.map((item, index) => (
                <pre
                  className="overflow-auto rounded bg-muted p-3 text-xs"
                  key={index}
                >
                  {JSON.stringify(item, null, 2)}
                </pre>
              ))}
              {!charge.reports.items.length && <p>暂无回执</p>}
              </Disclosure>
              <Disclosure title={`人工核算记录 · ${charge.reconciliations.total} 条`}>
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
              </Disclosure>
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
        title={reconcile?.entry_kind === "service" ? "核对实际处理用量" : "人工核算 / 追加更正"}
        description={reconcile?.entry_kind === "service" ? "根据执行日志填写实际处理音频时长，或确认该处理没有启动。运营核对不会把估计成本变成供应商账单。" : "按本次供应商账单或其他可核对依据确认费用，保留原记录和更正差额。"}
        open={!!reconcile}
        onOpenChange={(open) => {
          if (!open) setReconcile(null);
        }}
        onSubmit={async (data) => {
          if (reconcile!.entry_kind === "service") {
            const quantity = waive ? 0 : Math.round(Number(data.get("seconds")) * 1000);
            if (!Number.isSafeInteger(quantity) || quantity < 0 || quantity > (reconcile!.admitted_quantity_ms ?? 0)) throw new Error("实际处理时长不能超过获准音频范围");
            const result = await ops.submit("核对处理用量", `/admin/service-usage/${encodeURIComponent(call)}/reconcile`, {
              revision:reconcile!.revision,status:waive ? "not_started" : "completed",quantity_ms:quantity,
              reason:String(data.get("reason")),evidence:String(data.get("evidence")),
            });
            if (result) close();
            return result;
          }
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
          if (result) close();
          return result;
        }}
      >
        {reconcile?.entry_kind === "service" ? <Field label="实际处理音频时长（秒）"><Input name="seconds" type="number" min="0" step="0.001" max={(reconcile.admitted_quantity_ms ?? 0)/1000} required={!waive} /></Field> : <Field label="供应商成本（元）"><Input name="cost" inputMode="decimal" required={!waive} /></Field>}
        <label className="my-3 flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={waive}
            onChange={(e) => setWaive(e.target.checked)}
          />
          {reconcile?.entry_kind === "service" ? "有日志证据确认该处理尚未启动" : "账号免扣（释放占用）"}
        </label>
        {waive && (
          <p className="text-sm text-muted-foreground">
            {reconcile?.entry_kind === "service" ? "确认未启动后释放本次预占；用户继续时仅重试未完成的音频段。" : "不填供应商成本时保留原值；未知成本仍未知。"}
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

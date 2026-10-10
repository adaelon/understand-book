import { useState } from "react";
import { type Page, type Period } from "@/lib/types";
import { date, dateInput, money, moneyInput, timestamp } from "@/lib/format";
import { useData, DataState, DataTable, Pagination } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export function AllowancePanel({
  userId,
  current,
}: {
  userId: string;
  current: Period | null;
}) {
  const [action, setAction] = useState(""),
    [selected, setSelected] = useState<Period | null>(null),
    [offset, setOffset] = useState(0);
  const base = `/admin/users/${encodeURIComponent(userId)}`,
    ops = useOperations();
  const periods = useData<Page<Period>>(
    `${base}/allowance-periods?limit=20&offset=${offset}`,
  );
  const now = Math.floor(Date.now() / 1000);
  const open = (label: string, period: Period | null = current) => {
    setSelected(period);
    setAction(label);
  };
  const submit = async (data: FormData) => {
    const text = (key: string) => String(data.get(key) ?? "").trim();
    let path: string, body: Record<string, unknown>;
    const binding = {
      period_id: selected?.period_id,
      revision: selected?.revision,
    };
    if (action === "新建额度期") {
      path = "allowance-periods";
      body = {
        starts_at: timestamp(text("starts_at")),
        expires_at: timestamp(text("expires_at")),
      };
    } else if (action === "编辑有效期") {
      path = "allowance-validity";
      body = {
        ...binding,
        starts_at: timestamp(text("starts_at")),
        expires_at: timestamp(text("expires_at")),
      };
    } else if (action === "登记收款") {
      path = "receipts";
      body = {
        ...binding,
        amount_fen: moneyInput(text("amount"), 2),
        delta_micro_cny: moneyInput(text("delta")),
        paid_at: timestamp(text("paid_at")),
        channel: text("channel"),
        external_ref: text("external_ref") || null,
        note: text("note"),
      };
    } else {
      path = "allowance-adjustments";
      body = {
        ...binding,
        delta_micro_cny: moneyInput(text("delta")),
        reason: text("reason"),
      };
    }
    return ops.submit(action, `${base}/${path}`, body);
  };
  return (
    <section className="space-y-4 rounded-lg border p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>AI 使用额度（元）</h2>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            disabled={ops.blocked}
            onClick={() => open("新建额度期", null)}
          >
            新建额度期
          </Button>
          {current &&
            ["赠送 / 调整", "登记收款", "编辑有效期"].map((label) => (
              <Button
                key={label}
                variant="outline"
                disabled={ops.blocked}
                onClick={() => open(label)}
              >
                {label}
              </Button>
            ))}
        </div>
      </div>
      <p className="text-sm text-muted-foreground">
        使用额度用于承担模型调用成本，与实际收款分别记录。新一期不自动结转旧余额。
      </p>
      {current ? (
        <>
          <div className="grid grid-cols-2 gap-4 lg:grid-cols-5">
            {(
              [
                ["可用", "available_micro_cny"],
                ["授予 / 调整", "granted_micro_cny"],
                ["已扣减", "debited_micro_cny"],
                ["在途预留", "active_reserved_micro_cny"],
                ["待核算占用", "pending_micro_cny"],
              ] as const
            ).map(([label, key]) => (
              <div key={key} className="rounded-md bg-muted/50 p-3">
                <p className="text-xs text-muted-foreground">{label}</p>
                <strong className="mt-2 block text-lg tabular-nums">
                  {money(current.balance[key])}
                </strong>
              </div>
            ))}
          </div>
          <p className="text-sm">
            有效期：{date(current.starts_at)} — {date(current.expires_at)}
            （香港时间）
          </p>
        </>
      ) : (
        <p>当前无有效额度期。账号仍可阅读已授权材料与历史。</p>
      )}
      <details>
        <summary className="cursor-pointer text-sm">
          查看各额度期 / 操作旧期
        </summary>
        <DataState query={periods} />
        {periods.data && (
          <>
            <DataTable
              rows={periods.data.items}
              columns={[
                { header: "额度期", accessorKey: "period_id" },
                {
                  header: "开始",
                  cell: ({ row }) => date(row.original.starts_at),
                },
                {
                  header: "到期",
                  cell: ({ row }) => date(row.original.expires_at),
                },
                {
                  header: "可用（元）",
                  cell: ({ row }) =>
                    money(row.original.balance.available_micro_cny),
                },
                {
                  header: "操作",
                  cell: ({ row }) => (
                    <div className="flex gap-1">
                      {["赠送 / 调整", "登记收款", "编辑有效期"].map(
                        (label) => (
                          <Button
                            variant="ghost"
                            disabled={ops.blocked}
                            key={label}
                            onClick={() => open(label, row.original)}
                          >
                            {label}
                          </Button>
                        ),
                      )}
                    </div>
                  ),
                },
              ]}
            />
            <Pagination
              offset={offset}
              total={periods.data.total}
              change={setOffset}
            />
          </>
        )}
      </details>
      <OperationDialog
        title={action}
        description={
          action === "登记收款"
            ? "确认外部已收款后登记；实际收款与授予使用额度在同一笔操作中保存。"
            : action === "编辑有效期"
              ? "修改后，该期全部剩余额度使用新的到期时间。"
              : action === "赠送 / 调整"
                ? "赠送输入正数，下调输入负数；下调不得占用已结算与在途金额。"
                : "新建一个独立额度期，初始额度为零。时间均为香港时间。"
        }
        open={!!action}
        onOpenChange={(open) => {
          if (!open) setAction("");
        }}
        onSubmit={submit}
      >
        {selected && (
          <p className="break-all text-sm text-muted-foreground">
            额度期 {selected.period_id}
          </p>
        )}
        {["新建额度期", "编辑有效期"].includes(action) && (
          <>
            <Field label="开始时间（香港）">
              <Input
                name="starts_at"
                type="datetime-local"
                defaultValue={dateInput(selected?.starts_at ?? now)}
                required
              />
            </Field>
            <Field label="到期时间（香港）">
              <Input
                name="expires_at"
                type="datetime-local"
                defaultValue={dateInput(
                  selected?.expires_at ?? now + 30 * 86400,
                )}
                required
              />
            </Field>
          </>
        )}
        {action === "赠送 / 调整" && (
          <>
            <Field label="额度调整（元）">
              <Input
                name="delta"
                inputMode="decimal"
                placeholder="例如 10 或 -2"
                required
              />
            </Field>
            <Field label="原因">
              <Input name="reason" required />
            </Field>
          </>
        )}
        {action === "登记收款" && (
          <>
            <div className="grid grid-cols-2 gap-3">
              <Field label="实际收款（元）">
                <Input name="amount" inputMode="decimal" required />
              </Field>
              <Field label="授予 AI 使用额度（元）">
                <Input name="delta" inputMode="decimal" required />
              </Field>
            </div>
            <Field label="付款时间（香港）">
              <Input
                name="paid_at"
                type="datetime-local"
                defaultValue={dateInput(now)}
                required
              />
            </Field>
            <Field label="渠道">
              <Input name="channel" defaultValue="微信" required />
            </Field>
            <Field label="外部交易编号（可选）">
              <Input name="external_ref" />
            </Field>
            <Field label="收款备注">
              <Input name="note" required />
            </Field>
          </>
        )}
      </OperationDialog>
    </section>
  );
}

const operationNames: Record<string, string> = {
  "create-user": "创建账号",
  password: "改密",
  status: "启停",
  "revoke-sessions": "撤销会话",
  "book-grants": "授权材料",
  "book-revocations": "撤销授权",
  "allowance-periods": "新建额度期",
  "allowance-adjustments": "赠送 / 调整",
  "allowance-validity": "编辑有效期",
  receipts: "登记收款",
  "receipt-corrections": "收款更正",
  "charge-reconcile": "费用核算",
};
type Row = Record<string, string | number | null>;
export function HistoryPanel({ userId }: { userId: string }) {
  const [kind, setKind] = useState("receipts"),
    [offset, setOffset] = useState(0),
    [correction, setCorrection] = useState<Row | null>(null);
  const [periodId, setPeriodId] = useState(""),
    [periodOffset, setPeriodOffset] = useState(0);
  const base = `/admin/users/${encodeURIComponent(userId)}`,
    ops = useOperations();
  const query = useData<Page<Row>>(`${base}/${kind}?limit=20&offset=${offset}`);
  const periods = useData<Page<Period>>(
    `${base}/allowance-periods?limit=20&offset=${periodOffset}`,
  );
  return (
    <section className="space-y-3 rounded-lg border p-5">
      <h2>收款与额度明细</h2>
      <Field label="明细类型">
        <select
          value={kind}
          onChange={(e) => {
            setKind(e.target.value);
            setOffset(0);
          }}
        >
          <option value="operations">账号管理操作</option>
          <option value="receipts">收款记录</option>
          <option value="allowance-adjustments">额度调整</option>
          <option value="receipt-corrections">收款追加更正</option>
        </select>
      </Field>
      <DataState query={query} />
      {query.data && (
        <>
          <DataTable
            rows={query.data.items}
            columns={
              kind === "receipts"
                ? [
                    {
                      header: "付款时间",
                      cell: ({ row }) => date(Number(row.original.paid_at)),
                    },
                    {
                      header: "实际收款 / 更正后（元）",
                      cell: ({ row }) =>
                        `${money(Number(row.original.amount_fen), 2)} / ${money(Number(row.original.effective_amount_fen), 2)}`,
                    },
                    {
                      header: "渠道 / 外部编号",
                      cell: ({ row }) =>
                        `${row.original.channel} / ${row.original.external_ref ?? "—"}`,
                    },
                    { header: "备注", accessorKey: "note" },
                    {
                      header: "操作",
                      cell: ({ row }) => (
                        <Button
                          variant="outline"
                          disabled={ops.blocked}
                          onClick={() => {
                            setCorrection(row.original);
                            setPeriodId(String(row.original.period_id));
                          }}
                        >
                          追加更正
                        </Button>
                      ),
                    },
                  ]
                : kind === "operations"
                  ? [
                      {
                        header: "时间",
                        cell: ({ row }) =>
                          date(Number(row.original.created_at)),
                      },
                      { header: "操作人", accessorKey: "actor" },
                      { header: "操作号", accessorKey: "operation_id" },
                      {
                        header: "操作",
                        cell: ({ row }) =>
                          operationNames[String(row.original.kind)] ??
                          row.original.kind,
                      },
                    ]
                  : [
                      {
                        header: "时间",
                        cell: ({ row }) =>
                          date(Number(row.original.created_at)),
                      },
                      { header: "操作人", accessorKey: "actor" },
                      { header: "操作号", accessorKey: "operation_id" },
                      {
                        header:
                          kind === "allowance-adjustments"
                            ? "额度差额（元）"
                            : "收款差额（元）",
                        cell: ({ row }) =>
                          kind === "allowance-adjustments"
                            ? money(Number(row.original.delta_micro_cny))
                            : money(Number(row.original.delta_fen), 2),
                      },
                      { header: "原因", accessorKey: "reason" },
                    ]
            }
          />
          <Pagination
            offset={offset}
            total={query.data.total}
            change={setOffset}
          />
        </>
      )}
      <OperationDialog
        title="追加收款更正"
        description="保留原收款并追加差额。这是账面更正，不代表已向读者退款。"
        open={!!correction}
        onOpenChange={(open) => {
          if (!open) setCorrection(null);
        }}
        onSubmit={(data) => {
          const period = periods.data?.items.find(
            (p) => p.period_id === periodId,
          );
          if (!period) throw new Error("请先加载原收款所属额度期。");
          return ops.submit("追加收款更正", `${base}/receipt-corrections`, {
            receipt_id: correction!.receipt_id,
            period_id: period.period_id,
            revision: period.revision,
            delta_fen: moneyInput(String(data.get("amount")), 2),
            delta_micro_cny: moneyInput(String(data.get("delta"))),
            reason: String(data.get("reason")),
          });
        }}
      >
        <p className="break-all text-sm">收款 {correction?.receipt_id}</p>
        <Field label="原收款额度期">
          <Input readOnly value={periodId} />
        </Field>
        <DataState query={periods} />
        {periods.data &&
          !periods.data.items.some((p) => p.period_id === periodId) && (
            <>
              <p>请翻页加载原额度期。</p>
              <Pagination
                offset={periodOffset}
                total={periods.data.total}
                change={setPeriodOffset}
              />
            </>
          )}
        <Field label="收款差额（元）">
          <Input name="amount" inputMode="decimal" required />
        </Field>
        <Field label="额度差额（元）">
          <Input name="delta" inputMode="decimal" defaultValue="0" required />
        </Field>
        <Field label="更正原因">
          <Input name="reason" required />
        </Field>
      </OperationDialog>
    </section>
  );
}

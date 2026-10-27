import { useState } from "react";
import { type Page, type Period, type Percentage, type AllowanceBasis, type AllowanceOffer } from "@/lib/types";
import { date, dateInput, money, moneyInput, timestamp } from "@/lib/format";
import { useData, DataState, DataTable, Pagination } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { percentage, percentageInput, milliPercentage } from "@/lib/allowance";
import { McpCorrectionDialog } from "./mcp";
import { CloudPlansPanel } from "./cloud-plans";
import { AllowanceValue, Disclosure, Distribution, Metric, Notice, overviewMoney, overviewPercentage } from '@/components/analytics';

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
        basis: { version: text("basis_version"), basis_micro_cny: moneyInput(text("basis_amount")) },
      };
    } else if (action === "配置百分比基准") {
      path = "allowance-basis";
      body = { ...binding, basis: { version: text("basis_version"), basis_micro_cny: moneyInput(text("basis_amount")) } };
    } else if (action === "编辑有效期") {
      path = "allowance-validity";
      body = {
        ...binding,
        starts_at: timestamp(text("starts_at")),
        expires_at: timestamp(text("expires_at")),
      };
    } else if (action === "登记收款") {
      path = "percentage-receipts";
      body = {
        ...binding,
        amount_fen: moneyInput(text("amount"), 2),
        basis_version: selected?.balance.allowance_basis?.version,
        delta_milli_percent: percentageInput(text("delta")),
        paid_at: timestamp(text("paid_at")),
        channel: text("channel"),
        external_ref: text("external_ref") || null,
        note: text("note"),
      };
    } else {
      path = "allowance-percentage-adjustments";
      body = {
        ...binding,
        basis_version: selected?.balance.allowance_basis?.version,
        delta_milli_percent: percentageInput(text("delta")),
        reason: text("reason"),
      };
    }
    return ops.submit(action, `${base}/${path}`, body);
  };
  return (
    <section className="analytics-panel">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>AI 使用额度</h2>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            disabled={ops.blocked}
            onClick={() => open("新建额度期", null)}
          >
            新建额度期
          </Button>
          {current &&
            (current.balance.allowance_basis ? ["赠送 / 调整", "登记收款", "编辑有效期"] : ["配置百分比基准", "编辑有效期"]).map((label) => (
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
        百分比按本期固定基准计算，实际收款单独以元记录。
      </p>
      {current ? (
        <>
          <dl className="grid grid-cols-2 gap-3"><Metric label="当前可用" value={<AllowanceValue ratio={current.balance.percentages?.available} amount={current.balance.available_micro_cny}/>} detail="可用于后续 AI 调用" tone="green" /><Metric label="本期授予 / 调整" value={<AllowanceValue ratio={current.balance.percentages?.granted} amount={current.balance.granted_micro_cny}/>} detail={current.balance.allowance_basis ? '按本期固定基准计算，可超过 100%' : '本额度期累计授予与调整金额'} /></dl>
          <Distribution label="本期额度分配" total={current.balance.granted_micro_cny} segments={([
            ['debited','已扣减',current.balance.debited_micro_cny,'blue'],
            ['active_reserved','在途预留',current.balance.active_reserved_micro_cny,'violet'],
            ['pending','待核算占用',current.balance.pending_micro_cny,'amber'],
            ['available','可用',Math.max(0,current.balance.available_micro_cny),'green'],
          ] as const).map(([key,label,value,tone])=>({label,value,tone,display:current.balance.percentages ? overviewPercentage(current.balance.percentages[key]) : `${overviewMoney(value)} 元`}))} note={current.balance.allowance_basis ? '色条表示本期额度的分配；各项百分比沿用固定基准。' : '色条按本期实际金额分配。'} />
          {!current.balance.allowance_basis && <Notice warning>此额度期尚未设置百分比基准，当前按金额展示。配置基准后可使用比例管理。</Notice>}
          {current.balance.pending_micro_cny > 0 && <Notice warning>有 <AllowanceValue ratio={current.balance.percentages?.pending} amount={current.balance.pending_micro_cny}/> 额度等待核算，可在「费用与账单」中处理。</Notice>}
          {current.balance.available_micro_cny < 0 && <Notice warning>当前可用额度不足，请核对账目或补充额度。</Notice>}
          <Disclosure title="精确额度与基准"><dl className="number-detail text-sm">{([
            ['granted','本期授予 / 调整',current.balance.granted_micro_cny],
            ['available','当前可用',current.balance.available_micro_cny],
            ['debited','已扣减',current.balance.debited_micro_cny],
            ['active_reserved','在途预留',current.balance.active_reserved_micro_cny],
            ['pending','待核算占用',current.balance.pending_micro_cny],
          ] as const).map(([key,label,value])=><div key={key}><dt>{label}</dt><dd>{money(value)} 元{current.balance.percentages && <> · {percentage(current.balance.percentages[key])}</>}</dd></div>)}<div><dt>固定百分比基准</dt><dd>{current.balance.allowance_basis ? `${current.balance.allowance_basis.version} · ${money(current.balance.allowance_basis.basis_micro_cny)} 元` : '尚未配置'}</dd></div></dl></Disclosure>
          <p className="text-sm">
            有效期：{date(current.starts_at)} — {date(current.expires_at)}
            （香港时间）
          </p>
        </>
      ) : (
        <p>当前无有效额度期。账号仍可阅读已授权材料与历史。</p>
      )}
      <CloudPlansPanel userId={userId} />
      <p className="text-xs text-muted-foreground">补充额度不会改变旧用量比例，新一期不自动结转旧余额。</p>
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
                  header: "可用额度",
                  cell: ({ row }) =>
                    <AllowanceValue ratio={row.original.balance.percentages?.available} amount={row.original.balance.available_micro_cny}/>,
                },
                {
                  header: "操作",
                  cell: ({ row }) => (
                    <div className="flex gap-1">
                      {(row.original.balance.allowance_basis ? ["赠送 / 调整", "登记收款", "编辑有效期"] : ["配置百分比基准", "编辑有效期"]).map(
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
                ? "赠送输入正百分比，下调输入负百分比；下调不得占用已结算与在途额度。"
                : "固定基准一经绑定不能更改。新建额度期初始为零，时间均为香港时间。"
        }
        open={!!action}
        onOpenChange={(open) => {
          if (!open) setAction("");
        }}
        onSubmit={submit}
      >
        {selected && (
          <p className="break-all text-sm text-muted-foreground">
            额度期 {selected.period_id} · 基准 {selected.balance.allowance_basis?.version ?? "待配置"}
          </p>
        )}
        {["新建额度期", "配置百分比基准"].includes(action) && <>
          <Field label="固定基准版本"><Input name="basis_version" required /></Field>
          <Field label="100% 对应内部成本额度（元）"><Input name="basis_amount" inputMode="decimal" required /></Field>
          <p className="text-sm text-muted-foreground">按已确定的商业配置填写；该值不表示用户实际付款价格。</p>
        </>}
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
            <Field label="额度调整（%）">
              <Input
                name="delta"
                inputMode="decimal"
                placeholder="例如 50 或 -2.5"
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
              <Field label="授予 AI 使用额度（%）">
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
      <AllowanceOffers />
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
  "allowance-percentage-adjustments": "赠送 / 调整",
  "allowance-basis": "配置百分比基准",
  "percentage-receipts": "登记收款",
  "percentage-receipt-corrections": "收款更正",
  "allowance-offers": "补充方案",
  "allowance-validity": "编辑有效期",
  receipts: "登记收款",
  "receipt-corrections": "收款更正",
  "charge-reconcile": "费用核算",
  "mcp-packs": "登记 MCP 购包",
  "mcp-free-gift": "赠送 MCP 当月次数",
  "mcp-corrections": "更正 MCP 收款 / 次数",
};
type Row = { [key: string]: string | number | null | Percentage | AllowanceBasis | undefined; delta_percentage?: Percentage | null; grant_percentage?: Percentage | null; allowance_basis?: AllowanceBasis | null };
export function HistoryPanel({ userId }: { userId: string }) {
  const [kind, setKind] = useState("receipts"),
    [offset, setOffset] = useState(0),
    [correction, setCorrection] = useState<Row | null>(null);
  const [mcpReceiptId, setMcpReceiptId] = useState<string | null>(null);
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
                      header: "收款用途 / 权益",
                      cell: ({ row }) => row.original.entitlement_kind === "mcp_call_pack"
                        ? <span className="break-all">MCP 调用包 · {String(row.original.pack_id)}</span>
                        : <span className="break-all">模型使用额度 · {String(row.original.period_id)}</span>,
                    },
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
                      header: "原授予额度",
                      cell: ({ row }) => row.original.entitlement_kind === "mcp_call_pack" ? "见调用包" : `${percentage(row.original.grant_percentage)} · 基准 ${row.original.allowance_basis?.version ?? "待配置"}`,
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
                            if (row.original.entitlement_kind === "mcp_call_pack") {
                              setMcpReceiptId(String(row.original.receipt_id));
                              return;
                            }
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
                          String(row.original.kind),
                      },
                    ]
                  : [
                      ...(kind === "receipt-corrections" ? [{
                        header: "收款用途 / 原收款",
                        cell: ({ row }: { row: { original: Row } }) => `${row.original.entitlement_kind === "mcp_call_pack" ? "MCP 调用包" : "模型使用额度"} · ${row.original.receipt_id}`,
                      }] : []),
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
                            ? "额度差额（%）"
                            : "收款差额（元）",
                        cell: ({ row }) =>
                          kind === "allowance-adjustments"
                            ? percentage(row.original.delta_percentage)
                            : money(Number(row.original.delta_fen), 2),
                      },
                      { header: "原期额度差额 / 基准", cell: ({ row }) => row.original.entitlement_kind === "mcp_call_pack" ? "见调用包" : `${percentage(row.original.delta_percentage)} · ${row.original.allowance_basis?.version ?? "待配置"}` },
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
          if (!period.balance.allowance_basis) throw new Error("请先为原额度期配置百分比基准。");
          return ops.submit("追加收款更正", `${base}/percentage-receipt-corrections`, {
            receipt_id: correction!.receipt_id,
            period_id: period.period_id,
            revision: period.revision,
            delta_fen: moneyInput(String(data.get("amount")), 2),
            basis_version: period.balance.allowance_basis.version,
            delta_milli_percent: percentageInput(String(data.get("delta"))),
            reason: String(data.get("reason")),
          });
        }}
      >
        <p className="break-all text-sm">收款 {String(correction?.receipt_id ?? "")}</p>
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
        <Field label="额度差额（%）">
          <Input name="delta" inputMode="decimal" defaultValue="0" required />
        </Field>
        <Field label="更正原因">
          <Input name="reason" required />
        </Field>
      </OperationDialog>
      {mcpReceiptId && <McpCorrectionDialog userId={userId} receiptId={mcpReceiptId} close={() => setMcpReceiptId(null)} />}
    </section>
  );
}


function AllowanceOffers() {
  const ops = useOperations();
  const offers = useData<{ items: AllowanceOffer[] }>("/admin/allowance-offers");
  const [editing, setEditing] = useState<AllowanceOffer | null>(null);
  const [open, setOpen] = useState(false);
  return <details className="border-t pt-3">
    <summary className="cursor-pointer text-sm">补充方案（所有账号）</summary>
    <p className="my-3 text-sm text-muted-foreground">方案展示实际价格和增加比例，仅对相同基准版本的有效额度期可见。修改方案不改写原收款回执。</p>
    <Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(null); setOpen(true); }}>新增补充方案</Button>
    <DataState query={offers} />
    {offers.data?.items.map(offer => <div key={offer.offer_id} className="my-3 rounded-md border p-3">
      <p>{offer.title} · {money(offer.amount_fen, 2)} 元 · 增加 {milliPercentage(offer.delta_milli_percent)} · {offer.enabled ? "已启用" : "已停用"}</p>
      <p className="text-sm text-muted-foreground">基准 {offer.basis_version} · {offer.instructions}</p>
      <Button variant="ghost" disabled={ops.blocked} onClick={() => { setEditing(offer); setOpen(true); }}>编辑方案</Button>
    </div>)}
    <OperationDialog title={editing ? "编辑补充方案" : "新增补充方案"} description="按已确认的销售配置填写；保存方案不会授予额度，实际收款后仍需登记收款。" open={open} onOpenChange={setOpen} onSubmit={data => {
      const value = (key: string) => String(data.get(key) ?? "").trim();
      const amount = moneyInput(value("amount"), 2), delta = percentageInput(value("delta"));
      if (amount <= 0 || delta <= 0) throw new Error("价格和增加比例须大于零。");
      return ops.submit("保存补充方案", "/admin/allowance-offers", { offer_id: editing?.offer_id ?? crypto.randomUUID(), title: value("title"), amount_fen: amount, delta_milli_percent: delta, basis_version: value("basis_version"), instructions: value("instructions"), enabled: data.get("enabled") === "on" });
    }}>
      <Field label="方案名称"><Input name="title" defaultValue={editing?.title ?? ""} required /></Field>
      <Field label="实际支付价格（元）"><Input name="amount" inputMode="decimal" defaultValue={editing ? money(editing.amount_fen, 2) : ""} required /></Field>
      <Field label="增加额度（%）"><Input name="delta" inputMode="decimal" defaultValue={editing ? String(editing.delta_milli_percent / 1000) : ""} required /></Field>
      <Field label="适用基准版本"><Input name="basis_version" defaultValue={editing?.basis_version ?? ""} required /></Field>
      <Field label="收款联系与说明"><Input name="instructions" defaultValue={editing?.instructions ?? ""} required /></Field>
      <Field label="启用方案"><input name="enabled" type="checkbox" defaultChecked={editing?.enabled ?? true} /></Field>
    </OperationDialog>
  </details>;
}

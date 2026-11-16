import { useState } from "react";
import { type Page, type Period, type Percentage, type AllowanceBasis, type AllowanceOffer } from "@/lib/types";
import { date, dateInput, money, moneyInput, timestamp } from "@/lib/format";
import { useData, DataState, DataTable, Pagination } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { McpCorrectionDialog } from "./mcp";
import { CloudPlansPanel } from "./cloud-plans";
import { Distribution, Metric, Notice, overviewMoney } from '@/components/analytics';

const actionsFor = (period: Period) => [
  ...(!period.balance.unlimited ? ['赠送 / 扣回预算', '收款并补充本期'] : []),
  ...(!period.is_plan ? ['编辑有效期'] : []),
];

export function AllowancePanel({ userId, current }: { userId: string; current: Period | null }) {
  const [action, setAction] = useState(''), [selected, setSelected] = useState<Period | null>(null), [offset, setOffset] = useState(0);
  const base = `/admin/users/${encodeURIComponent(userId)}`, ops = useOperations();
  const periods = useData<Page<Period>>(`${base}/allowance-periods?limit=20&offset=${offset}`);
  const now = Math.floor(Date.now() / 1000);
  const open = (label: string, period: Period | null = current) => { setSelected(period); setAction(label); };
  const creating = action === '单独赠送 AI 额度';
  const submit = async (data: FormData) => {
    const text = (key: string) => String(data.get(key) ?? '').trim();
    const binding = { period_id: selected?.period_id, revision: selected?.revision };
    let path: string, body: Record<string, unknown>;
    if (creating) {
      const grant = moneyInput(text('budget'));
      if (grant <= 0) throw new Error('赠送的 AI 预算须大于零。');
      path = 'allowance-periods';
      body = { starts_at: timestamp(text('starts_at')), expires_at: timestamp(text('expires_at')), initial_grant_micro_cny: grant, reason: text('reason') };
    } else if (action === '编辑有效期') {
      path = 'allowance-validity';
      body = { ...binding, starts_at: timestamp(text('starts_at')), expires_at: timestamp(text('expires_at')) };
    } else if (action === '收款并补充本期') {
      const amount = moneyInput(text('amount'), 2), budget = moneyInput(text('budget'));
      if (amount <= 0 || budget <= 0) throw new Error('实际收款和补充预算须大于零。');
      path = 'receipts';
      body = { ...binding, amount_fen: amount, delta_micro_cny: budget, paid_at: timestamp(text('paid_at')), channel: text('channel'), external_ref: text('external_ref') || null, note: text('note') };
    } else {
      const delta = moneyInput(text('budget'));
      if (!delta) throw new Error('请输入要赠送或扣回的预算金额。');
      path = 'allowance-adjustments'; body = { ...binding, delta_micro_cny: delta, reason: text('reason') };
    }
    if ('starts_at' in body && Number(body.expires_at) <= Number(body.starts_at)) throw new Error('到期时间须晚于开始时间。');
    return ops.submit(action, `${base}/${path}`, body);
  };
  return <section className="analytics-panel">
    <CloudPlansPanel userId={userId} />
    <section className="space-y-4 border-t pt-5" aria-label="本期 AI 使用预算">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>本期 AI 使用预算</h2>
        <div className="flex flex-wrap gap-2">
          {current && actionsFor(current).map(label => <Button key={label} variant="outline" disabled={ops.blocked} onClick={() => open(label)}>{label}</Button>)}
          {!current && <Button variant="outline" disabled={ops.blocked} onClick={() => open('单独赠送 AI 额度', null)}>单独赠送 AI 额度</Button>}
        </div>
      </div>
      <p className="text-sm text-muted-foreground">后台按服务成本管理预算，读者看到本期剩余比例。实际收款在「费用与账单」中单独记录。</p>
      {current?.balance.unlimited ? <dl className="grid grid-cols-2 gap-3"><Metric label="当前可用" value="不限额度" detail="内测期间有效" tone="green" /><Metric label="累计已用" value={`${money(current.balance.debited_micro_cny)} 元`} detail="使用成本记录" /></dl> : current ? <>
        <dl className="grid grid-cols-2 gap-3"><Metric label="当前可用预算" value={`${overviewMoney(current.balance.available_micro_cny)} 元`} detail="可用于后续 AI 服务" tone="green" /><Metric label="本期预算总额" value={`${overviewMoney(current.balance.granted_micro_cny)} 元`} detail="含套餐、补充与赠送调整" /></dl>
        <Distribution label="本期预算分配" total={current.balance.granted_micro_cny} segments={([
          ['已使用', current.balance.debited_micro_cny, 'blue'], ['进行中预留', current.balance.active_reserved_micro_cny, 'violet'],
          ['待核算占用', current.balance.pending_micro_cny, 'amber'], ['可用', Math.max(0, current.balance.available_micro_cny), 'green'],
        ] as const).map(([label, value, tone]) => ({ label, value, tone, display: `${overviewMoney(value)} 元` }))} note="可用预算已扣除进行中和待核算的占用，核算后会更新。" />
        <details className="text-xs text-muted-foreground"><summary className="cursor-pointer">查看精确预算</summary><p className="mt-2">可用 {money(current.balance.available_micro_cny)} 元 · 总额 {money(current.balance.granted_micro_cny)} 元 · 已用 {money(current.balance.debited_micro_cny)} 元 · 预留 {money(current.balance.active_reserved_micro_cny)} 元 · 待核算 {money(current.balance.pending_micro_cny)} 元</p></details>
        {current.balance.pending_micro_cny > 0 && <Notice warning>有 {money(current.balance.pending_micro_cny)} 元预算等待核算，可在「费用与账单」中处理。</Notice>}
        {current.balance.available_micro_cny < 0 && <Notice warning>当前预算不足，请核对账目或补充本期预算。</Notice>}
        <p className="text-sm">有效期：{date(current.starts_at)} — {date(current.expires_at)}（香港）</p>
        <p className="text-sm text-muted-foreground">补充本期预算立即可用，不延长有效期，也不增加上传空间。{current.is_plan && '需要延长使用时间，请在上方续费套餐。'}</p>
      </> : <div className="rounded-lg border border-dashed p-5"><p>尚未开通 AI 使用额度。</p><p className="mt-2 text-sm text-muted-foreground">收款开通请使用上方套餐入口；免费体验可选择「单独赠送 AI 额度」，填写预算和有效期即可。账号仍可阅读已授权材料与历史。</p></div>}
    </section>
    <details className="border-t pt-4"><summary className="cursor-pointer text-sm">查看有效期与历史预算</summary>
      <div className="mt-3 space-y-3"><Button variant="outline" disabled={ops.blocked} onClick={() => open('单独赠送 AI 额度', null)}>新建赠送有效期</Button><DataState query={periods} />
        {periods.data && <><DataTable rows={periods.data.items} columns={[
          { header: '开始时间', cell: ({ row }) => date(row.original.starts_at) },
          { header: '到期时间', cell: ({ row }) => row.original.balance.unlimited ? '内测期间' : date(row.original.expires_at) },
          { header: '可用 AI 预算', cell: ({ row }) => row.original.balance.unlimited ? '不限额度' : `${money(row.original.balance.available_micro_cny)} 元` },
          { header: '操作', cell: ({ row }) => <div className="flex flex-wrap gap-1">{actionsFor(row.original).map(label => <Button key={label} variant="ghost" disabled={ops.blocked} onClick={() => open(label, row.original)}>{label}</Button>)}</div> },
        ]} /><Pagination offset={offset} total={periods.data.total} change={setOffset} /></>}
      </div>
    </details>
    <OperationDialog title={action} description={creating ? '免费赠送 AI 使用预算，同时设置有效期。保存后即可使用，无需再做一次额度调整。不包含套餐课程与上传空间。' : action === '收款并补充本期' ? '确认实际收到款项后，增加所选有效期的 AI 预算。该操作不续期。' : action === '编辑有效期' ? '修改这笔独立额度的起止时间。套餐续费请使用套餐开通入口。' : '赠送输入正金额，扣回输入负金额；仅能扣回尚未使用或占用的预算。'} open={!!action} onOpenChange={value => { if (!value) setAction(''); }} onSubmit={submit} submitLabel={creating ? '确认赠送并开通' : action === '收款并补充本期' ? '确认收款并补充预算' : '保存调整'}>
      {selected && <p className="mb-3 rounded-md bg-muted/40 p-3 text-sm">操作有效期：{date(selected.starts_at)} — {selected.balance.unlimited ? '内测期间' : date(selected.expires_at)}（香港）<br />当前可用：{selected.balance.unlimited ? '不限额度' : `${money(selected.balance.available_micro_cny)} 元 AI 预算`}</p>}
      {action === '收款并补充本期' && <Field label="实际收款（元）"><Input name="amount" inputMode="decimal" required /></Field>}
      {action !== '编辑有效期' && <Field label={action === '赠送 / 扣回预算' ? '预算调整（元，赠送为正、扣回为负）' : '增加 AI 使用预算（元）'}><Input name="budget" inputMode="decimal" required /></Field>}
      {(creating || action === '编辑有效期') && <><Field label="开始时间（香港）"><Input name="starts_at" type="datetime-local" defaultValue={dateInput(selected?.starts_at ?? now)} required /></Field><Field label="到期时间（香港）"><Input name="expires_at" type="datetime-local" defaultValue={dateInput(selected?.expires_at ?? now + 30 * 86400)} required /></Field></>}
      {(creating || action === '赠送 / 扣回预算') && <Field label="原因"><Input name="reason" required /></Field>}
      {action === '收款并补充本期' && <><Field label="付款时间（香港）"><Input name="paid_at" type="datetime-local" defaultValue={dateInput(now)} required /></Field><Field label="收款渠道"><Input name="channel" defaultValue="微信" required /></Field><Field label="交易编号（选填）"><Input name="external_ref" /></Field><Field label="备注（选填）"><Input name="note" /></Field></>}
    </OperationDialog>
    <AllowanceOffers />
  </section>;
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
                      cell: ({ row }) => row.original.entitlement_kind === "mcp_call_pack" ? "见调用包" : `${money(Number(row.original.grant_micro_cny))} 元 AI 预算`,
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
                            ? "AI 预算差额（元）"
                            : "收款差额（元）",
                        cell: ({ row }) =>
                          kind === "allowance-adjustments"
                            ? `${money(Number(row.original.delta_micro_cny))} 元`
                            : money(Number(row.original.delta_fen), 2),
                      },
                      { header: "原期 AI 预算差额", cell: ({ row }) => row.original.entitlement_kind === "mcp_call_pack" ? "见调用包" : `${`${money(Number(row.original.delta_micro_cny))} 元`} · ${row.original.allowance_basis?.version ?? "待配置"}` },
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
        <Field label="AI 预算差额（元）">
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
  const [editing, setEditing] = useState<AllowanceOffer | null>(null), [open, setOpen] = useState(false);
  return <details className="border-t pt-4">
    <summary className="cursor-pointer text-sm">管理额度加购商品 · 所有账号共用</summary>
    <p className="my-3 text-sm text-muted-foreground">加购商品只增加当前有效期的 AI 预算，不续期，不增加空间。保存商品后，实际到账仍需在账号上「收款并补充本期」。</p>
    <Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(null); setOpen(true); }}>新增加购商品</Button>
    <DataState query={offers} />
    {offers.data?.items.map(offer => <div key={offer.offer_id} className="my-3 space-y-2 rounded-md border p-4">
      <p className="font-medium">{offer.title}</p>
      <p>售价 {money(offer.amount_fen, 2)} 元 · 增加 {money(offer.budget_micro_cny)} 元 AI 预算 · {offer.enabled ? "已启用" : "已停用"}</p>
      <p className="text-sm text-muted-foreground">{offer.instructions}</p>
      <Button variant="ghost" disabled={ops.blocked} onClick={() => { setEditing(offer); setOpen(true); }}>编辑加购商品</Button>
    </div>)}
    <OperationDialog title={editing ? "编辑加购商品" : "新增加购商品"} description="分别填写收取的价格和发放的 AI 预算。所有拥有有效额度的账号均可加购。" open={open} onOpenChange={setOpen} submitLabel="保存加购商品" onSubmit={data => {
      const value = (key: string) => String(data.get(key) ?? "").trim();
      const amount = moneyInput(value("amount"), 2), budget = moneyInput(value("budget"));
      if (amount <= 0 || budget <= 0) throw new Error("售价和增加预算须大于零。");
      return ops.submit("保存额度加购商品", "/admin/allowance-offers", { offer_id: editing?.offer_id ?? crypto.randomUUID(), title: value("title"), amount_fen: amount, budget_micro_cny: budget, instructions: value("instructions"), enabled: data.get("enabled") === "on" });
    }}>
      <Field label="商品名称"><Input name="title" defaultValue={editing?.title ?? ""} required /></Field>
      <Field label="售价（元）"><Input name="amount" inputMode="decimal" defaultValue={editing ? money(editing.amount_fen, 2) : ""} required /></Field>
      <Field label="增加 AI 预算（元）"><Input name="budget" inputMode="decimal" defaultValue={editing ? money(editing.budget_micro_cny) : ""} required /></Field>
      <Field label="开通说明与联系方式"><Input name="instructions" defaultValue={editing?.instructions ?? ""} required /></Field>
      <Field label="启用商品"><input name="enabled" type="checkbox" defaultChecked={editing?.enabled ?? true} /></Field>
    </OperationDialog>
  </details>;
}

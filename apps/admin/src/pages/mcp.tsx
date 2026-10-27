import { useState } from "react";
import type { McpUsage } from "@/lib/types";
import { date, dateInput, money, moneyInput, timestamp } from "@/lib/format";
import { useData, DataState, DataTable } from "@/components/data";
import { Field, OperationDialog, useOperations } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Disclosure, Distribution, Metric } from '@/components/analytics';

function integer(text: string, label: string, positive = false) {
  const value = Number(text);
  if (!/^-?\d+$/.test(text) || !Number.isSafeInteger(value) || (positive && value <= 0))
    throw new Error(`${label}须为${positive ? "正" : ""}整数。`);
  return value;
}
function packCount(text: string) {
  const count = integer(text, "包数", true);
  if (!Number.isSafeInteger(count * 1000)) throw new Error("包数超出可输入范围。");
  return count;
}
export function mcpDate(value: string) {
  return date(Date.parse(value) / 1000);
}

export function McpPanel({ userId }: { userId: string }) {
  const base = `/admin/users/${encodeURIComponent(userId)}/mcp`;
  const query = useData<McpUsage>(base), ops = useOperations();
  const [action, setAction] = useState("");
  const [count, setCount] = useState("1");
  const [receiptId, setReceiptId] = useState<string | null>(null);
  let preview = "请输入有效的正整数包数。";
  try {
    const packs = packCount(count);
    preview = `${packs} 包 · 确认收款 ${money(packs * 1000, 2)} 元 · 开通 ${packs * 1000} 次`;
  } catch { /* The form reports invalid input on submission. */ }
  const usage = query.data;
  return (
    <section className="analytics-panel" aria-label="MCP 次数管理">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2>MCP 调用次数</h2>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => void query.refetch()}>刷新次数</Button>
          {["登记 MCP 购包", "赠送当月次数"].map(label => (
            <Button key={label} variant="outline" disabled={ops.blocked || !usage || !!query.error}
              onClick={() => { setCount("1"); setAction(label); }}>{label}</Button>
          ))}
        </div>
      </div>
      <p className="text-sm text-muted-foreground">每月免费 1,000 次，免费优先，其次使用最早到期的有效调用包。每包 10 元 / 1,000 次，开通后有效 12 个日历月，跨月保留。</p>
      <DataState query={query} />
      {usage && <>
        <dl className="grid gap-3 sm:grid-cols-2"><Metric label="当月免费剩余" value={<>{usage.free_remaining.toLocaleString('zh-CN')} <small className="text-sm font-normal">次</small></>} detail={`免费恢复：${mcpDate(usage.free_resets_at)}（香港）`} tone="green" /><Metric label="有效已购剩余" value={<>{usage.paid_remaining.toLocaleString('zh-CN')} <small className="text-sm font-normal">次</small></>} detail={`下一批到期：${usage.paid_next_expires_at ? mcpDate(usage.paid_next_expires_at) : '暂无'}`} /></dl>
        <Distribution label="当月免费次数使用情况" total={usage.free_granted} segments={[{label:'已用',value:usage.free_used,tone:'blue'},{label:'免费剩余',value:usage.free_remaining,tone:'green'}]} note={`${usage.month_hk}：授予 ${usage.free_granted} 次，已用 ${usage.free_used} 次`} />
        <Disclosure title={`已购调用包明细 · ${usage.call_packs.length} 批`}>
        <DataTable rows={usage.call_packs} columns={[
          { header: "批次 / 收款", cell: ({ row: { original: p } }) => <span className="break-all">{p.pack_id}<br />{p.receipt_id}</span> },
          { header: "当前可用（次）", cell: ({ row: { original: p } }) => <div><strong>{p.available_units.toLocaleString('zh-CN')}</strong><p className="text-xs text-muted-foreground">已用 {p.used_units} · 未用 {p.remaining_units}</p></div> },
          { header: "购买与调整", cell: ({ row: { original: p } }) => <details><summary>购买条款与明细</summary><dl className="space-y-2 py-2 text-xs"><div><dt>购买条款</dt><dd>{p.pack_count} 包 · {money(p.unit_price_fen, 2)} 元/包 · {p.units_per_pack} 次/包</dd></div><div><dt>原始收款（元）</dt><dd>{money(p.price_fen,2)}</dd></div><div><dt>原授予 / 调整 / 已用（次）</dt><dd>{p.total_units} / {p.adjusted_units} / {p.used_units}</dd></div><div><dt>未用 / 当前可用（次）</dt><dd>{p.remaining_units} / {p.available_units}</dd></div></dl></details> },
          { header: "开通 / 到期（香港）", cell: ({ row: { original: p } }) => <>{mcpDate(p.opened_at)}<br />{mcpDate(p.expires_at)}</> },
          { header: "操作", cell: ({ row }) => <Button variant="outline" disabled={ops.blocked || !!query.error} onClick={() => setReceiptId(row.original.receipt_id)}>更正 MCP 收款 / 次数</Button> },
        ]} />
        <p className="text-sm text-muted-foreground">过期包保留未用次数，当前可用为零。更正后的收款金额见「费用与账单」中的收款记录。</p>
        </Disclosure>
      </>}
      <OperationDialog key={action} title={action} open={!!action} onOpenChange={open => { if (!open) setAction(""); }}
        description={action === "登记 MCP 购包" ? `账号 ${userId}：确认外部已收款后登记，包数决定金额与次数；有效期从服务端开通时起计算。` : `账号 ${userId}：赠送只在操作成功时所属的香港月份有效，记录原因，不生成收款。`}
        onSubmit={data => {
          const text = (key: string) => String(data.get(key) ?? "").trim();
          if (action === "登记 MCP 购包") {
            const channel = text("channel");
            if (!channel) throw new Error("请填写收款渠道。");
            return ops.submit(`${action} · ${userId}`, `${base}/packs`, {
              pack_count: packCount(text("pack_count")), paid_at: timestamp(text("paid_at")),
              channel, external_ref: text("external_ref") || null, note: text("note"),
            });
          }
          const reason = text("reason");
          if (!reason) throw new Error("请填写赠送原因。");
          return ops.submit(`${action} · ${userId}`, `${base}/gifts`, { units: integer(text("units"), "赠送次数", true), reason });
        }}>
        {action === "登记 MCP 购包" ? <>
          <Field label="购买包数"><Input name="pack_count" inputMode="numeric" value={count} onChange={e => setCount(e.target.value)} required /></Field>
          <p role="status">{preview}</p>
          <Field label="付款时间（香港）"><Input name="paid_at" type="datetime-local" defaultValue={dateInput(Math.floor(Date.now() / 1000))} required /></Field>
          <Field label="渠道"><Input name="channel" defaultValue="微信" required /></Field>
          <Field label="外部交易编号（可选）"><Input name="external_ref" /></Field>
          <Field label="收款备注"><Input name="note" required /></Field>
        </> : <>
          <Field label="赠送次数"><Input name="units" inputMode="numeric" required /></Field>
          <Field label="赠送原因"><Input name="reason" required /></Field>
        </>}
      </OperationDialog>
      {receiptId && <McpCorrectionDialog userId={userId} receiptId={receiptId} close={() => setReceiptId(null)} />}
    </section>
  );
}

export function McpCorrectionDialog({ userId, receiptId, close }: { userId: string; receiptId: string; close: () => void }) {
  const base = `/admin/users/${encodeURIComponent(userId)}/mcp`;
  const query = useData<McpUsage>(base), ops = useOperations();
  const pack = query.data?.call_packs.find(p => p.receipt_id === receiptId);
  return <OperationDialog title="更正 MCP 收款 / 次数" open onOpenChange={open => { if (!open) close(); }}
    description={`账号 ${userId}：金额与次数分别追加差额。次数只能扣回未消费部分；更正不延长有效期，金额更正不代表外部退款。`}
    onSubmit={data => {
      if (!pack || query.error) throw new Error("请先重新读取原调用包。");
      const delta_fen = moneyInput(String(data.get("amount")), 2);
      const delta_units = integer(String(data.get("units")).trim(), "次数差额");
      const reason = String(data.get("reason") ?? "").trim();
      if (!delta_fen && !delta_units) throw new Error("金额与次数差额至少一项非零。");
      if (delta_units < -pack.remaining_units) throw new Error("只能扣回尚未消费的次数。");
      if (!reason) throw new Error("请填写更正原因。");
      return ops.submit(`更正 MCP 收款 / 次数 · ${userId}`, `${base}/corrections`, {
        receipt_id: receiptId, revision: pack.revision, delta_fen, delta_units, reason,
      });
    }}>
    <p className="break-all text-sm">原收款：{receiptId}</p>
    <DataState query={query} />
    {pack ? <p className="text-sm">已消费 {pack.used_units} 次，最多可扣回 {pack.remaining_units} 次；到期 {mcpDate(pack.expires_at)}（香港时间）。</p> : !query.isPending && <p>未找到原调用包，请重新读取。</p>}
    <Button type="button" variant="outline" onClick={() => void query.refetch()}>重新读取调用包</Button>
    <Field label="收款差额（元）"><Input name="amount" inputMode="decimal" defaultValue="0" required /></Field>
    <Field label="次数差额（次）"><Input name="units" inputMode="numeric" defaultValue="0" required /></Field>
    <Field label="更正原因"><Input name="reason" required /></Field>
  </OperationDialog>;
}

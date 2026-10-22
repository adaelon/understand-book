import { useState } from 'react';
import { DataState, useData } from '@/components/data';
import { Field, OperationDialog, useOperations } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cloudPlanForm, durationLabels, tierLabels, unavailableLabels, type CloudPlanProduct, type UserCloudPlans } from '@/lib/cloud-plans';
import { date, dateInput, money, timestamp } from '@/lib/format';
import { milliPercentage } from '@/lib/allowance';

const megabytes = (value: number | null | undefined) => value == null ? '' : String(value / 1024 ** 2);
export function CloudPlansPanel({ userId }: { userId: string }) {
  const ops = useOperations();
  const path = `/admin/users/${encodeURIComponent(userId)}/cloud-plan-purchases`;
  const user = useData<UserCloudPlans>(path);
  const catalog = useData<{ items: CloudPlanProduct[] }>('/admin/cloud-plans');
  const [open, setOpen] = useState(false), [editing, setEditing] = useState<CloudPlanProduct | null>(null);
  const [purchase, setPurchase] = useState<CloudPlanProduct | null>(null);
  const items = catalog.data?.items ?? [];
  return <section className="space-y-4 border-t pt-5" aria-label="云端套餐">
    <h3>云端套餐与续期</h3>
    <p className="text-sm text-muted-foreground">网站与 Windows 共用权益。确认实际收款后按所选商品版本开通；当前仍有有效期时接到最后一期之后，新一期不结转旧余额。</p>
    <DataState query={user} />
    {user.data && <div className="grid gap-3 md:grid-cols-2">
      {([['当前套餐', user.data.current], ['下一期套餐', user.data.next]] as const).map(([title, plan]) => <div key={title} className="rounded-md border p-3">
        <h4>{title}</h4>
        {plan ? <><p>{plan.title} · {tierLabels[plan.tier]} · {durationLabels[plan.duration]}</p><p className="text-sm">{date(plan.starts_at)} — {date(plan.expires_at)}（香港时间）</p><p className="text-sm">额度 {milliPercentage(plan.grant_milli_percent)} · 空间 {megabytes(plan.storage_bytes)} MB</p></> : <p className="text-sm text-muted-foreground">尚无安排</p>}
      </div>)}
    </div>}
    <details>
      <summary className="cursor-pointer">套餐商品与开通（所有账号共用商品配置）</summary>
      <p className="my-3 text-sm text-muted-foreground">未确定的价格、额度和空间可留空，配置不完整的商品不能出售。测试商品仅用于验收，不会在读者购买界面展示。</p>
      <Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(null); setOpen(true); }}>新增套餐商品</Button>
      <DataState query={catalog} />
      {!catalog.isPending && !catalog.error && !items.length && <p className="my-3">尚未配置套餐商品。</p>}
      <div className="my-3 grid gap-3 lg:grid-cols-2">{items.map(product => <article key={product.product_id} className="space-y-2 rounded-md border p-3">
        <h4>{product.title} · {tierLabels[product.tier]} · {durationLabels[product.duration]}</h4>
        <p className="text-sm">{product.amount_fen === null ? '价格待定' : `${money(product.amount_fen, 2)} 元`} · {product.grant_milli_percent === null ? '额度待定' : `额度 ${milliPercentage(product.grant_milli_percent)}`} · {product.storage_bytes === null ? '空间待定' : `${megabytes(product.storage_bytes)} MB`}</p>
        <p className="text-sm text-muted-foreground">{product.test_only ? '测试商品' : product.saleable ? '可售' : '暂不可售'} · {product.enabled ? '已启用' : '已停用'} · 版本 {product.revision}</p>
        {!product.saleable && <p className="text-sm text-muted-foreground">{product.unavailable_reasons.map(reason => unavailableLabels[reason] ?? '暂不可售').join('；')}</p>}
        <div className="flex flex-wrap gap-2"><Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(product); setOpen(true); }}>编辑套餐</Button><Button disabled={ops.blocked || !product.saleable || product.test_only} onClick={() => setPurchase(product)}>核款并开通</Button></div>
      </article>)}</div>
    </details>
    <OperationDialog title={editing ? '编辑套餐商品' : '新增套餐商品'} description="按已确认的商业配置填写；未定数值留空。保存只更新商品，不修改已售权益，也不向当前账号授予额度。" open={open} onOpenChange={setOpen} onSubmit={data => ops.submit('保存云端套餐商品', '/admin/cloud-plans', cloudPlanForm(data, editing))}>
      <Field label="套餐名称"><Input name="title" defaultValue={editing?.title ?? ''} required /></Field>
      <Field label="档位"><select name="tier" defaultValue={editing?.tier ?? 'basic'}>{Object.entries(tierLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></Field>
      <Field label="时长"><select name="duration" defaultValue={editing?.duration ?? 'day1'}>{Object.entries(durationLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></Field>
      <Field label="售价（元，未定留空）"><Input name="amount" inputMode="decimal" defaultValue={editing?.amount_fen == null ? '' : money(editing.amount_fen, 2)} /></Field>
      <Field label="额度基准名称（未定留空）"><Input name="basis_version" defaultValue={editing?.basis?.version ?? ''} /></Field>
      <Field label="100% 对应内部额度（元，未定留空）"><Input name="basis_amount" inputMode="decimal" defaultValue={editing?.basis ? money(editing.basis.basis_micro_cny) : ''} /></Field>
      <Field label="每期授予额度（%，未定留空）"><Input name="grant" inputMode="decimal" defaultValue={editing?.grant_milli_percent == null ? '' : String(editing.grant_milli_percent / 1000)} /></Field>
      <Field label="个人空间（MB，未定留空）"><Input name="storage_mb" inputMode="decimal" defaultValue={megabytes(editing?.storage_bytes)} /></Field>
      <Field label="单文件上限（MB，未定留空）"><Input name="max_file_mb" inputMode="decimal" defaultValue={megabytes(editing?.max_file_bytes)} /></Field>
      <Field label="到期后私人资料保留（天，未定留空）"><Input name="retention_days" inputMode="decimal" defaultValue={editing?.retention_seconds == null ? '' : String(editing.retention_seconds / 86400)} /></Field>
      <Field label="联系与开通说明"><textarea name="instructions" className="w-full rounded-md border p-2" defaultValue={editing?.instructions ?? ''} /></Field>
      <Field label="启用商品"><input name="enabled" type="checkbox" defaultChecked={editing?.enabled ?? false} /></Field>
      <Field label="仅供测试"><input name="test_only" type="checkbox" defaultChecked={editing?.test_only ?? false} /></Field>
    </OperationDialog>
    <OperationDialog title="核款并开通套餐" description="确认实际收到款项后登记。本次将按商品版本固定价格与权益；未生效期接续已有安排，实际起止时间由系统计算。" open={!!purchase} onOpenChange={value => { if (!value) setPurchase(null); }} onSubmit={data => {
      if (!purchase) return Promise.resolve(false);
      return ops.submit('核款并开通云端套餐', path, { product_id: purchase.product_id, product_revision: purchase.revision, paid_at: timestamp(String(data.get('paid_at'))), channel: String(data.get('channel') ?? '').trim(), external_ref: String(data.get('external_ref') ?? '').trim() || null, note: String(data.get('note') ?? '').trim() });
    }}>
      {purchase && <p>{purchase.title} · {durationLabels[purchase.duration]} · 实收 {money(purchase.amount_fen, 2)} 元 · 授予 {milliPercentage(purchase.grant_milli_percent!)}</p>}
      <Field label="实际付款时间（香港时间）"><Input name="paid_at" type="datetime-local" defaultValue={dateInput(Math.floor(Date.now() / 1000))} required /></Field>
      <Field label="实际收款渠道"><Input name="channel" required /></Field>
      <Field label="外部交易编号"><Input name="external_ref" /></Field>
      <Field label="核款备注"><Input name="note" required /></Field>
    </OperationDialog>
  </section>;
}

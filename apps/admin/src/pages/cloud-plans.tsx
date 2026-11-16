import { useState } from 'react';
import { DataState, useData } from '@/components/data';
import { Field, OperationDialog, useOperations } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cloudPlanForm, durationLabels, planBudget, tierLabels, unitInput, unavailableLabels, type CloudPlanProduct, type UserCloudPlans } from '@/lib/cloud-plans';
import { date, dateInput, money, timestamp } from '@/lib/format';
import { Notice, StatusBadge } from '@/components/analytics';

const megabytes = (value: number | null | undefined) => unitInput(value, 1024 ** 2);
const storage = (value: number) => value >= 1024 ** 3 ? `${value / 1024 ** 3} GB` : `${megabytes(value)} MB`;
export function CloudPlansPanel({ userId }: { userId: string }) {
  const ops = useOperations();
  const path = `/admin/users/${encodeURIComponent(userId)}/cloud-plan-purchases`;
  const user = useData<UserCloudPlans>(path);
  const catalog = useData<{ items: CloudPlanProduct[] }>('/admin/cloud-plans');
  const [open, setOpen] = useState(false), [editing, setEditing] = useState<CloudPlanProduct | null>(null);
  const [purchaseOpen, setPurchaseOpen] = useState(false), [productId, setProductId] = useState('');
  const items = catalog.data?.items ?? [];
  const available = items.filter(product => product.saleable && !product.test_only);
  const purchase = available.find(product => product.product_id === productId);
  const queued = !!user.data && user.data.purchase_starts_at > Math.floor(Date.now() / 1000);
  const unlimited = user.data?.purchase_blocked_by_unlimited === true;
  return <section className="space-y-4" aria-label="套餐开通与续费">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div><h2>套餐开通与续费</h2><p className="mt-2 text-sm text-muted-foreground">选套餐、确认到账，一次开通有效期、AI 额度和上传空间。</p></div>
      <Button disabled={ops.blocked || !user.data || !available.length || unlimited} onClick={() => { setProductId(available[0]?.product_id ?? ''); setPurchaseOpen(true); }}>开通 / 续费套餐</Button>
    </div>
    <DataState query={user} />
    {user.data && <div className="grid gap-3 md:grid-cols-2">
      {([['当前套餐', user.data.current], ['下一期套餐', user.data.next]] as const).map(([title, plan]) => <div key={title} className="rounded-lg border bg-muted/20 p-4">
        <h3 className="text-sm text-muted-foreground">{title}</h3>
        {plan ? <div className="mt-3 space-y-3"><p className="text-lg font-semibold">{plan.title}</p><StatusBadge tone={title === '当前套餐' ? 'green' : 'blue'}>{tierLabels[plan.tier]} · {durationLabels[plan.duration]}</StatusBadge><dl className="number-detail text-sm"><div><dt>本期 AI 预算</dt><dd>{money(planBudget(plan))} 元</dd></div><div><dt>上传空间</dt><dd>{storage(plan.storage_bytes)}</dd></div></dl><p className="text-xs text-muted-foreground">{date(plan.starts_at)} — {date(plan.expires_at)}（香港）</p></div> : <p className="py-4 text-sm text-muted-foreground">{title === '当前套餐' ? '尚未开通套餐' : '尚未安排续费'}</p>}
      </div>)}
    </div>}
    {unlimited ? <Notice>该账号正在内测不限额度。需先结束内测有效期，再开通收费套餐。</Notice> : <p className="text-sm text-muted-foreground">{queued ? `新套餐将从 ${date(user.data!.purchase_starts_at)}（香港时间）起接续。` : '首次开通立即生效。'}续费增加下一期；本期不够用时，请在下方补充本期预算。</p>}
    <details className="rounded-lg border p-4">
      <summary className="cursor-pointer font-medium">管理套餐商品 <span className="text-sm font-normal text-muted-foreground">· 所有账号共用</span></summary>
      <div className="mt-4 space-y-4">
        <p className="text-sm text-muted-foreground">这里设置卖什么、卖多少钱、包含多少 AI 预算和空间。保存商品后，再为账号开通。已有套餐按购买时的配置执行。</p>
        <Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(null); setOpen(true); }}>新增套餐商品</Button>
        <DataState query={catalog} />
        {!catalog.isPending && !catalog.error && !items.length && <p>尚无套餐商品。先新增并启用一个套餐，即可为账号开通。</p>}
        <div className="grid gap-3 lg:grid-cols-2">{items.map(product => <article key={product.product_id} className="space-y-3 rounded-md border p-4">
          <h4 className="font-semibold">{product.title}</h4><p className="text-xs text-muted-foreground">{tierLabels[product.tier]} · {durationLabels[product.duration]}</p>
          <dl className="number-detail text-sm"><div><dt>售价</dt><dd>{product.amount_fen == null ? '待定' : `${money(product.amount_fen, 2)} 元`}</dd></div><div><dt>AI 使用预算</dt><dd>{planBudget(product) == null ? '待定' : `${money(planBudget(product))} 元`}</dd></div><div><dt>上传空间</dt><dd>{product.storage_bytes == null ? '待定' : storage(product.storage_bytes)}</dd></div></dl>
          <p className="text-sm text-muted-foreground">{product.test_only ? '仅供测试' : product.saleable ? '已上架，可开通' : '尚未上架'}{!product.saleable && ` · ${[...new Set(product.unavailable_reasons.map(reason => unavailableLabels[reason] ?? '暂不可售'))].join('；')}`}</p>
          <Button variant="outline" disabled={ops.blocked} onClick={() => { setEditing(product); setOpen(true); }}>编辑套餐</Button>
        </article>)}</div>
      </div>
    </details>
    {!catalog.isPending && !catalog.error && !available.length && <Notice>暂无可开通的套餐。展开「管理套餐商品」，补齐配置并启用套餐。</Notice>}
    <OperationDialog title={editing ? '编辑套餐商品' : '新增套餐商品'} description="售价是向用户收取的金额；AI 使用预算是这个套餐允许消耗的服务成本。未确定的项目可留空保存草稿。" open={open} onOpenChange={setOpen} submitLabel="保存套餐商品" onSubmit={data => ops.submit('保存套餐商品', '/admin/cloud-plans', cloudPlanForm(data, editing))}>
      <Field label="套餐名称"><Input name="title" defaultValue={editing?.title ?? ''} required placeholder="例如：基础月度套餐" /></Field>
      <div className="grid grid-cols-2 gap-3"><Field label="档位"><select name="tier" defaultValue={editing?.tier ?? 'basic'}>{Object.entries(tierLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></Field><Field label="有效时长"><select name="duration" defaultValue={editing?.duration ?? 'month'}>{Object.entries(durationLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></Field></div>
      <div className="grid gap-3 sm:grid-cols-2"><Field label="售价（元）"><Input name="amount" inputMode="decimal" defaultValue={editing?.amount_fen == null ? '' : money(editing.amount_fen, 2)} /></Field><Field label="AI 使用预算（元）"><Input name="budget" inputMode="decimal" defaultValue={editing && planBudget(editing) != null ? money(planBudget(editing)) : ''} /></Field></div>
      <p className="text-sm text-muted-foreground">例如售价 30 元、AI 预算 10 元：收款 30 元，授予 10 元的 AI 使用预算。读者开通后看到剩余 100%。</p>
      <div className="grid grid-cols-2 gap-3"><Field label="上传空间（MB）"><Input name="storage_mb" inputMode="decimal" defaultValue={megabytes(editing?.storage_bytes)} /></Field><Field label="单文件上限（MB）"><Input name="max_file_mb" inputMode="decimal" defaultValue={megabytes(editing?.max_file_bytes)} /></Field></div>
      <Field label="到期后原始资料保留（天）"><Input name="retention_days" inputMode="decimal" defaultValue={unitInput(editing?.retention_seconds, 86400)} /></Field>
      <Field label="读者看到的开通说明与联系方式"><textarea name="instructions" className="w-full rounded-md border p-2" defaultValue={editing?.instructions ?? ''} /></Field>
      <label className="flex items-center gap-2 py-2 text-sm"><input name="enabled" type="checkbox" defaultChecked={editing?.enabled ?? false} />启用商品（配置完整后可开通）</label>
      <label className="flex items-center gap-2 py-2 text-sm"><input name="test_only" type="checkbox" defaultChecked={editing?.test_only ?? false} />仅供测试</label>
    </OperationDialog>
    <OperationDialog title="开通 / 续费套餐" description={`为账号 ${userId} 登记已收到的款项，并开通所选套餐。`} open={purchaseOpen} onOpenChange={setPurchaseOpen} submitLabel={queued ? '确认收款并安排下一期' : '确认收款并立即开通'} onSubmit={data => {
      if (!purchase) throw new Error('请选择一个可开通的套餐。');
      return ops.submit('开通 / 续费套餐', path, { product_id: purchase.product_id, product_revision: purchase.revision, paid_at: timestamp(String(data.get('paid_at'))), channel: String(data.get('channel') ?? '').trim(), external_ref: String(data.get('external_ref') ?? '').trim() || null, note: String(data.get('note') ?? '').trim() });
    }}>
      <Field label="选择套餐"><select name="product_id" value={productId} onChange={event => setProductId(event.target.value)}>{available.map(product => <option key={product.product_id} value={product.product_id}>{product.title} · {durationLabels[product.duration]} · {money(product.amount_fen, 2)} 元</option>)}</select></Field>
      {purchase && <div className="my-3 space-y-2 rounded-lg border bg-muted/30 p-4"><p className="font-semibold">确认实收 {money(purchase.amount_fen, 2)} 元</p><p>发放 {money(planBudget(purchase))} 元 AI 预算 · 上传空间 {storage(purchase.storage_bytes!)} · {durationLabels[purchase.duration]}</p><p className="text-sm">{queued ? `从 ${date(user.data!.purchase_starts_at)}（香港时间）起接续已有安排。` : '确认后立即生效。'}旧期剩余额度不结转。</p></div>}
      <Field label="实际付款时间（香港）"><Input name="paid_at" type="datetime-local" defaultValue={dateInput(Math.floor(Date.now() / 1000))} required /></Field>
      <Field label="收款渠道"><Input name="channel" defaultValue="微信" required /></Field>
      <Field label="交易编号（选填）"><Input name="external_ref" /></Field>
      <Field label="备注（选填）"><Input name="note" /></Field>
    </OperationDialog>
  </section>;
}

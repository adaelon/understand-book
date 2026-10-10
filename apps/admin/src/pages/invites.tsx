import { useMemo, useRef, useState } from "react";
import { ApiError, queryClient, session } from "@/lib/api";
import { InviteJournal, formatInvite, type Invite, type InviteBatch } from "@/lib/invites";
import { date } from "@/lib/format";
import { useData, DataState, Pagination } from "@/components/data";
import { Field } from "@/components/operation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";

const labels = { unused: "未使用", used: "已使用", disabled: "已停用" };
export function InvitesPage() {
  const { identity, epoch } = session.snapshot();
  const journal = useMemo(() => new InviteJournal(identity!.user_id, sessionStorage,
    <T,>(path: string, method?: string, body?: unknown) => {
      if (session.snapshot().epoch !== epoch) throw new ApiError(409, "CLIENT_IDENTITY_STALE", "身份已改变");
      return session.request<T>(path, method, body, AbortSignal.timeout(15_000));
    }), [identity!.user_id, epoch]);
  const [pending, setPending] = useState(() => journal.pending());
  const [batch, setBatch] = useState<InviteBatch | null>(null);
  const [count, setCount] = useState("1"), [state, setState] = useState(""), [offset, setOffset] = useState(0);
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [notice, setNotice] = useState("");
  const [disable, setDisable] = useState<Invite | null>(null), [disableError, setDisableError] = useState("");
  const running = useRef(false);
  const query = useData<{ invites: Invite[]; total: number }>(`/admin/invites?limit=20&offset=${offset}${state ? `&state=${state}` : ""}`);
  async function execute(action: () => Promise<InviteBatch | null>) {
    if (running.current) return;
    running.current = true;
    setBusy(true); setError(""); setNotice("");
    try {
      const result = await action();
      if (result) {
        setBatch(result);
        void queryClient.invalidateQueries();
      } else setError("暂未查到原批次，可按原操作重试。数量和操作号保持不变。");
    } catch (failure) {
      setError(failure instanceof ApiError ? failure.message : "生成结果尚未确认，请核对原批次或按原操作重试。");
    } finally {
      setPending(journal.pending()); running.current = false; setBusy(false);
    }
  }
  async function copy(text: string) {
    setError(""); setNotice("");
    try { await navigator.clipboard.writeText(text); setNotice("已复制内测码。"); }
    catch { setError("复制未成功，请选中内测码文字手动复制。"); }
  }
  async function confirmDisable() {
    if (!disable || running.current) return;
    running.current = true; setBusy(true); setDisableError("");
    try {
      const result = await session.request<Invite>(`/admin/invites/${encodeURIComponent(disable.invite_id)}/disable`, "POST", {}, AbortSignal.timeout(15_000));
      setBatch(current => current ? { ...current, invites: current.invites.map(i => i.invite_id === result.invite_id ? result : i) } : null);
      setDisable(null); setNotice("内测码已停用。");
    } catch (failure) {
      setDisableError(failure instanceof ApiError ? failure.message : "停用结果尚未确认，可再次确认同一码；重复停用不会改变结果。");
    } finally {
      void queryClient.invalidateQueries(); running.current = false; setBusy(false);
    }
  }
  function cards(invites: Invite[]) {
    return <div className="grid min-w-0 gap-3 lg:grid-cols-2">{invites.map(invite => <article key={invite.invite_id} className="min-w-0 space-y-3 rounded-lg border p-4">
      <div className="flex flex-wrap items-center justify-between gap-2"><code className="select-all break-all text-sm font-semibold">{formatInvite(invite.code)}</code><Badge variant="outline">{labels[invite.state]}</Badge></div>
      <p className="break-all text-sm text-muted-foreground">生成：{date(invite.created_at)} · {invite.actor}</p>
      {invite.state === "used" && <p className="break-all text-sm">使用账号：{invite.used_by}<br />邮箱：{invite.used_email ?? "未绑定邮箱"}<br />使用时间：{date(invite.used_at)}</p>}
      {invite.state === "disabled" && <p className="text-sm text-muted-foreground">停用：{date(invite.disabled_at)}</p>}
      <div className="flex flex-wrap gap-2"><Button variant="outline" onClick={() => void copy(formatInvite(invite.code))}>复制</Button>{invite.state === "unused" && <Button variant="ghost" disabled={busy} onClick={() => { setDisable(invite); setDisableError(""); }}>停用</Button>}</div>
    </article>)}</div>;
  }
  return <>
    <div><h1>内测码</h1><p className="mt-2 text-sm text-muted-foreground">生成后手动发送给受邀者。每码可注册一个账号，长期有效；材料与使用额度另行开通。</p></div>
    <form className="flex flex-wrap items-end gap-3" onSubmit={event => {
      event.preventDefault();
      void execute(async () => {
        journal.prepare(Number(count)); setPending(journal.pending()); setBatch(null);
        try { return await journal.send(); }
        catch (failure) { if (journal.pending()) { const found = await journal.lookup(); if (found) return found; } throw failure; }
      });
    }}><Field label="生成数量（1–100）"><Input type="number" min={1} max={100} step={1} required value={count} disabled={busy || !!pending} onChange={e => setCount(e.target.value)} /></Field><Button className="mb-2" disabled={busy || !!pending} type="submit">生成内测码</Button></form>
    {pending && <section aria-label="待核对批次" className="space-y-3 rounded-lg border border-amber-500 p-4"><h2>有一批内测码待核对</h2><p className="break-all text-sm">数量：{pending.count} · 操作号：{pending.operation_id}</p><p className="text-sm">刷新页面后仍保留原批次。重试会先回查，再使用同一操作号提交。</p><div className="flex flex-wrap gap-2"><Button disabled={busy} onClick={() => void execute(() => journal.recover(false))}>核对原批次</Button><Button disabled={busy} variant="outline" onClick={() => void execute(() => journal.recover(true))}>按原操作重试</Button></div></section>}
    {error && <p role="alert" className="text-destructive">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {batch && <section aria-label="本次批次" className="space-y-3"><div className="flex flex-wrap items-center gap-3"><h2>本次批次 · {batch.count} 个</h2><Button variant="outline" disabled={!batch.invites.some(i => i.state === "unused")} onClick={() => void copy(batch.invites.filter(i => i.state === "unused").map(i => formatInvite(i.code)).join("\n"))}>复制本批未使用码</Button><Button variant="ghost" onClick={() => setBatch(null)}>收起批次</Button></div>{cards(batch.invites)}</section>}
    <section className="space-y-3"><div className="flex flex-wrap items-center gap-3"><h2>全部内测码</h2><Field label="内测码状态"><select value={state} onChange={e => { setState(e.target.value); setOffset(0); }}><option value="">全部状态</option>{Object.entries(labels).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></Field><Button variant="outline" onClick={() => void query.refetch()}>刷新列表</Button></div><DataState query={query} />{query.data && <>{cards(query.data.invites)}{query.data.total === 0 && <p className="py-6 text-muted-foreground">暂无内测码</p>}<Pagination offset={offset} total={query.data.total} change={setOffset} /></>}</section>
    <Dialog open={!!disable} onOpenChange={open => { if (!open && !busy) setDisable(null); }}><DialogContent><DialogHeader><DialogTitle>停用内测码</DialogTitle><DialogDescription>停用后不能恢复，受邀者将无法使用此码完成注册。</DialogDescription></DialogHeader><code className="break-all">{disable ? formatInvite(disable.code) : ""}</code>{disableError && <p role="alert" className="text-destructive">{disableError}</p>}<Button disabled={busy} onClick={() => void confirmDisable()}>确认停用</Button></DialogContent></Dialog>
  </>;
}

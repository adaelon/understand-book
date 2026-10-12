// @vitest-environment happy-dom
import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { queryClient, session } from "@/lib/api";
import type { McpUsage, Period } from "@/lib/types";
import { OperationsProvider } from "@/components/operation";
import { McpPanel } from "./mcp";
import { AllowancePanel, HistoryPanel } from "./allowance";

const base = "/api/admin/users/A";
const pack = {
  pack_id: "pack-one", receipt_id: "receipt-mcp", pack_count: 2, unit_price_fen: 1000,
  units_per_pack: 1000, price_fen: 2000, total_units: 2000, adjusted_units: -100,
  used_units: 500, remaining_units: 1400, available_units: 1400,
  opened_at: "2026-10-11T10:00:00+08:00", expires_at: "2027-10-11T10:00:00+08:00", revision: 3,
};
const period: Period = { period_id: "model-period", starts_at: 1791648000, expires_at: 1891648000, revision: 2,
  balance: { granted_micro_cny: 1000000, debited_micro_cny: 0, active_reserved_micro_cny: 0, pending_micro_cny: 0, available_micro_cny: 1000000 } };
let usage: McpUsage;
let root: Root | undefined;
let host: HTMLDivElement;
let fetcher: ReturnType<typeof vi.fn>;
let mutate: (path: string, body: Record<string, unknown>) => Promise<Response>;
let lookup: () => Promise<Response>;
const page = (items: unknown[]) => ({ items, total: items.length, limit: 20, offset: 0 });
const receipt = (body: Record<string, unknown>, kind: string) => ({
  operation_id: body.operation_id, user_id: "A", actor: "operator", kind, created_at: 1791684000,
  ok: true, parameters: body, receipt_id: pack.receipt_id, call_pack: pack,
});
const writes = () => fetcher.mock.calls.filter(([, init]) => init?.method === "POST");
const text = () => document.body.textContent ?? "";
async function settle() { await act(async () => { await new Promise(r => setTimeout(r, 25)); }); }
async function mount(children: ReactNode = <McpPanel userId="A" />) {
  host = document.createElement("div"); document.body.append(host); root = createRoot(host);
  await act(async () => root!.render(<QueryClientProvider client={queryClient}><OperationsProvider>{children}</OperationsProvider></QueryClientProvider>));
  await settle();
}
async function unmount() { if (root) { await act(async () => root!.unmount()); root = undefined; host.remove(); } }
async function click(label: string, index = 0) {
  const button = [...document.querySelectorAll("button")].filter(b => b.textContent === label)[index];
  expect(button, label).toBeDefined();
  await act(async () => button.click()); await settle();
}
async function fill(name: string, value: string) {
  const input = document.querySelector<HTMLInputElement>(`[name="${name}"]`)!;
  expect(input, name).not.toBeNull();
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  });
}
async function submit() {
  await act(async () => document.querySelector("[role=dialog] form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  await settle();
}
beforeEach(async () => {
  (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  sessionStorage.clear(); queryClient.clear();
  usage = { month_hk: "2026-10", free_granted: 1050, free_used: 50, free_remaining: 1000,
    free_resets_at: "2026-11-01T00:00:00+08:00", paid_remaining: 1400,
    paid_next_expires_at: pack.expires_at, call_packs: [pack, { ...pack, pack_id: "expired-pack", receipt_id: "old-receipt", available_units: 0, expires_at: "2026-09-01T00:00:00+08:00" }] };
  mutate = async (path, body) => Response.json(path.endsWith("/gifts")
    ? { ...receipt(body, "mcp-free-gift"), call_pack: undefined, receipt_id: undefined, month_hk: "2026-10", usage: { ...usage, free_remaining: 1050 } }
    : receipt(body, path.endsWith("/corrections") ? "mcp-corrections" : "mcp-packs"));
  lookup = async () => Response.json({ error_code: "NOT_FOUND" }, { status: 404 });
  fetcher = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/auth/me") return Response.json({ user_id: "operator", csrf_token: "csrf", capabilities: { admin: true } });
    if (init?.method === "POST") return mutate(url, JSON.parse(String(init.body)));
    if (url.startsWith("/api/admin/operations/")) return lookup();
    if (url === `${base}/mcp`) return Response.json(usage);
    if (url.startsWith(`${base}/allowance-periods`)) return Response.json(page([period]));
    if (url.startsWith(`${base}/receipts`)) return Response.json(page([
      { receipt_id: "receipt-mcp", entitlement_kind: "mcp_call_pack", pack_id: pack.pack_id, period_id: null, amount_fen: 2000, effective_amount_fen: 1500, paid_at: 1791684000, channel: "微信", external_ref: "tx-mcp", note: "MCP" },
      { receipt_id: "receipt-model", entitlement_kind: "model_allowance", pack_id: null, period_id: period.period_id, amount_fen: 1000, effective_amount_fen: 900, paid_at: 1791684000, channel: "微信", external_ref: "tx-model", note: "模型" },
    ]));
    if (url.startsWith(`${base}/receipt-corrections`)) return Response.json(page([
      { entitlement_kind: "mcp_call_pack", receipt_id: "receipt-mcp", delta_fen: -500, reason: "MCP 金额误录", created_at: 1791684000 },
      { entitlement_kind: "model_allowance", receipt_id: "receipt-model", delta_fen: -100, reason: "模型金额误录", created_at: 1791684000 },
    ]));
    throw new Error(`Unexpected request ${url}`);
  });
  vi.stubGlobal("fetch", fetcher); await session.refresh();
});
afterEach(async () => { await unmount(); queryClient.clear(); vi.unstubAllGlobals(); });

it("shows free, paid, expired batch history and Hong Kong reset/expiry separately", async () => {
  await mount();
  expect(text()).toContain("当月免费剩余 1000 次");
  expect(text()).toContain("有效已购剩余 1400 次");
  expect(text()).toContain("1400 / 0");
  expect(text()).toContain("2000 / -100 / 500");
  expect(text()).toContain("2026年11月1日 00:00");
  expect(text()).toContain("2027年10月11日 10:00");
  expect(writes()).toHaveLength(0);
});

it("previews integer packs and submits the fixed-price contract with CSRF", async () => {
  await mount(); await click("登记 MCP 购包");
  await fill("pack_count", "3"); await fill("paid_at", "2026-10-11T09:00");
  await fill("external_ref", " tx-new "); await fill("note", "已核对到账");
  expect(text()).toContain("3 包 · 确认收款 30.00 元 · 开通 3000 次");
  await submit();
  const [path, options] = writes()[0];
  expect(path).toBe(`${base}/mcp/packs`);
  expect(JSON.parse(options.body)).toEqual({ pack_count: 3, paid_at: Date.parse("2026-10-11T09:00+08:00") / 1000,
    channel: "微信", external_ref: "tx-new", note: "已核对到账", operation_id: expect.any(String) });
  expect(options.headers["X-CSRF-Token"]).toBe("csrf");
  expect(text()).toContain("操作已完成 · A");
  expect(text()).toContain("原操作回执");
});

it("rejects fractional, zero and unsafe pack counts before preparing an operation", async () => {
  await mount(); await click("登记 MCP 购包");
  for (const value of ["1.5", "0", "-1", "9007199254741"]) {
    await fill("pack_count", value); await submit();
    expect(writes()).toHaveLength(0); expect(sessionStorage.length).toBe(0);
    expect(text()).toMatch(/包数须为正整数|包数超出可输入范围/);
  }
});

it("gifts positive monthly units with a reason and shows the original gift month", async () => {
  await mount(); await click("赠送当月次数");
  await fill("units", "0"); await submit(); expect(writes()).toHaveLength(0);
  await fill("units", "50"); await fill("reason", " 反馈奖励 "); await submit();
  expect(JSON.parse(writes()[0][1].body)).toEqual({ units: 50, reason: "反馈奖励", operation_id: expect.any(String) });
  expect(writes()[0][0]).toBe(`${base}/mcp/gifts`);
  expect(text()).toContain("赠送月份：2026-10");
  expect(text()).toContain("免费剩余 1050 次");
});

it("sends only one gift while duplicate form submissions are in flight", async () => {
  let finish!: () => void;
  mutate = (_path, body) => new Promise(resolve => { finish = () => resolve(Response.json({
    ...receipt(body, "mcp-free-gift"), call_pack: undefined, month_hk: "2026-10", usage,
  })); });
  await mount(); await click("赠送当月次数"); await fill("units", "25"); await fill("reason", "补赠");
  const form = document.querySelector("[role=dialog] form")!;
  await act(async () => {
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
  expect(writes()).toHaveLength(1);
  await act(async () => finish()); await settle();
  expect(sessionStorage.length).toBe(0); expect(text()).toContain("操作已完成 · A");
});

it("shows unavailable account errors and prevents writes until usage can be read", async () => {
  fetcher.mockImplementation(async (url: string) => {
    if (url === `${base}/mcp`) return Response.json({ error_code: "USER_UNAVAILABLE" }, { status: 404 });
    throw new Error(`Unexpected request ${url}`);
  });
  await mount();
  expect(text()).toContain("账号已停用或不可用");
  expect([...document.querySelectorAll("button")].find(b => b.textContent === "登记 MCP 购包")?.disabled).toBe(true);
  expect(writes()).toHaveLength(0);
});

it.each([["-5.25", "0", -525, 0], ["0", "-100", 0, -100]])(
  "keeps cash and units independent for correction %s / %s", async (amount, units, fen, delta) => {
    await mount(); await click("更正 MCP 收款 / 次数");
    await fill("amount", amount); await fill("units", units); await fill("reason", "误录更正"); await submit();
    expect(writes()[0][0]).toBe(`${base}/mcp/corrections`);
    expect(JSON.parse(writes()[0][1].body)).toEqual({ receipt_id: "receipt-mcp", revision: 3, delta_fen: fen, delta_units: delta, reason: "误录更正", operation_id: expect.any(String) });
  },
);

it("rejects empty corrections and reclaiming consumed units", async () => {
  await mount(); await click("更正 MCP 收款 / 次数");
  await fill("reason", "误录"); await submit(); expect(text()).toContain("至少一项非零");
  await fill("units", "-1401"); await submit(); expect(text()).toContain("只能扣回尚未消费的次数");
  expect(writes()).toHaveLength(0);
});

it("preserves the exact purchase across timeout and reload, looks up before retrying", async () => {
  mutate = async () => { throw new TypeError("网络中断"); };
  await mount(); await click("登记 MCP 购包"); await fill("pack_count", "2"); await fill("note", "确认收款"); await submit();
  const original = JSON.parse(writes()[0][1].body);
  expect(sessionStorage.length).toBe(1); expect(text()).toContain("有一笔操作待核对");
  expect(text()).toContain("20.00 元 / 2000 次");
  await unmount(); await mount();
  expect(text()).toContain(original.operation_id);
  expect([...document.querySelectorAll("button")].find(b => b.textContent === "登记 MCP 购包")?.disabled).toBe(true);
  mutate = async (_path, body) => Response.json(receipt(body, "mcp-packs"));
  await click("按原操作重试");
  expect(writes()).toHaveLength(2);
  expect(JSON.parse(writes()[1][1].body)).toEqual(original);
  const calls = fetcher.mock.calls, postIndex = calls.length - 1 - [...calls].reverse().findIndex(([, opts]) => opts?.method === "POST");
  expect(calls[postIndex - 1][0]).toBe(`/api/admin/operations/${original.operation_id}`);
  expect(sessionStorage.length).toBe(0);
});

it("recovers an already committed correction by receipt without a second POST", async () => {
  let saved: Record<string, unknown>;
  mutate = async (_path, body) => { saved = body; throw new TypeError("timeout"); };
  lookup = async () => Response.json(receipt(saved, "mcp-corrections"));
  await mount(); await click("更正 MCP 收款 / 次数");
  await fill("units", "-100"); await fill("reason", "误录"); await submit();
  expect(writes()).toHaveLength(1); expect(sessionStorage.length).toBe(0);
  expect(text()).toContain("操作已完成 · A"); expect(text()).toContain("次数差额（次）-100");
});

it("reloads a conflicting revision and uses a new operation only after rejection", async () => {
  mutate = async () => {
    usage = { ...usage, call_packs: [{ ...pack, revision: 4 }] };
    return Response.json({ error_code: "MCP_PACK_REVISION_CONFLICT" }, { status: 409 });
  };
  await mount(); await click("更正 MCP 收款 / 次数"); await fill("units", "-100"); await fill("reason", "误录"); await submit();
  expect(text()).toContain("调用包已被更正"); expect(sessionStorage.length).toBe(0);
  mutate = async (_path, body) => Response.json(receipt(body, "mcp-corrections"));
  await click("更正 MCP 收款 / 次数"); await fill("units", "-100"); await fill("reason", "核对后更正"); await submit();
  const bodies = writes().map(([, opts]) => JSON.parse(opts.body));
  expect(bodies[1].revision).toBe(4); expect(bodies[1].operation_id).not.toBe(bodies[0].operation_id);
});

it("routes mixed receipts to their own correction forms and preserves model correction", async () => {
  await mount(<HistoryPanel userId="A" />);
  expect(text()).toContain("MCP 调用包 · pack-one"); expect(text()).toContain("模型使用额度 · model-period");
  expect(text()).toContain("20.00 / 15.00"); expect(text()).toContain("10.00 / 9.00");
  await click("追加更正", 0);
  expect(document.querySelector('[name="delta"]')).toBeNull();
  await fill("amount", "-1"); await fill("reason", "MCP 金额"); await submit();
  expect(writes()[0][0]).toBe(`${base}/mcp/corrections`);
  await click("追加更正", 1);
  expect(document.querySelector('[name="units"]')).toBeNull();
  await fill("amount", "-1"); await fill("delta", "-0.5"); await fill("reason", "模型金额"); await submit();
  expect(writes()[1][0]).toBe(`${base}/receipt-corrections`);
  expect(JSON.parse(writes()[1][1].body)).toMatchObject({ receipt_id: "receipt-model", period_id: "model-period", revision: 2, delta_fen: -100, delta_micro_cny: -500000 });
});

it("labels both purposes in appended cash corrections", async () => {
  await mount(<HistoryPanel userId="A" />);
  const select = document.querySelector("select")!;
  await act(async () => { select.value = "receipt-corrections"; select.dispatchEvent(new Event("change", { bubbles: true })); }); await settle();
  expect(text()).toContain("MCP 调用包 · receipt-mcp"); expect(text()).toContain("模型使用额度 · receipt-model");
  expect(text()).toContain("-5.00"); expect(text()).toContain("-1.00");
});

it("retains the original model receipt amount and micro-currency grant contract", async () => {
  await mount(<AllowancePanel userId="A" current={period} />); await click("登记收款");
  await fill("amount", "12.50"); await fill("delta", "7.123456"); await fill("note", "已收款"); await submit();
  expect(writes()[0][0]).toBe(`${base}/receipts`);
  expect(JSON.parse(writes()[0][1].body)).toMatchObject({ period_id: "model-period", revision: 2, amount_fen: 1250, delta_micro_cny: 7123456 });
});

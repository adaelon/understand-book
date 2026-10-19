import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import type { BuildControlConfig } from "../src/build-control.ts";

/** HTTP contract fixture; monetary ledger invariants are tested against the Rust service. */
export async function costFixture() {
  const calls: { path: string; body: any }[] = [];
  const charges = new Map<string, any>();
  let block = false;
  let receiptOffline = false;
  let budgetLimit = 10000;
  const server = createServer(async (req, res) => {
    let text = ""; for await (const chunk of req) text += chunk;
    const body = text ? JSON.parse(text) : undefined;
    const route = req.url!; calls.push({ path: route, body });
    res.setHeader("Content-Type", "application/json");
    if (req.headers.cookie !== "operator-test" || req.headers["x-csrf-token"] !== "csrf-test") {
      res.writeHead(403).end("{}"); return;
    }
    if (route.endsWith("/admit")) {
      if (block) { res.writeHead(409).end("{}"); return; }
      charges.set(body.call_id, { state: "sent" });
      res.end(JSON.stringify({ admitted: true, call_id: body.call_id })); return;
    }
    if (route.includes("/charges/")) {
      const charge = charges.get(route.split("/charges/")[1]);
      if (!charge) res.writeHead(404).end("{}");
      else res.end(JSON.stringify({ charge }));
      return;
    }
    if (req.method === 'GET' && /^\/api\/admin\/build-tasks\/[^/]+$/u.test(route)) {
      res.end(JSON.stringify({cost_owner:'platform',max_micro_cny:budgetLimit})); return;
    }
    if (route.endsWith("/receipts") && receiptOffline) { res.writeHead(503).end("{}"); return; }
    res.end("{}");
  });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  const config: NonNullable<BuildControlConfig["externalCost"]> = {
    binding: { version: "external_build_cost.v1", cost_owner: "platform", max_micro_cny: 10000,
      reserved_per_execution_micro_cny: 100, confirmation: "operator-confirmed-cost" },
    connection: { base_url: url, origin: url, cookie: "operator-test", csrf_token: "csrf-test" },
  };
  return { config, calls, budgetLimit:(value:number)=>{budgetLimit=value;}, rejectAdmissions: () => { block = true; }, receiptsOffline: (v: boolean) => { receiptOffline = v; },
    admissions: () => calls.filter(c => c.path.endsWith("/admit")).map(c => c.body),
    receipts: () => calls.filter(c => c.path.endsWith("/receipts")).map(c => c.body),
    close: () => new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve())) };
}

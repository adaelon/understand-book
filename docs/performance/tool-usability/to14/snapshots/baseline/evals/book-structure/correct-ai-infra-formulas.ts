import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** Source-reviewed corrections to the isolated ai-infra copy, never to canonical source or the old publication. */
export function correctAiInfraFormulas(workspace: string, receipt: string) {
  const file = path.join(workspace, "formula_semantics.json");
  const value = JSON.parse(readFileSync(file, "utf8"));
  const items = Array.isArray(value) ? value : value.items;
  const before = structuredClone(items.filter((f: any) => ["10.8.3.21", "10.9.2.13", "10.12.4.7", "10.13.4.20", "10.13.4.51"].includes(f.formula_lid)));
  if (before.length !== 5) throw Error("Expected the five source-reviewed ai-infra formulas");
  const item = (lid: string) => items.find((f: any) => f.formula_lid === lid);
  Object.assign(item("10.8.3.21").composition, { meaning: "长请求输入8192 token，加上送回模型的前255个输出token，共计算8447 token；短请求输入为2048。",
    evidence_lids: ["10.8.3.18", "10.8.3.19", "10.8.3.20", "10.8.3.21"] });
  const width = item("10.9.2.13");
  Object.assign(width.parameters.find((p: any) => p.symbol === "s_w"), { label: "每权重元素字节数", meaning: "每个权重元素占用的字节数，BF16为2 bytes。", unit: "bytes/权重元素",
    evidence_lids: ["10.9.2.8", "10.9.2.9", "10.9.2.10", "10.9.2.11", "10.9.2.12", "10.9.2.13"] });
  Object.assign(width.composition, { meaning: "BF16每个权重元素占2 bytes；批内权重只读取一次、乘加计两次运算时，权重读取的算术强度约为2b/s_w。",
    evidence_lids: ["10.9.2.8", "10.9.2.9", "10.9.2.10", "10.9.2.11", "10.9.2.12", "10.9.2.13"] });
  Object.assign(item("10.12.4.7").composition, { meaning: "并行生成整段草稿只需一次3.57 ms前向，加上26.26 ms目标验证，平均每轮3个输出，得到约9.9 ms/token；同条件四次串行前向为13.5 ms/token。",
    evidence_lids: ["10.12.4.4", "10.12.4.5", "10.12.4.6", "10.12.4.7", "10.12.4.8"] });
  const rate = item("10.13.4.20");
  Object.assign(rate.parameters.find((p: any) => p.symbol === "r_D"), { label: "请求decode调用速率", meaning: "同一批次16条请求每27.35 ms各执行一次decode，约585请求decode调用/s；不是每秒新输出token的统计。", unit: "请求decode调用/s",
    evidence_lids: ["10.13.4.19", "10.13.4.20", "10.13.4.21"] });
  Object.assign(rate.composition, { meaning: "16条请求除以每轮0.02735秒，得到约585请求decode调用/s；首输出由prefill产生，随后255次decode用于计算每请求GPU占用份额。",
    evidence_lids: ["10.13.4.19", "10.13.4.20", "10.13.4.21", "10.13.4.22", "10.13.4.23"] });
  Object.assign(item("10.13.4.51").composition, { meaning: "输出缩至16 token时，方案D为2.23+5×0.02735≈2.37秒；方案B为2.04+15×0.02735≈2.45秒，D仅快0.08秒。",
    evidence_lids: ["10.13.4.48", "10.13.4.49", "10.13.4.50", "10.13.4.51", "10.13.4.52"] });
  const corrected = before.map((f: any) => ({ formula_lid: f.formula_lid, before: f, after: item(f.formula_lid) }));
  const source = readFileSync(path.join(workspace, "source.txt"), "utf8");
  const base = JSON.parse(readFileSync(path.join(workspace, "base.json"), "utf8"));
  const lids = [...new Set<string>(corrected.flatMap((f: any) => [
    ...f.after.composition.evidence_lids, ...f.after.parameters.flatMap((p: any) => p.evidence_lids)]))];
  const reviewed_source = lids.map(lid => { const n = base.lid_nodes.find((n: any) => n.lid === lid);
    if (!n) throw Error(`Missing correction source ${lid}`);
    return { lid, text: source.slice(n.span.start, n.span.end) }; });
  writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
  writeFileSync(receipt, JSON.stringify({ status: "corrected", workspace, corrected, reviewed_source }, null, 2) + "\n");
  return { corrected: corrected.length, receipt };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(JSON.stringify(correctAiInfraFormulas(process.argv[2], process.argv[3]), null, 2));
}

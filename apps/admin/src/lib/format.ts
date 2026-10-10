// Parse decimal currency using integer arithmetic. Never multiply binary floating-point amounts.
export function moneyInput(text: string, digits = 6): number {
  const match = text.trim().match(/^(-?)(\d+)(?:\.(\d+))?$/);
  if (!match || (match[3]?.length ?? 0) > digits)
    throw new Error(`金额最多保留 ${digits} 位小数。`);
  const value =
    BigInt(match[2] + (match[3] ?? "").padEnd(digits, "0")) *
    (match[1] ? -1n : 1n);
  const number = Number(value);
  if (!Number.isSafeInteger(number)) throw new Error("金额超出可输入范围。");
  return number;
}
export function money(value: number | null | undefined, digits = 6): string {
  if (value == null) return "未知";
  if (!Number.isSafeInteger(value)) return "金额超出显示范围";
  const amount = BigInt(value),
    sign = amount < 0 ? "-" : "",
    absolute = amount < 0 ? -amount : amount;
  const scale = 10n ** BigInt(digits);
  const fraction = (absolute % scale)
    .toString()
    .padStart(digits, "0")
    .replace(/0+$/, "")
    .padEnd(2, "0");
  return `${sign}${absolute / scale}.${fraction}`;
}
export function date(value: number | null | undefined) {
  return value == null
    ? "—"
    : new Intl.DateTimeFormat("zh-CN", {
        timeZone: "Asia/Hong_Kong",
        dateStyle: "medium",
        timeStyle: "short",
      }).format(new Date(value * 1000));
}
export function dateInput(value: number) {
  return new Date((value + 8 * 3600) * 1000).toISOString().slice(0, 16);
}
export function timestamp(value: string) {
  const parsed = Date.parse(`${value}+08:00`);
  if (!Number.isFinite(parsed)) throw new Error("请输入有效日期时间。");
  return Math.floor(parsed / 1000);
}

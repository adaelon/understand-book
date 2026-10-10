import { expect, it, vi } from "vitest";
import { ApiError } from "./api";
import { OperationJournal } from "./operations";
function storage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    key: (index: number) => [...values.keys()][index] ?? null,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  };
}
it("persists an unresolved write across reload, queries its receipt first, and never saves passwords", async () => {
  const saved = storage();
  const request = vi.fn().mockRejectedValueOnce(new TypeError("disconnected"));
  const first = new OperationJournal("operator", saved, request);
  const operation = first.prepare("create", "/admin/users", {
    user_id: "reader",
    password: "private-password",
  });
  await expect(first.send("private-password")).rejects.toThrow("disconnected");
  expect(JSON.stringify(first.pending())).not.toContain("private-password");
  const reloaded = new OperationJournal("operator", saved, request);
  expect(reloaded.pending()?.operation_id).toBe(operation.operation_id);
  expect(() => reloaded.prepare("second", "/admin/users", {})).toThrow(
    "请先核对",
  );
  request.mockResolvedValueOnce({
    operation_id: operation.operation_id,
    ok: true,
  });
  expect(await reloaded.lookup()).toMatchObject({ ok: true });
  expect(reloaded.pending()).toBeNull();
  expect(request.mock.calls.filter((call) => call[1] === "POST")).toHaveLength(
    1,
  );
});
it("a missing receipt retains frozen amounts and the retry id; another user sees no operation", async () => {
  const saved = storage();
  const request = vi
    .fn()
    .mockRejectedValueOnce(new ApiError(404, "NOT_FOUND", "missing"));
  const journal = new OperationJournal("operator", saved, request),
    op = journal.prepare("receipt", "/admin/users/A/receipts", {
      amount_fen: 125,
      delta_micro_cny: 1,
    });
  expect(await journal.lookup()).toBeNull();
  expect(new OperationJournal("reader", saved, request).pending()).toBeNull();
  request.mockResolvedValueOnce({ ok: true, operation_id: op.operation_id });
  await journal.send();
  expect(request.mock.calls[1]).toEqual([
    op.path,
    "POST",
    { ...op.body, operation_id: op.operation_id },
  ]);
});
it("definitive revision conflicts permit correction, unknown or identity failures keep the pending key", async () => {
  for (const [status, code, remains] of [
    [409, "ALLOWANCE_REVISION_CONFLICT", false],
    [409, "CLIENT_IDENTITY_STALE", true],
    [503, "UNAVAILABLE", true],
    [403, "CSRF_REJECTED", true],
  ] as const) {
    const journal = new OperationJournal(
      "operator",
      storage(),
      vi.fn().mockRejectedValue(new ApiError(status, code, code)),
    );
    journal.prepare("gift", "/admin/users/A/allowance-adjustments", {
      delta_micro_cny: 50,
    });
    await expect(journal.send()).rejects.toThrow(code);
    expect(!!journal.pending()).toBe(remains);
  }
});

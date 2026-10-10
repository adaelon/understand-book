import { expect, it, vi } from "vitest";
import { ApiError } from "./api";
import { InviteJournal, formatInvite } from "./invites";

function storage(): Storage {
  const values = new Map<string,string>();
  return { get length() { return values.size; }, clear: () => values.clear(), key: i => [...values.keys()][i] ?? null,
    getItem: key => values.get(key) ?? null, setItem: (key,value) => { values.set(key,value); }, removeItem: key => { values.delete(key); } };
}
it("inv2_ recovers a committed batch after timeout and reload without another POST", async () => {
  const saved = storage(), request = vi.fn().mockRejectedValueOnce(new DOMException("Timed out", "TimeoutError"));
  const first = new InviteJournal("admin",saved,request);
  first.prepare(3);
  const pending = first.pending();
  await expect(first.send()).rejects.toThrow("Timed out");
  const reload = new InviteJournal("admin",saved,request);
  expect(reload.pending()).toEqual(pending);
  expect(() => reload.prepare(1)).toThrow("核对");
  expect(new InviteJournal("other",saved,request).pending()).toBeNull();
  request.mockResolvedValueOnce({...pending,invites:[]});
  expect(await reload.recover(true)).toMatchObject(pending!);
  expect(request.mock.calls.map(c => c[1] ?? "GET")).toEqual(["POST","GET"]);
  expect(reload.pending()).toBeNull();
});
it("inv2_ a missing batch retries the frozen id and count, lookup failure never submits", async () => {
  const request = vi.fn().mockRejectedValueOnce(new TypeError("offline"));
  const journal = new InviteJournal("admin",storage(),request);
  journal.prepare(100);
  const pending = journal.pending();
  await expect(journal.recover(true)).rejects.toThrow("offline");
  expect(request.mock.calls).toHaveLength(1);
  request.mockRejectedValueOnce(new ApiError(404,"NOT_FOUND","missing")).mockResolvedValueOnce({...pending,invites:[]});
  await journal.recover(true);
  expect(request.mock.calls[2]).toEqual(["/admin/invite-batches","POST",pending]);
  expect(journal.pending()).toBeNull();
});
it("inv2_ unknown, conflict and stale-session outcomes retain the batch; invalid input can be corrected", async () => {
  for (const [status,code,retained] of [[503,"UNAVAILABLE",true],[409,"INVITE_BATCH_CONFLICT",true],[409,"CLIENT_IDENTITY_STALE",true],[403,"CSRF_REJECTED",true],[400,"INVALID_REQUEST",false]] as const) {
    const journal = new InviteJournal("admin",storage(),vi.fn().mockRejectedValue(new ApiError(status,code,code)));
    journal.prepare(1);
    await expect(journal.send()).rejects.toThrow(code);
    expect(!!journal.pending()).toBe(retained);
  }
});
it("inv2_ checks quantity before saving and never sends when storage cannot preserve the operation", () => {
  const saved = storage(), request = vi.fn(), journal = new InviteJournal("admin",saved,request);
  for (const count of [0,101,1.5,NaN]) expect(() => journal.prepare(count)).toThrow("1–100");
  saved.setItem = () => { throw new Error("storage full"); };
  expect(() => journal.prepare(1)).toThrow("storage full");
  expect(request).not.toHaveBeenCalled();
  expect(journal.pending()).toBeNull();
  expect(formatInvite("ABCDE23456FGHJK789AB")).toBe("ABCDE-23456-FGHJK-789AB");
});

import { expect, it, vi } from "vitest";
import { AdminSession } from "./api";
it('INV5 stores verified email separately from the server account ID', async () => {
  const value = { user_id: 'legacy', email: 'reader@example.com', csrf_token: 'token', capabilities: { admin: true } };
  const transport = vi.fn(async () => Response.json(value));
  const session = new AdminSession(transport as typeof fetch, vi.fn());
  await session.refresh();
  expect(session.snapshot().identity).toEqual(value);
  await session.request('/admin/users/legacy', 'POST', { disabled: false });
  expect(transport).toHaveBeenLastCalledWith('/api/admin/users/legacy', expect.objectContaining({ headers: expect.objectContaining({ 'X-CSRF-Token': 'token' }) }));
});
const identity = (token = "first", admin = true) => ({
  user_id: "operator",
  csrf_token: token,
  capabilities: { admin },
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
it("rejects an old account response after a same-user session change and clears queries", async () => {
  const delayed = deferred<Response>();
  let me = identity();
  const clear = vi.fn();
  const transport = vi.fn((path: string) =>
    path.endsWith("/me") ? Promise.resolve(Response.json(me)) : delayed.promise,
  );
  const session = new AdminSession(transport as typeof fetch, clear);
  await session.refresh();
  const old = session.request("/admin/users");
  const assertion = expect(old).rejects.toMatchObject({
    code: "CLIENT_IDENTITY_STALE",
  });
  me = identity("second");
  await session.refresh();
  delayed.resolve(Response.json({ users: ["private"] }));
  await assertion;
  expect(clear).toHaveBeenCalledTimes(2);
  me = identity("second", false);
  await session.refresh();
  expect(clear).toHaveBeenCalledTimes(3);
});
it("ignores a stale identity lookup after a cross-tab invalidation", async () => {
  const delayed = deferred<Response>();
  const transport = vi
    .fn()
    .mockImplementationOnce(() => delayed.promise)
    .mockResolvedValue(Response.json(identity("new", false)));
  const session = new AdminSession(transport, vi.fn());
  const old = session.refresh();
  session.invalidate();
  await session.refresh();
  delayed.resolve(Response.json(identity()));
  await old;
  expect(session.snapshot().identity).toEqual(identity("new", false));
});
it("refreshes rejected CSRF without replaying the write and clears identity on 401", async () => {
  const transport = vi.fn(async (path: string) =>
    path.endsWith("/me")
      ? Response.json(identity())
      : Response.json({ error_code: "CSRF_REJECTED" }, { status: 403 }),
  );
  const session = new AdminSession(transport as typeof fetch, vi.fn());
  await session.refresh();
  await expect(
    session.request("/admin/users", "POST", {}),
  ).rejects.toMatchObject({ code: "CSRF_REJECTED" });
  expect(
    transport.mock.calls.filter(([path]) => path === "/api/admin/users"),
  ).toHaveLength(1);
  transport.mockImplementation(async () =>
    Response.json({ error_code: "AUTH_REQUIRED" }, { status: 401 }),
  );
  await expect(session.request("/admin/users")).rejects.toMatchObject({
    status: 401,
  });
  expect(session.snapshot().identity).toBeNull();
});

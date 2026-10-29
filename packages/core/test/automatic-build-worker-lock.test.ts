import { existsSync, mkdtempSync } from "node:fs";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { withAutomaticBuildWorkerSchedulerLock } from "../src/automatic-build-worker";

vi.mock("node:fs", async importOriginal => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return { ...actual, readFileSync: vi.fn(actual.readFileSync) };
});

beforeEach(() => {
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", mkdtempSync(path.join(tmpdir(), "worker-lock-contention-")));
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllEnvs(); });

describe("worker scheduler lock contention", () => {
  it.each(["EPERM", "EACCES"])("waits through a transient %s reading the current owner's lock", async code => {
    const invocation = `abinv1_${"d".repeat(64)}`;
    const lock = path.join(process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT!, "worker-scheduler", `${invocation}.json.lock`);
    const order: string[] = [];
    let release!: () => void;
    const owner = withAutomaticBuildWorkerSchedulerLock(invocation, async () => {
      order.push("owner");
      await new Promise<void>(resolve => { release = resolve; });
      order.push("owner released");
    });
    const unavailable = Object.assign(new Error("lock is being replaced on Windows"), { code, syscall: "open" });
    vi.mocked(fs.readFileSync).mockImplementationOnce(() => { throw unavailable; });
    const contender = withAutomaticBuildWorkerSchedulerLock(invocation, () => {
      order.push("contender");
      return "scheduled";
    }).then(value => value, error => error);
    try {
      await new Promise(resolve => setTimeout(resolve, 40));
      expect(order).toEqual(["owner"]);
      expect(existsSync(lock)).toBe(true);
    } finally {
      release();
      await owner;
    }
    expect(await contender).toBe("scheduled");
    expect(order).toEqual(["owner", "owner released", "contender"]);
    expect(existsSync(lock)).toBe(false);
  });

  it("preserves unrelated read failures and the current owner's lock", async () => {
    const invocation = `abinv1_${"e".repeat(64)}`;
    let release!: () => void;
    const owner = withAutomaticBuildWorkerSchedulerLock(invocation, () => new Promise<void>(resolve => { release = resolve; }));
    const failure = Object.assign(new Error("storage read failed"), { code: "EIO" });
    vi.mocked(fs.readFileSync).mockImplementationOnce(() => { throw failure; });
    const run = vi.fn();
    try {
      await expect(withAutomaticBuildWorkerSchedulerLock(invocation, run)).rejects.toBe(failure);
      expect(run).not.toHaveBeenCalled();
    } finally {
      release();
      await owner;
    }
  });
});

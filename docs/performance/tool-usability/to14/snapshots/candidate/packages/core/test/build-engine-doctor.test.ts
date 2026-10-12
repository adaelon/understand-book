import { cpSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, expect, it, vi } from "vitest";

const root = path.resolve(import.meta.dirname, "../../..");
afterEach(() => { vi.unstubAllEnvs(); vi.resetModules(); });

it("checks Engine assets without requiring Codex role or registration files", async () => {
  const fixture = mkdtempSync(path.join(tmpdir(), "ub-engine-doctor-"));
  cpSync(path.join(root, "agents"), path.join(fixture, "agents"), { recursive: true });
  const source = path.join(fixture, "source.md");
  writeFileSync(source, "# Synthetic material\n\nA synthetic definition.\n");
  vi.stubEnv("UNDERSTAND_BOOK_PLUGIN_ROOT", fixture);
  vi.resetModules();
  const { automaticBuildEngineContract, automaticBuildProtocolContract } = await import("../../../skills/build/automatic-build");
  const engine = automaticBuildEngineContract(source, fixture);
  expect(engine.status).toBe("compatible");
  expect(engine.version).toBe("automatic_build_engine_doctor.v1");
  const codex = automaticBuildProtocolContract(source, fixture);
  expect(codex.status).toBe("incompatible");
  expect(codex.checks.executor_role.status).toBe("incompatible");
  expect(codex.checks.prompt_provider).toEqual(engine.checks.prompt_provider);
  const extractor = engine.checks.prompt_provider.checked_extractors[0];
  rmSync(path.join(fixture, "agents", extractor));
  const missingPrompt = automaticBuildEngineContract(source, fixture);
  expect(missingPrompt.status).toBe("incompatible");
  expect(missingPrompt.checks.prompt_provider.status).toBe("incompatible");
  expect(automaticBuildProtocolContract(source, fixture).checks.prompt_provider).toEqual(missingPrompt.checks.prompt_provider);
});

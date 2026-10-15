/** Host observations associated with the exact plan shown to a human. */
export interface DshBuildConfirmationV1 {
  version: "dsh_build_confirmation.v1";
  plan_id: string;
  plan_revision: number;
  plan_digest: string;
  root_session_id: string;
  question_id: string;
  selected: "批准";
  answered_at: string;
}

export function readDshBuildConfirmation(value: unknown): DshBuildConfirmationV1 {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("build confirmation is required");
  const v = value as Record<string, unknown>;
  const keys = ["version", "plan_id", "plan_revision", "plan_digest", "root_session_id", "question_id", "selected", "answered_at"];
  if (Object.keys(v).length !== keys.length || keys.some(k => !Object.hasOwn(v, k))
    || v.version !== "dsh_build_confirmation.v1" || v.selected !== "批准"
    || !Number.isSafeInteger(v.plan_revision) || (v.plan_revision as number) < 1
    || typeof v.plan_digest !== "string" || !/^[a-f0-9]{64}$/u.test(v.plan_digest)
    || ["plan_id", "root_session_id", "question_id", "answered_at"].some(k => typeof v[k] !== "string" || !(v[k] as string).trim())
    || !Number.isFinite(Date.parse(v.answered_at as string))) throw new Error("build confirmation is invalid");
  return { ...v } as unknown as DshBuildConfirmationV1;
}

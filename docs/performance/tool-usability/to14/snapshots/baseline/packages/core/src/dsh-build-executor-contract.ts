export const DSH_BUILD_EXECUTOR_CONTRACT_V1 = Object.freeze({
  version: "dsh_build_executor_contract.v1",
  bootstrap_version: "understand_book_dsh_executor_bootstrap.v1",
  session_protocol: "automatic_build_executor_session.v4",
  profile_id: "dsh_native_v4",
  profile_revision: 1,
  input_manifest_version: "automatic_build_executor_input_manifest.v4",
  handoff_record_version: "automatic_build_opaque_handoff_record.v5",
});

/** Startup negotiation also covers the control plane used before executor creation. */
export const DSH_BUILD_CONTROL_CONTRACT_V1 = Object.freeze({
  version: "dsh_build_capabilities.v1",
  prepare: "dsh_build_prepare.v2",
  invocation_read: "dsh_build_invocation_read.v2",
  controller: "dsh_build_controller.v1",
});

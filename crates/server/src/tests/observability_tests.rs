use reader_service::observability::tests::{self as contracts, AdapterWrapper};
use runtime::ModelAdapter;
use std::sync::{Arc, Mutex};

struct BudgetedAdapter;

impl AdapterWrapper for BudgetedAdapter {
    fn with_adapter(
        &self,
        inner: &dyn ModelAdapter,
        exercise: &mut dyn FnMut(&dyn ModelAdapter),
    ) -> u64 {
        let limited = crate::service_limits::LimitedAdapter {
            inner,
            resources: crate::service_limits::Resources::new(Arc::new(Default::default())),
            owner: "observation-fixture".into(),
            cancellation: Default::default(),
            background: false,
            usage: Mutex::new(Default::default()),
            stream: None,
            authorization: None,
        };
        exercise(&limited);
        let total_tokens = limited.usage.lock().unwrap().known_total_tokens;
        total_tokens
    }
}

#[test]
fn provider_and_teaching_purpose_survive_runtime_wrappers_and_export() {
    contracts::provider_and_teaching_purpose_contract(&BudgetedAdapter);
}

#[test]
fn request_diagnostics_reach_export_with_usage_from_final_native_and_react_requests() {
    contracts::request_diagnostics_contract(&BudgetedAdapter);
}

#[test]
#[ignore = "manual Linux LangSmith connectivity check; use a dedicated canary project"]
fn live_request_diagnostics_canary() {
    contracts::live_request_diagnostics_contract(&BudgetedAdapter);
}

//! Real-service trials observe and bound actual sends without rewriting signed endpoints.
use crate::model_rates::{ModelRateSnapshot, ModelRates};
use runtime::model_spend::{ModelSpendPort, SendEvidence, SendIdentity, SendOutcome, SpendStop};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};

#[derive(Clone)]
struct Reservation {
    identity: Value,
    rate: ModelRateSnapshot,
    reserved_micro_cny: i64,
    outcome: Option<Value>,
    stop: Option<SpendStop>,
}

struct CostLimit {
    max_micro_cny: i64,
    rates: Result<ModelRates, SpendStop>,
}

struct State {
    file: File,
    sends: u64,
    tokens: u64,
    unknown_usage: bool,
    paid_micro_cny: i64,
    reservations: BTreeMap<String, Reservation>,
}

pub(super) struct TrialBudget {
    state: Mutex<State>,
    max_requests: u64,
    max_tokens: u64,
    cost: Option<CostLimit>,
}

impl TrialBudget {
    pub(super) fn open(output: &Path) -> Self {
        fn limit(name: &str, fallback: u64) -> u64 {
            std::env::var(name)
                .map(|value| {
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|n| *n > 0)
                        .unwrap_or_else(|| panic!("{name} must be a positive integer"))
                })
                .unwrap_or(fallback)
        }
        let cost = match (std::env::var("MODEL_TRIAL_MAX_COST_MICRO_CNY").ok(), std::env::var("MODEL_TRIAL_RATES_FILE").ok()) {
            (None, None) => None,
            (Some(amount), Some(filename)) => Some(CostLimit {
                max_micro_cny: amount.parse::<i64>().ok().filter(|n|*n>0).expect("MODEL_TRIAL_MAX_COST_MICRO_CNY must be a positive integer"),
                rates: ModelRates::load(Path::new(&filename)),
            }),
            _ => panic!("MODEL_TRIAL_MAX_COST_MICRO_CNY and MODEL_TRIAL_RATES_FILE must be supplied together"),
        };
        Self::with_cost_limit(
            output,
            limit("MODEL_TRIAL_MAX_REQUESTS", 24),
            limit("MODEL_TRIAL_MAX_TOKENS", 1_200_000),
            cost,
        )
    }

    fn with_limits(output: &Path, max_requests: u64, max_tokens: u64) -> Self {
        Self::with_cost_limit(output, max_requests, max_tokens, None)
    }

    fn with_cost_limit(
        output: &Path,
        max_requests: u64,
        max_tokens: u64,
        cost: Option<CostLimit>,
    ) -> Self {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join("provider-sends.jsonl"))
            .expect("Use a new evidence directory; earlier provider sends must remain unchanged");
        Self {
            state: Mutex::new(State {
                file,
                sends: 0,
                tokens: 0,
                unknown_usage: false,
                paid_micro_cny: 0,
                reservations: BTreeMap::new(),
            }),
            max_requests,
            max_tokens,
            cost,
        }
    }

    fn record(state: &mut State, record: Value) -> Result<(), SpendStop> {
        writeln!(state.file, "{record}")
            .and_then(|_| state.file.flush())
            .map_err(|_| SpendStop::StorageUnavailable)
    }

    fn reservation(
        &self,
        state: &State,
        identity: &SendIdentity,
        request: &Value,
    ) -> Result<Option<Reservation>, SpendStop> {
        let Some(cost) = &self.cost else {
            return Ok(None);
        };
        if state.reservations.contains_key(&identity.call_id) {
            return Err(SpendStop::ReconciliationRequired);
        }
        let rate = cost.rates.as_ref().map_err(|reason| *reason)?.select(
            &identity.provider,
            &identity.model,
            crate::multi_user_host::now(),
        )?;
        let reserve = rate
            .estimate_for_model(&identity.model, request)?
            .reserved_micro_cny
            .value();
        let held = state
            .reservations
            .values()
            .try_fold(0i64, |total, call| {
                total.checked_add(call.reserved_micro_cny)
            })
            .ok_or(SpendStop::InsufficientAllowance)?;
        if state
            .paid_micro_cny
            .checked_add(held)
            .and_then(|total| total.checked_add(reserve))
            .is_none_or(|total| total > cost.max_micro_cny)
        {
            return Err(SpendStop::InsufficientAllowance);
        }
        Ok(Some(Reservation {
            identity: json!(identity),
            rate,
            reserved_micro_cny: reserve,
            outcome: None,
            stop: None,
        }))
    }
}

fn logged_identity(identity: &SendIdentity) -> Value {
    let mut value = json!(identity);
    if let Ok(mut url) = url::Url::parse(&identity.provider) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        value["provider"] = json!(url.as_str());
    }
    value
}

fn frozen_rate(rate: &ModelRateSnapshot) -> Value {
    json!({"version":rate.version,"usage_contract":rate.usage_contract,
        "input_micro_cny_per_million":rate.input_micro_cny_per_million,
        "cached_input_micro_cny_per_million":rate.cached_input_micro_cny_per_million,
        "cache_creation_input_micro_cny_per_million":rate.cache_creation_input_micro_cny_per_million,
        "output_micro_cny_per_million":rate.output_micro_cny_per_million})
}

impl ModelSpendPort for TrialBudget {
    fn before_send(&self, identity: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        let mut state = self.state.lock().unwrap();
        let reservation = if state.unknown_usage {
            Err(SpendStop::ReconciliationRequired)
        } else if state.sends >= self.max_requests || state.tokens >= self.max_tokens {
            Err(SpendStop::InsufficientAllowance)
        } else {
            self.reservation(&state, identity, request)
        };
        let reservation = match reservation {
            Ok(reservation) => reservation,
            Err(reason) => {
                Self::record(
                    &mut state,
                    json!({"phase":"rejected","identity":logged_identity(identity),"reason":reason.code()}),
                )?;
                return Err(reason);
            }
        };
        let cost = reservation.as_ref().map(|call|json!({"currency":"CNY","reserved_micro_cny":call.reserved_micro_cny,
            "max_micro_cny":self.cost.as_ref().unwrap().max_micro_cny,"rate":frozen_rate(&call.rate)}));
        Self::record(
            &mut state,
            json!({"phase":"before_send","identity":logged_identity(identity),"cost":cost}),
        )?;
        if let Some(reservation) = reservation {
            state
                .reservations
                .insert(identity.call_id.clone(), reservation);
        }
        state.sends += 1;
        Ok(())
    }

    fn after_send(&self, identity: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        let mut state = self.state.lock().unwrap();
        let mut charge = None;
        let mut stop = None;
        let mut cost_record = Value::Null;
        if let Some(cost) = &self.cost {
            let call = state
                .reservations
                .get(&identity.call_id)
                .ok_or(SpendStop::ReconciliationRequired)?;
            if call.identity != json!(identity) {
                return Err(SpendStop::ReconciliationRequired);
            }
            if let Some(previous) = &call.outcome {
                return if previous == &json!(outcome) {
                    call.stop.map_or(Ok(()), Err)
                } else {
                    Err(SpendStop::ReconciliationRequired)
                };
            }
            let computed = if let Some(usage) = &outcome.usage {
                call.rate
                    .cost(usage)
                    .map(|amount| amount.value())
                    .map_err(|reason| json!(reason))
            } else if outcome.evidence == SendEvidence::NotSent {
                Ok(0)
            } else {
                Err(json!("missing_usage"))
            };
            match computed {
                Ok(amount) => {
                    charge = Some(amount);
                    let paid = state
                        .paid_micro_cny
                        .checked_add(amount)
                        .ok_or(SpendStop::ReconciliationRequired)?;
                    let other_reservations = state
                        .reservations
                        .iter()
                        .filter(|(id, _)| *id != &identity.call_id)
                        .try_fold(0i64, |total, (_, call)| {
                            total.checked_add(call.reserved_micro_cny)
                        })
                        .ok_or(SpendStop::ReconciliationRequired)?;
                    if paid
                        .checked_add(other_reservations)
                        .is_none_or(|total| total > cost.max_micro_cny)
                    {
                        stop = Some(SpendStop::InsufficientAllowance);
                    }
                    cost_record = json!({"currency":"CNY","actual_micro_cny":amount,"paid_micro_cny":paid,
                        "remaining_reserved_micro_cny":other_reservations,"rate_version":call.rate.version,"status":"confirmed"});
                }
                Err(reason) => {
                    stop = Some(SpendStop::ReconciliationRequired);
                    cost_record = json!({"currency":"CNY","actual_micro_cny":null,"retained_reserved_micro_cny":call.reserved_micro_cny,
                        "rate_version":call.rate.version,"status":"unknown","reason":reason});
                }
            }
        }
        Self::record(
            &mut state,
            json!({"phase":"after_send","identity":logged_identity(identity),"outcome":outcome,"cost":cost_record}),
        )?;
        if self.cost.is_some() {
            if let Some(amount) = charge {
                state.paid_micro_cny += amount;
            } else {
                state.unknown_usage = true;
            }
            let call = state.reservations.get_mut(&identity.call_id).unwrap();
            if charge.is_some() {
                call.reserved_micro_cny = 0;
            }
            call.outcome = Some(json!(outcome));
            call.stop = stop;
        }
        if outcome.evidence != runtime::model_spend::SendEvidence::NotSent {
            match outcome.usage.as_ref().and_then(|usage| {
                usage.total_tokens.or_else(|| {
                    usage
                        .input_tokens
                        .zip(usage.output_tokens)
                        .map(|(a, b)| a.saturating_add(b))
                })
            }) {
                Some(tokens) => state.tokens = state.tokens.saturating_add(u64::from(tokens)),
                None => state.unknown_usage = true,
            }
        }
        stop.map_or(Ok(()), Err)
    }
}

#[test]
fn model_trial_budget_records_attempts_and_rejects_before_an_extra_send() {
    let directory = tempfile::tempdir().unwrap();
    let budget = TrialBudget::with_limits(directory.path(), 1, 100);
    let identity = SendIdentity {
        call_id: "one".into(),
        logical_call_id: "logical".into(),
        attempt: 1,
        scope: None,
        purpose: "trial".into(),
        model: "fixture".into(),
        provider: "http://localhost".into(),
    };
    budget
        .before_send(&identity, &json!({"secret":"not recorded"}))
        .unwrap();
    budget
        .after_send(
            &identity,
            &SendOutcome {
                evidence: runtime::model_spend::SendEvidence::Response,
                usage: Some(runtime::provider_stream::ModelUsage {
                    total_tokens: Some(20),
                    ..Default::default()
                }),
                succeeded: true,
            },
        )
        .unwrap();
    assert_eq!(
        budget.before_send(&identity, &json!({})),
        Err(SpendStop::InsufficientAllowance)
    );
    let log = std::fs::read_to_string(directory.path().join("provider-sends.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 3);
    assert!(!log.contains("not recorded"));
}

#[test]
fn model_trial_budget_stops_when_a_sent_request_has_unknown_usage() {
    let directory = tempfile::tempdir().unwrap();
    let budget = TrialBudget::with_limits(directory.path(), 3, 100);
    let identity = SendIdentity {
        call_id: "one".into(),
        logical_call_id: "logical".into(),
        attempt: 1,
        scope: None,
        purpose: "trial".into(),
        model: "fixture".into(),
        provider: "http://localhost".into(),
    };
    budget.before_send(&identity, &json!({})).unwrap();
    budget
        .after_send(
            &identity,
            &SendOutcome {
                evidence: runtime::model_spend::SendEvidence::OutcomeUnknown,
                usage: None,
                succeeded: false,
            },
        )
        .unwrap();
    assert_eq!(
        budget.before_send(&identity, &json!({})),
        Err(SpendStop::ReconciliationRequired)
    );
}

fn test_rates() -> ModelRates {
    let rates: ModelRates = serde_json::from_value(json!({"rates":[{
        "version":"trial-price-v1","provider":"https://fixture.example?private=hidden-value","model":"fixture",
        "starts_at":0,"expires_at":i64::MAX,"input_micro_cny_per_million":1000000,
        "cached_input_micro_cny_per_million":1000000,"output_micro_cny_per_million":1000000,
        "usage_contract":"input_output","image_meter":"unsupported",
        "source":{"currency":"CNY","original_prices":"fixture","cny_conversion":"1",
            "checked_on":"2026-11-18","official_url":"https://fixture.example/prices"}
    }]})).unwrap();
    rates.validate().unwrap();
    rates
}

fn test_identity(call: &str) -> SendIdentity {
    SendIdentity {
        call_id: call.into(),
        logical_call_id: call.into(),
        attempt: 1,
        scope: None,
        purpose: "trial".into(),
        model: "fixture".into(),
        provider: "https://fixture.example?private=hidden-value".into(),
    }
}

fn test_request() -> Value {
    json!({"model":"fixture","max_tokens":10,"messages":[{"role":"user","content":"never-log-input"}]})
}

#[test]
fn model_trial_cost_budget_blocks_before_send_and_keeps_unreported_reservations() {
    let directory = tempfile::tempdir().unwrap();
    let rates = test_rates();
    let reserve = rates.rates[0]
        .estimate_for_model("fixture", &test_request())
        .unwrap()
        .reserved_micro_cny
        .value();
    let budget = TrialBudget::with_cost_limit(
        directory.path(),
        10,
        1000,
        Some(CostLimit {
            max_micro_cny: reserve * 2 - 1,
            rates: Ok(rates),
        }),
    );
    let first = test_identity("first");
    budget.before_send(&first, &test_request()).unwrap();
    let second = test_identity("second");
    assert_eq!(
        budget.before_send(&second, &test_request()),
        Err(SpendStop::InsufficientAllowance)
    );
    assert_eq!(budget.state.lock().unwrap().sends, 1);
    let receipt = SendOutcome {
        evidence: SendEvidence::Response,
        succeeded: true,
        usage: Some(runtime::provider_stream::ModelUsage {
            input_tokens: Some(3),
            output_tokens: Some(2),
            total_tokens: Some(5),
            ..Default::default()
        }),
    };
    budget.after_send(&first, &receipt).unwrap();
    budget.after_send(&first, &receipt).unwrap();
    assert_eq!(budget.state.lock().unwrap().paid_micro_cny, 5);
    budget.before_send(&second, &test_request()).unwrap();
    assert_eq!(
        budget.before_send(&test_identity("third"), &test_request()),
        Err(SpendStop::InsufficientAllowance)
    );
    let state = budget.state.lock().unwrap();
    assert_eq!(state.sends, 2);
    assert_eq!(state.reservations["second"].reserved_micro_cny, reserve);
    drop(state);
    let records = std::fs::read_to_string(directory.path().join("provider-sends.jsonl")).unwrap();
    assert!(!records.contains("hidden-value"));
    assert!(!records.contains("never-log-input"));
    assert_eq!(
        records
            .lines()
            .filter(|line| line.contains("\"phase\":\"after_send\""))
            .count(),
        1
    );
}

#[test]
fn model_trial_cost_budget_invalid_rates_never_authorize_a_send() {
    let directory = tempfile::tempdir().unwrap();
    let rates_file = directory.path().join("rates.json");
    std::fs::write(&rates_file, b"{invalid}").unwrap();
    let budget = TrialBudget::with_cost_limit(
        directory.path(),
        10,
        1000,
        Some(CostLimit {
            max_micro_cny: 2_000_000,
            rates: ModelRates::load(&rates_file),
        }),
    );
    assert_eq!(
        budget.before_send(&test_identity("bad-rate"), &test_request()),
        Err(SpendStop::RateUnavailable)
    );
    assert_eq!(budget.state.lock().unwrap().sends, 0);
    let records = std::fs::read_to_string(directory.path().join("provider-sends.jsonl")).unwrap();
    assert_eq!(records.lines().count(), 1);
    assert!(records.contains("\"phase\":\"rejected\""));
}

#[test]
fn model_trial_cost_budget_unknown_charge_retains_reservation_and_stops_immediately() {
    let directory = tempfile::tempdir().unwrap();
    let budget = TrialBudget::with_cost_limit(
        directory.path(),
        10,
        1000,
        Some(CostLimit {
            max_micro_cny: 2_000_000,
            rates: Ok(test_rates()),
        }),
    );
    let identity = test_identity("missing-output");
    budget.before_send(&identity, &test_request()).unwrap();
    let reserve = budget.state.lock().unwrap().reservations["missing-output"].reserved_micro_cny;
    let unknown = SendOutcome {
        evidence: SendEvidence::Response,
        succeeded: true,
        usage: Some(runtime::provider_stream::ModelUsage {
            input_tokens: Some(10),
            ..Default::default()
        }),
    };
    assert_eq!(
        budget.after_send(&identity, &unknown),
        Err(SpendStop::ReconciliationRequired)
    );
    assert_eq!(
        budget.before_send(&test_identity("next"), &test_request()),
        Err(SpendStop::ReconciliationRequired)
    );
    let state = budget.state.lock().unwrap();
    assert_eq!(
        state.reservations["missing-output"].reserved_micro_cny,
        reserve
    );
    assert_eq!(state.paid_micro_cny, 0);
    assert!(state.unknown_usage);
}

#[test]
fn model_trial_cost_budget_releases_only_provably_unsent_calls() {
    let directory = tempfile::tempdir().unwrap();
    let rates = test_rates();
    let reserve = rates.rates[0]
        .estimate_for_model("fixture", &test_request())
        .unwrap()
        .reserved_micro_cny
        .value();
    let budget = TrialBudget::with_cost_limit(
        directory.path(),
        10,
        1000,
        Some(CostLimit {
            max_micro_cny: reserve,
            rates: Ok(rates),
        }),
    );
    let first = test_identity("unsent");
    budget.before_send(&first, &test_request()).unwrap();
    budget
        .after_send(
            &first,
            &SendOutcome {
                evidence: SendEvidence::NotSent,
                succeeded: false,
                usage: None,
            },
        )
        .unwrap();
    budget
        .before_send(&test_identity("next"), &test_request())
        .unwrap();
    assert_eq!(budget.state.lock().unwrap().paid_micro_cny, 0);
}

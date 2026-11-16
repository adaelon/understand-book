use super::*;
use crate::{
    account_allowance::{AllowanceBasis, MicroCny, ReceiptFen},
    admin_store::{AdminStore, Command},
    control_store::{ControlStore, ServiceWriter},
};
use time::{Date, Month, UtcOffset};

// These values are test fixtures, not a saleable price or quota configuration.
const TEST_PRICE_FEN: i64 = 123;
const TEST_BASIS_MICRO_CNY: i64 = 1_000_000;
const TEST_STORAGE_BYTES: i64 = 20_000;
const TEST_FILE_BYTES: i64 = 10_000;
const TEST_RETENTION_SECONDS: i64 = 30 * 86_400;

struct Fixture {
    store: AdminStore,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer).unwrap();
        for user in ["operator", "A"] {
            control.create_user(user).unwrap();
        }
        Self {
            store: AdminStore::new(control),
            _root: root,
        }
    }

    fn apply(&self, id: &str, mutation: Mutation, now: i64) -> Value {
        self.store
            .apply("operator", id, "A", Command::CloudPlan(mutation), now)
            .unwrap()
            .0
    }

    fn save(&self, id: &str, product: Product, now: i64) -> Value {
        self.apply(id, Mutation::Product(product), now)
    }

    fn purchase(&self, id: &str, product: &str, revision: i64, now: i64) -> Value {
        self.apply(
            id,
            Mutation::Purchase(purchase(product, revision, now)),
            now,
        )
    }

    fn count(&self, table: &str) -> i64 {
        self.store
            .control
            .lock()
            .unwrap()
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}

fn product(id: &str, tier: &str, duration: &str) -> Product {
    Product {
        product_id: id.into(),
        title: format!("{id} (test fixture)"),
        tier: tier.into(),
        duration: duration.into(),
        revision: 0,
        enabled: true,
        test_only: false,
        amount_fen: Some(ReceiptFen::try_from(TEST_PRICE_FEN).unwrap()),
        budget_micro_cny: None,
        basis: Some(AllowanceBasis {
            version: "cloud-plan-test-basis".into(),
            basis_micro_cny: MicroCny::new(TEST_BASIS_MICRO_CNY),
        }),
        grant_milli_percent: Some(100_000),
        storage_bytes: Some(TEST_STORAGE_BYTES),
        max_file_bytes: Some(TEST_FILE_BYTES),
        retention_seconds: Some(TEST_RETENTION_SECONDS),
        instructions: "Fixture only; no payment is collected.".into(),
    }
}

fn purchase(product: &str, revision: i64, now: i64) -> Purchase {
    Purchase {
        product_id: product.into(),
        product_revision: revision,
        paid_at: now,
        channel: "test-fixture".into(),
        external_ref: None,
        note: "Fixture payment, no real charge".into(),
    }
}

#[test]
fn budget_admin_products_use_exact_money_and_preserve_sold_terms() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    let mut first = product("budget-basic", "basic", "day1");
    first.basis = None;
    first.grant_milli_percent = None;
    first.budget_micro_cny = Some(MicroCny::new(3_333_333));
    let saved = f.save("budget-save", first.clone(), now);
    assert_eq!(saved["cloud_plan"]["budget_micro_cny"], 3_333_333);
    assert_eq!(saved["cloud_plan"]["saleable"], true);
    let paid = f.purchase("budget-paid", "budget-basic", 1, now);
    assert_eq!(paid["allowance_period"]["balance"]["granted_micro_cny"], 3_333_333);
    assert_eq!(paid["allowance_period"]["is_plan"], true);
    assert_eq!(f.purchase("budget-paid", "budget-basic", 1, now), paid);
    first.revision = 1;
    first.budget_micro_cny = Some(MicroCny::new(10_000_000));
    f.save("budget-edit", first, now + 1);
    let mut plus = product("budget-plus", "plus", "day1");
    plus.budget_micro_cny = Some(MicroCny::new(20_000_000));
    f.save("budget-plus", plus, now + 1);
    let control = f.store.control.lock().unwrap();
    let sold = current(&control.connection, "A", now + 1).unwrap().unwrap();
    assert_eq!(sold.basis.grant(sold.grant_milli_percent).unwrap().value(), 3_333_333);
    assert_eq!(sold.amount_fen.value(), TEST_PRICE_FEN);
    let summary = summary(&control.connection, "A", now + 1).unwrap();
    assert_eq!(summary["purchase_starts_at"], sold.expires_at);
    assert_eq!(summary["purchase_blocked_by_unlimited"], false);
}

#[test]
fn budget_admin_paid_plan_cannot_be_queued_after_unlimited_beta() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("catalog", product("basic", "basic", "month"), now);
    {
        let control = f.store.control.lock().unwrap();
        control.connection.execute("INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at,unlimited) VALUES('beta','A',?,253402300799,1)", [now]).unwrap();
        assert_eq!(summary(&control.connection, "A", now).unwrap()["purchase_blocked_by_unlimited"], true);
    }
    let error = f.store.apply("operator", "paid", "A", Command::CloudPlan(Mutation::Purchase(purchase("basic", 1, now))), now).unwrap_err();
    assert_eq!(error.error_code, "CLOUD_PLAN_UNLIMITED");
    assert_eq!(f.count("manual_receipts"), 0);
    assert_eq!(f.count("cloud_plan_entitlements"), 0);
}

fn at(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> i64 {
    Date::from_calendar_date(year, month, day)
        .unwrap()
        .with_hms(hour, minute, 0)
        .unwrap()
        .assume_offset(UtcOffset::from_hms(8, 0, 0).unwrap())
        .unix_timestamp()
}

#[test]
fn cloud_plans_catalog_hides_drafts_and_test_products_from_new_accounts() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    let mut draft = product("draft", "basic", "month");
    draft.amount_fen = None;
    draft.grant_milli_percent = None;
    draft.storage_bytes = None;
    f.save("draft", draft, now);
    let mut disabled = product("disabled", "basic", "day1");
    disabled.enabled = false;
    f.save("disabled", disabled, now);
    let mut test = product("test-product", "basic", "day1");
    test.test_only = true;
    f.save("test-product", test, now);
    f.save("public", product("public", "basic", "day1"), now);

    {
        let control = f.store.control.lock().unwrap();
        let catalog = catalog(&control.connection, true).unwrap();
        assert_eq!(catalog["items"].as_array().unwrap().len(), 1);
        assert_eq!(catalog["items"][0]["product_id"], "public");
        assert!(current(&control.connection, "A", now).unwrap().is_none());
        let account = summary(&control.connection, "A", now).unwrap();
        assert!(account["current"].is_null());
        assert_eq!(account["catalog"]["items"], catalog["items"]);
    }

    for id in ["draft", "disabled", "test-product"] {
        assert!(f
            .store
            .apply(
                "operator",
                &format!("buy-{id}"),
                "A",
                Command::CloudPlan(Mutation::Purchase(purchase(id, 1, now))),
                now,
            )
            .is_err());
    }
    assert_eq!(f.count("manual_receipts"), 0);
    assert_eq!(f.count("allowance_periods"), 0);
}

#[test]
fn cloud_plans_purchase_keeps_sold_entitlement_after_product_changes() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("create", product("sprint", "basic", "day1"), now);
    f.purchase("paid", "sprint", 1, now);
    let (period, receipt, sold_title) = {
        let control = f.store.control.lock().unwrap();
        let sold = current(&control.connection, "A", now).unwrap().unwrap();
        (sold.period_id, sold.receipt_id, sold.title)
    };
    let mut changed = product("sprint", "pro", "month");
    changed.revision = 1;
    changed.title = "Revised fixture".into();
    changed.amount_fen = Some(ReceiptFen::try_from(456).unwrap());
    changed.grant_milli_percent = Some(300_000);
    changed.storage_bytes = Some(TEST_STORAGE_BYTES * 3);
    changed.max_file_bytes = Some(TEST_FILE_BYTES * 2);
    changed.retention_seconds = Some(TEST_RETENTION_SECONDS * 2);
    f.save("edit", changed, now + 10);

    let control = f.store.control.lock().unwrap();
    let sold = current(&control.connection, "A", now + 10)
        .unwrap()
        .unwrap();
    assert_eq!(sold.period_id, period);
    assert_eq!(sold.receipt_id, receipt);
    assert_eq!(sold.title, sold_title);
    assert_eq!(sold.tier, "basic");
    assert_eq!(sold.duration, "day1");
    assert_eq!(sold.expires_at - sold.starts_at, 86_400);
    assert_eq!(sold.storage_bytes, TEST_STORAGE_BYTES);
    assert_eq!(sold.max_file_bytes, TEST_FILE_BYTES);
    assert_eq!(sold.retention_seconds, TEST_RETENTION_SECONDS);
    assert_eq!(sold.amount_fen.value(), TEST_PRICE_FEN);
    assert_eq!(sold.grant_milli_percent, 100_000);
    assert_eq!(sold.basis.basis_micro_cny.value(), TEST_BASIS_MICRO_CNY);
    let granted: i64 = control
        .connection
        .query_row(
            "SELECT sum(delta_micro_cny) FROM allowance_adjustments WHERE period_id=?",
            [&period],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(granted, TEST_BASIS_MICRO_CNY);
    let paid: i64 = control
        .connection
        .query_row(
            "SELECT amount_fen FROM manual_receipts WHERE receipt_id=?",
            [&receipt],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(paid, TEST_PRICE_FEN);
}

#[test]
fn cloud_plans_late_evening_day_purchase_runs_for_full_twenty_four_hours() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 23, 57);
    f.save("create", product("day", "basic", "day1"), now);
    let mut payment = purchase("day", 1, now);
    payment.paid_at = now - 3_600;
    f.apply("paid", Mutation::Purchase(payment), now);
    let control = f.store.control.lock().unwrap();
    let sold = current(&control.connection, "A", now).unwrap().unwrap();
    assert_eq!(sold.starts_at, now);
    assert_eq!(sold.expires_at, at(2027, Month::January, 11, 23, 57));
    assert!(current(&control.connection, "A", sold.expires_at - 1)
        .unwrap()
        .is_some());
    assert!(current(&control.connection, "A", sold.expires_at)
        .unwrap()
        .is_none());
}

#[test]
fn cloud_plans_duplicate_payment_reference_rolls_back_the_new_term() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("create", product("day", "basic", "day1"), now);
    let mut first = purchase("day", 1, now);
    first.external_ref = Some("single-payment".into());
    f.apply("first-confirmation", Mutation::Purchase(first.clone()), now);
    let duplicate = f.store.apply(
        "operator", "second-confirmation", "A",
        Command::CloudPlan(Mutation::Purchase(first)), now + 5,
    ).unwrap_err();
    assert_eq!(duplicate.error_code, "RECEIPT_EXTERNAL_REF_CONFLICT");
    for table in ["manual_receipts", "allowance_periods", "allowance_adjustments", "cloud_plan_entitlements"] {
        assert_eq!(f.count(table), 1, "failed duplicate payment must roll back {table}");
    }
}

#[test]
fn cloud_plans_month_end_renewals_keep_the_original_day_across_february() {
    for (year, february_last_day) in [(2027, 28), (2028, 29)] {
        let f = Fixture::new();
        let now = at(year, Month::January, 31, 18, 20);
        f.save("create", product("monthly", "basic", "month"), now);
        f.purchase("month-one", "monthly", 1, now);
        f.purchase("month-two", "monthly", 1, now + 1);
        f.purchase("month-three", "monthly", 1, now + 2);
        let control = f.store.control.lock().unwrap();
        let first = current(&control.connection, "A", now).unwrap().unwrap();
        assert_eq!(first.starts_at, now);
        assert_eq!(
            first.expires_at,
            at(year, Month::February, february_last_day, 18, 20)
        );
        let second = current(&control.connection, "A", first.expires_at)
            .unwrap()
            .unwrap();
        assert_eq!(second.starts_at, first.expires_at);
        assert_eq!(second.expires_at, at(year, Month::March, 31, 18, 20));
        let third = current(&control.connection, "A", second.expires_at)
            .unwrap()
            .unwrap();
        assert_eq!(third.starts_at, second.expires_at);
        assert_eq!(third.expires_at, at(year, Month::April, 30, 18, 20));
        assert_eq!(first.month_anchor_day, second.month_anchor_day);
        assert_eq!(second.month_anchor_day, third.month_anchor_day);
    }
}

#[test]
fn cloud_plans_next_tier_starts_at_expiry_without_carrying_unused_allowance() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("basic", product("basic-day", "basic", "day1"), now);
    let mut next_product = product("pro-month", "pro", "month");
    next_product.grant_milli_percent = Some(300_000);
    next_product.storage_bytes = Some(TEST_STORAGE_BYTES * 3);
    f.save("pro", next_product, now);
    f.purchase("first", "basic-day", 1, now);
    let (first_period, old_balance) = {
        let control = f.store.control.lock().unwrap();
        let first = current(&control.connection, "A", now).unwrap().unwrap();
        control.connection.execute(
            "INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at)
             VALUES('old-period-call','old-period-call',1,'A',?,'run','answer','fixture','{}','{}','settled',0,250000,'confirmed',250000,'fixture',?,?)",
            params![first.period_id, now, now],
        ).unwrap();
        let balance = crate::allowance_admin::period(&control.connection, "A", &first.period_id)
            .unwrap()["balance"]
            .clone();
        (first.period_id, balance)
    };
    f.purchase("next", "pro-month", 1, now + 1);

    let control = f.store.control.lock().unwrap();
    let first = current(&control.connection, "A", now).unwrap().unwrap();
    assert_eq!(first.tier, "basic");
    let second = current(&control.connection, "A", first.expires_at)
        .unwrap()
        .unwrap();
    assert_eq!(second.starts_at, first.expires_at);
    assert_eq!(second.tier, "pro");
    assert_ne!(first.period_id, second.period_id);
    let first_balance =
        crate::account_allowance::balance(&control.connection, &first.period_id).unwrap();
    let next_balance =
        crate::account_allowance::balance(&control.connection, &second.period_id).unwrap();
    assert_eq!(
        first_balance.available_micro_cny.value(),
        TEST_BASIS_MICRO_CNY - 250_000
    );
    assert_eq!(
        next_balance.available_micro_cny.value(),
        TEST_BASIS_MICRO_CNY * 3,
        "unused allowance remains on the old period"
    );
    let account = summary(&control.connection, "A", now).unwrap();
    assert_eq!(account["current"]["period_id"], first.period_id);
    assert_eq!(account["next"]["period_id"], second.period_id);
    assert_eq!(
        crate::allowance_admin::period(&control.connection, "A", &first_period).unwrap()["balance"],
        old_balance,
        "renewing does not rewrite prior debits or their historical percentages"
    );
}

#[test]
fn cloud_plans_lost_receipt_response_replays_once_after_catalog_change_and_reopen() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("create", product("day", "basic", "day1"), now);
    let result = f.purchase("paid", "day", 1, now);
    let mut changed = product("day", "basic", "day1");
    changed.revision = 1;
    changed.amount_fen = Some(ReceiptFen::try_from(456).unwrap());
    f.save("edit", changed, now + 1);
    assert_eq!(f.purchase("paid", "day", 1, now), result);

    let writer = f.store.control.lock().unwrap().writer.clone();
    let reopened = AdminStore::new(ControlStore::open(writer).unwrap());
    let replayed = reopened
        .apply(
            "operator",
            "paid",
            "A",
            Command::CloudPlan(Mutation::Purchase(purchase("day", 1, now))),
            now + 2,
        )
        .unwrap()
        .0;
    assert_eq!(replayed, result);
    assert_eq!(f.count("manual_receipts"), 1);
    assert_eq!(f.count("allowance_periods"), 1);
    assert_eq!(f.count("allowance_adjustments"), 1);
    assert_eq!(reopened.operation("operator", "paid").unwrap(), result);
}

#[test]
fn cloud_plans_sold_term_cannot_be_extended_by_legacy_validity_editor() {
    let f = Fixture::new();
    let now = at(2027, Month::January, 10, 12, 0);
    f.save("create", product("day", "basic", "day1"), now);
    f.purchase("paid", "day", 1, now);
    let sold = {
        let control = f.store.control.lock().unwrap();
        current(&control.connection, "A", now).unwrap().unwrap()
    };
    let attempted = f.store.apply(
        "operator",
        "stretch-term",
        "A",
        Command::Allowance(crate::allowance_admin::Mutation::Validity(
            crate::allowance_admin::Validity {
                period_id: sold.period_id.clone(),
                revision: 1,
                starts_at: sold.starts_at,
                expires_at: sold.expires_at + 86_400,
            },
        )),
        now + 1,
    );
    assert!(attempted.is_err());
    let control = f.store.control.lock().unwrap();
    let actual = crate::allowance_admin::period(&control.connection, "A", &sold.period_id).unwrap();
    assert_eq!(actual["starts_at"], sold.starts_at);
    assert_eq!(actual["expires_at"], sold.expires_at);
    assert_eq!(
        actual["revision"], 1,
        "a failed edit rolls its revision back"
    );
    assert!(current(&control.connection, "A", sold.expires_at)
        .unwrap()
        .is_none());
}

//! Cloud products and sold rights. AdminStore owns the transaction and replay
//! receipt, so payment, term, allowance grant and rights commit together.
use crate::{
    account_allowance::{AllowanceBasis, MicroCny, ReceiptFen},
    admin_store::invalid,
    allowance_admin::{self, storage},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::{Date, Month, OffsetDateTime, UtcOffset};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Product {
    pub product_id: String,
    pub title: String,
    pub tier: String,
    pub duration: String,
    /// Expected version when saving; the returned product contains the new one.
    pub revision: i64,
    pub enabled: bool,
    #[serde(default)]
    pub test_only: bool,
    pub amount_fen: Option<ReceiptFen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_micro_cny: Option<MicroCny>,
    pub basis: Option<AllowanceBasis>,
    pub grant_milli_percent: Option<i64>,
    pub storage_bytes: Option<i64>,
    pub max_file_bytes: Option<i64>,
    pub retention_seconds: Option<i64>,
    pub instructions: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Purchase {
    pub product_id: String,
    pub product_revision: i64,
    pub paid_at: i64,
    pub channel: String,
    pub external_ref: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entitlement {
    pub period_id: String,
    pub product_id: String,
    pub product_revision: i64,
    pub title: String,
    pub tier: String,
    pub duration: String,
    pub starts_at: i64,
    pub expires_at: i64,
    pub storage_bytes: i64,
    pub max_file_bytes: i64,
    pub retention_seconds: i64,
    pub receipt_id: String,
    pub month_anchor_day: Option<u8>,
    pub basis: AllowanceBasis,
    pub grant_milli_percent: i64,
    pub amount_fen: ReceiptFen,
}

pub(crate) enum Mutation {
    Product(Product),
    Purchase(Purchase),
}

fn conflict(code: &str, message: &str) -> ToolError {
    error(code, "conflict", message)
}

fn trim_required(value: &mut String) -> Result<(), ToolError> {
    *value = value.trim().to_owned();
    if value.is_empty() {
        return Err(invalid());
    }
    Ok(())
}

impl Product {
    fn normalize(&mut self) -> Result<(), ToolError> {
        if let Some(amount) = self.budget_micro_cny {
            self.basis = Some(AllowanceBasis::for_budget(amount)?);
            self.grant_milli_percent = Some(100_000);
        }
        for value in [
            &mut self.product_id,
            &mut self.title,
            &mut self.tier,
            &mut self.duration,
        ] {
            trim_required(value)?;
        }
        self.instructions = self.instructions.trim().to_owned();
        if !matches!(self.tier.as_str(), "basic" | "plus" | "pro")
            || !matches!(self.duration.as_str(), "day1" | "day3" | "day7" | "month")
            || self.revision < 0
        {
            return Err(invalid());
        }
        for value in [
            self.grant_milli_percent,
            self.storage_bytes,
            self.max_file_bytes,
            self.retention_seconds,
        ]
        .into_iter()
        .flatten()
        {
            if value <= 0 {
                return Err(invalid());
            }
        }
        if let (Some(max), Some(total)) = (self.max_file_bytes, self.storage_bytes) {
            if max > total {
                return Err(invalid());
            }
        }
        if let Some(basis) = &mut self.basis {
            trim_required(&mut basis.version)?;
            basis.validate()?;
            if let Some(grant) = self.grant_milli_percent {
                basis.grant(grant)?;
            }
        }
        Ok(())
    }

    fn unavailable_reasons(&self) -> Vec<&'static str> {
        let mut reasons = Vec::new();
        if !self.enabled {
            reasons.push("disabled");
        }
        if self.test_only {
            reasons.push("test_only");
        }
        if self.amount_fen.is_none() {
            reasons.push("price_required");
        }
        if self.basis.is_none() {
            reasons.push("basis_required");
        }
        if self.grant_milli_percent.is_none() {
            reasons.push("allowance_required");
        }
        if self.storage_bytes.is_none() {
            reasons.push("storage_required");
        }
        if self.max_file_bytes.is_none() {
            reasons.push("file_limit_required");
        }
        if self.retention_seconds.is_none() {
            reasons.push("retention_required");
        }
        if self.instructions.is_empty() {
            reasons.push("payment_instructions_required");
        }
        reasons
    }

    fn saleable(&self) -> bool {
        self.unavailable_reasons().is_empty()
    }

    fn public_value(&self) -> Value {
        let mut value = json!(self);
        value["budget_micro_cny"] = json!(self.basis.as_ref().zip(self.grant_milli_percent).and_then(|(basis, grant)| basis.grant(grant).ok()));
        value["saleable"] = json!(self.saleable());
        value["unavailable_reasons"] = json!(self.unavailable_reasons());
        value
    }

    fn save(&self, db: &Connection, now: i64) -> Result<Value, ToolError> {
        let current = product(db, &self.product_id)?;
        if current.as_ref().map_or(0, |p| p.revision) != self.revision {
            return Err(conflict(
                "CLOUD_PLAN_REVISION_CONFLICT",
                "套餐配置已更改，请刷新后重试",
            ));
        }
        if let Some(basis) = &self.basis {
            let existing: Option<MicroCny> = db
                .query_row(
                    "SELECT basis_micro_cny FROM allowance_bases WHERE version=?",
                    [&basis.version],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if existing.is_some_and(|amount| amount != basis.basis_micro_cny) {
                return Err(conflict(
                    "ALLOWANCE_BASIS_VERSION_CONFLICT",
                    "同一额度基准版本不能改变标准值",
                ));
            }
            db.execute(
                "INSERT OR IGNORE INTO allowance_bases(version,basis_micro_cny) VALUES(?,?)",
                params![basis.version, basis.basis_micro_cny],
            )
            .map_err(storage)?;
        }
        let mut saved = self.clone();
        saved.revision = self.revision.checked_add(1).ok_or_else(invalid)?;
        db.execute(
            "INSERT INTO cloud_plan_products(product_id,revision,configuration_json,updated_at) VALUES(?,?,?,?)
             ON CONFLICT(product_id) DO UPDATE SET revision=excluded.revision,configuration_json=excluded.configuration_json,updated_at=excluded.updated_at",
            params![saved.product_id, saved.revision, json!(saved).to_string(), now],
        ).map_err(storage)?;
        Ok(json!({"cloud_plan": saved.public_value()}))
    }
}

impl Mutation {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        match self {
            Self::Product(product) => product.normalize(),
            Self::Purchase(purchase) => {
                trim_required(&mut purchase.product_id)?;
                trim_required(&mut purchase.channel)?;
                purchase.note = purchase.note.trim().to_owned();
                if let Some(reference) = &mut purchase.external_ref {
                    trim_required(reference)?;
                }
                if purchase.product_revision < 1 {
                    return Err(invalid());
                }
                Ok(())
            }
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Product(_) => "cloud-plans",
            Self::Purchase(_) => "cloud-plan-purchases",
        }
    }

    pub(crate) fn parameters(&self) -> Value {
        match self {
            Self::Product(value) => json!(value),
            Self::Purchase(value) => json!(value),
        }
    }

    pub(crate) fn apply(
        &self,
        db: &Connection,
        actor: &str,
        op: &str,
        owner: &str,
        now: i64,
    ) -> Result<Value, ToolError> {
        match self {
            Self::Product(product) => product.save(db, now),
            Self::Purchase(purchase) => purchase.apply(db, actor, op, owner, now),
        }
    }
}

impl Purchase {
    fn apply(
        &self,
        db: &Connection,
        actor: &str,
        op: &str,
        owner: &str,
        now: i64,
    ) -> Result<Value, ToolError> {
        let product = product(db, &self.product_id)?.ok_or_else(crate::authorization::missing)?;
        if product.revision != self.product_revision {
            return Err(conflict(
                "CLOUD_PLAN_REVISION_CONFLICT",
                "套餐配置已更改，请确认新的价格和权益",
            ));
        }
        if !product.saleable() {
            return Err(conflict(
                "CLOUD_PLAN_UNAVAILABLE",
                "套餐尚未配置完整或未开放购买",
            ));
        }
        let enabled: bool = db
            .query_row(
                "SELECT disabled=0 FROM users WHERE user_id=?",
                [owner],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !enabled {
            return Err(conflict("USER_UNAVAILABLE", "账号不可用"));
        }
        // Include legacy/manual periods and already booked renewals. Scheduling
        // after the last open term preserves the single-period allowance model.
        let preceding: Option<(String, i64)> = db.query_row(
            "SELECT period_id,expires_at FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND expires_at>=? ORDER BY expires_at DESC LIMIT 1",
            params![owner, now], |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional().map_err(storage)?;
        let starts_at = preceding.as_ref().map_or(now, |(_, end)| *end);
        let unlimited: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND unlimited=1 AND expires_at>?)", params![owner, now], |r| r.get(0)).map_err(storage)?;
        if unlimited {
            return Err(conflict("CLOUD_PLAN_UNLIMITED", "账号仍有内测不限额度，请先结束内测有效期，再开通收费套餐。"));
        }
        let inherited_anchor = if product.duration == "month" {
            preceding.as_ref().map(|(id, _)| {
                db.query_row("SELECT month_anchor_day FROM cloud_plan_entitlements WHERE period_id=? AND duration='month'", [id], |r| r.get::<_, u8>(0))
                    .optional().map_err(storage)
            }).transpose()?.flatten()
        } else {
            None
        };
        let (expires_at, anchor) = term_end(&product.duration, starts_at, inherited_anchor)?;
        let created = allowance_admin::Mutation::Period(allowance_admin::Period {
            starts_at,
            expires_at,
            basis: product.basis.clone(),
            initial_grant_micro_cny: None,
            reason: None,
        })
        .apply(db, actor, op, owner, now)?;
        let period_id = created["allowance_period"]["period_id"]
            .as_str()
            .ok_or_else(invalid)?;
        let basis = product.basis.as_ref().unwrap();
        let granted = allowance_admin::Mutation::Receipt(allowance_admin::Receipt {
            period_id: period_id.to_owned(),
            revision: 0,
            amount_fen: product.amount_fen.unwrap(),
            delta_micro_cny: basis.grant(product.grant_milli_percent.unwrap())?,
            paid_at: self.paid_at,
            channel: self.channel.clone(),
            external_ref: self.external_ref.clone(),
            note: self.note.clone(),
        })
        .apply(db, actor, op, owner, now)?;
        let receipt_id = granted["receipt_id"].as_str().ok_or_else(invalid)?;
        db.execute(
            "INSERT INTO cloud_plan_entitlements(period_id,user_id,receipt_id,product_id,product_revision,tier,duration,starts_at,expires_at,month_anchor_day,storage_bytes,max_file_bytes,retention_seconds,product_snapshot_json,purchased_at)
             VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![period_id,owner,receipt_id,product.product_id,product.revision,product.tier,product.duration,starts_at,expires_at,anchor,product.storage_bytes.unwrap(),product.max_file_bytes.unwrap(),product.retention_seconds.unwrap(),json!(product).to_string(),now],
        ).map_err(storage)?;
        Ok(json!({
            "entitlement": entitlement(db, owner, period_id)?,
            "allowance_period": allowance_admin::period(db, owner, period_id)?,
            "receipt_id": receipt_id,
        }))
    }
}

/// The service's calendar is UTC+8, matching existing account reporting. The
/// timestamp remains UTC seconds; a day pass always spans exactly 86,400 seconds.
fn term_end(
    duration: &str,
    starts_at: i64,
    inherited_anchor: Option<u8>,
) -> Result<(i64, Option<u8>), ToolError> {
    let days = match duration {
        "day1" => Some(1),
        "day3" => Some(3),
        "day7" => Some(7),
        "month" => None,
        _ => return Err(invalid()),
    };
    if let Some(days) = days {
        return Ok((
            starts_at.checked_add(days * 86_400).ok_or_else(invalid)?,
            None,
        ));
    }
    let start = OffsetDateTime::from_unix_timestamp(starts_at)
        .map_err(|_| invalid())?
        .to_offset(UtcOffset::from_hms(8, 0, 0).unwrap());
    let anchor = inherited_anchor.unwrap_or(start.day());
    let (year, month) = if start.month() == Month::December {
        (
            start.year().checked_add(1).ok_or_else(invalid)?,
            Month::January,
        )
    } else {
        (start.year(), start.month().next())
    };
    let last_day = month.length(year);
    let end = Date::from_calendar_date(year, month, anchor.min(last_day))
        .map_err(|_| invalid())?
        .with_time(start.time())
        .assume_offset(start.offset());
    Ok((end.unix_timestamp(), Some(anchor)))
}

fn product(db: &Connection, id: &str) -> Result<Option<Product>, ToolError> {
    let saved: Option<String> = db
        .query_row(
            "SELECT configuration_json FROM cloud_plan_products WHERE product_id=?",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    saved
        .map(|text| serde_json::from_str(&text).map_err(|_| invalid()))
        .transpose()
}

fn products(db: &Connection) -> Result<Vec<Product>, ToolError> {
    let mut statement = db
        .prepare("SELECT configuration_json FROM cloud_plan_products ORDER BY product_id")
        .map_err(storage)?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(storage)?;
    rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(|_| invalid()))
        .collect()
}

pub(crate) fn catalog(db: &Connection, reader: bool) -> Result<Value, ToolError> {
    Ok(
        json!({"items": products(db)?.into_iter().filter(|p| !reader || p.saleable()).map(|p| p.public_value()).collect::<Vec<_>>()}),
    )
}

fn entitlement(
    db: &Connection,
    owner: &str,
    period: &str,
) -> Result<Option<Entitlement>, ToolError> {
    let row: Option<(String, String, i64, i64, Option<u8>)> = db.query_row(
        "SELECT product_snapshot_json,receipt_id,starts_at,expires_at,month_anchor_day FROM cloud_plan_entitlements WHERE user_id=? AND period_id=?",
        params![owner, period], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).optional().map_err(storage)?;
    row.map(
        |(saved, receipt_id, starts_at, expires_at, month_anchor_day)| {
            let product: Product = serde_json::from_str(&saved).map_err(|_| invalid())?;
            Ok(Entitlement {
                period_id: period.to_owned(),
                product_id: product.product_id,
                product_revision: product.revision,
                title: product.title,
                tier: product.tier,
                duration: product.duration,
                starts_at,
                expires_at,
                storage_bytes: product.storage_bytes.ok_or_else(invalid)?,
                max_file_bytes: product.max_file_bytes.ok_or_else(invalid)?,
                retention_seconds: product.retention_seconds.ok_or_else(invalid)?,
                receipt_id,
                month_anchor_day,
                basis: product.basis.ok_or_else(invalid)?,
                grant_milli_percent: product.grant_milli_percent.ok_or_else(invalid)?,
                amount_fen: product.amount_fen.ok_or_else(invalid)?,
            })
        },
    )
    .transpose()
}

pub(crate) fn current(
    db: &Connection,
    owner: &str,
    now: i64,
) -> Result<Option<Entitlement>, ToolError> {
    let period: Option<String> = db.query_row(
        "SELECT p.period_id FROM allowance_periods p JOIN cloud_plan_entitlements e USING(period_id,user_id) WHERE p.user_id=? AND p.closed_at IS NULL AND p.starts_at<=? AND ?<p.expires_at",
        params![owner, now, now], |r| r.get(0),
    ).optional().map_err(storage)?;
    period
        .map(|id| entitlement(db, owner, &id))
        .transpose()
        .map(Option::flatten)
}

/// Last purchased term, including expired terms, for preservation and exports.
/// Future booked terms do not give storage or paid processing rights early.
pub(crate) fn latest(db: &Connection, owner: &str) -> Result<Option<Entitlement>, ToolError> {
    let period: Option<String> = db.query_row(
        "SELECT period_id FROM cloud_plan_entitlements WHERE user_id=? ORDER BY expires_at DESC LIMIT 1",
        [owner], |r| r.get(0),
    ).optional().map_err(storage)?;
    period
        .map(|id| entitlement(db, owner, &id))
        .transpose()
        .map(Option::flatten)
}

pub(crate) fn summary(db: &Connection, owner: &str, now: i64) -> Result<Value, ToolError> {
    let (scheduled_end, unlimited): (Option<i64>, bool) = db.query_row(
        "SELECT MAX(expires_at),COALESCE(MAX(unlimited),0) FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND expires_at>?",
        params![owner, now], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(storage)?;
    let next: Option<String> = db.query_row(
        "SELECT p.period_id FROM allowance_periods p JOIN cloud_plan_entitlements e USING(period_id,user_id) WHERE p.user_id=? AND p.closed_at IS NULL AND p.starts_at>? ORDER BY p.starts_at LIMIT 1",
        params![owner, now], |r| r.get(0),
    ).optional().map_err(storage)?;
    Ok(json!({
        "current": current(db, owner, now)?,
        "next": next.map(|id| entitlement(db, owner, &id)).transpose()?.flatten(),
        "catalog": catalog(db, true)?,
        "purchase_starts_at": scheduled_end.unwrap_or(now),
        "purchase_blocked_by_unlimited": unlimited,
    }))
}

impl crate::admin_store::AdminStore {
    pub(crate) fn cloud_plan_catalog(&self) -> Result<Value, ToolError> {
        catalog(&self.control.lock().unwrap().connection, false)
    }

    pub(crate) fn cloud_plan_summary(&self, owner: &str, now: i64) -> Result<Value, ToolError> {
        crate::user_storage_paths::validate_user_id(owner)?;
        let control = self.control.lock().unwrap();
        let exists: bool = control
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)",
                [owner],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if !exists {
            return Err(crate::authorization::missing());
        }
        summary(&control.connection, owner, now)
    }
}

#[cfg(test)]
#[path = "cloud_plans_tests.rs"]
mod tests;

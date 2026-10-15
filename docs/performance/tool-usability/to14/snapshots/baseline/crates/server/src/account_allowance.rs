//! ADM1 monetary units. Receipt amounts and model-cost allowances cannot be
//! mixed implicitly. Signed micro_cny also represents adjustments and deficits.
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

/// One millionth of a CNY yuan. Provider-specific pricing belongs to ADM4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MicroCny(i64);

impl MicroCny {
    pub const ZERO: Self = Self(0);
    pub const PER_YUAN: i64 = 1_000_000;

    pub const fn new(value: i64) -> Self {
        Self(value)
    }
    pub const fn value(self) -> i64 {
        self.0
    }
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl ToSql for MicroCny {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl FromSql for MicroCny {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

/// A positive manual receipt in hundredths of a CNY yuan. No automatic
/// conversion to an allowance: the operator supplies the grant separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct ReceiptFen(i64);

impl ReceiptFen {
    pub const PER_YUAN: i64 = 100;
    pub const fn value(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for ReceiptFen {
    type Error = &'static str;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err("Receipt amount_fen must be positive")
        }
    }
}
impl From<ReceiptFen> for i64 {
    fn from(value: ReceiptFen) -> Self {
        value.0
    }
}
impl ToSql for ReceiptFen {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}
impl FromSql for ReceiptFen {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let amount = i64::column_result(value)?;
        Self::try_from(amount).map_err(|_| rusqlite::types::FromSqlError::OutOfRange(amount))
    }
}

#[derive(Debug, Serialize)]
pub struct AllowanceBalance {
    pub granted_micro_cny: MicroCny,
    pub debited_micro_cny: MicroCny,
    pub active_reserved_micro_cny: MicroCny,
    pub pending_micro_cny: MicroCny,
    pub available_micro_cny: MicroCny,
}
pub(crate) fn balance(
    db: &rusqlite::Connection,
    period: &str,
) -> Result<AllowanceBalance, read_tools::ToolError> {
    let granted: i64 = db
        .query_row(
            "SELECT COALESCE(sum(delta_micro_cny),0) FROM allowance_adjustments WHERE period_id=?",
            [period],
            |r| r.get(0),
        )
        .map_err(crate::allowance_admin::storage)?;
    let (debit,active,pending): (i64,i64,i64) = db.query_row("SELECT COALESCE(sum(account_debit_micro_cny),0),COALESCE(sum(CASE WHEN state IN ('reserved','sent') THEN reserved_micro_cny ELSE 0 END),0),COALESCE(sum(CASE WHEN state='pending' THEN reserved_micro_cny ELSE 0 END),0) FROM model_call_charges WHERE period_id=?",[period],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(crate::allowance_admin::storage)?;
    let available = granted
        .checked_sub(debit)
        .and_then(|v| v.checked_sub(active))
        .and_then(|v| v.checked_sub(pending))
        .ok_or_else(crate::admin_store::invalid)?;
    Ok(AllowanceBalance {
        granted_micro_cny: MicroCny(granted),
        debited_micro_cny: MicroCny(debit),
        active_reserved_micro_cny: MicroCny(active),
        pending_micro_cny: MicroCny(pending),
        available_micro_cny: MicroCny(available),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adm1_integer_money_roundtrips_without_float_or_unit_conversion() {
        let root = tempfile::tempdir().unwrap();
        let db = rusqlite::Connection::open(root.path().join("money.sqlite")).unwrap();
        let receipt = ReceiptFen::try_from(123).unwrap();
        // Beyond f64's exact integer range, but still exact in JSON and SQLite.
        let allowance = MicroCny::new(9_007_199_254_740_993);
        let pair = db
            .query_row("SELECT ?1,?2", rusqlite::params![receipt, allowance], |r| {
                Ok((r.get::<_, ReceiptFen>(0)?, r.get::<_, MicroCny>(1)?))
            })
            .unwrap();
        assert_eq!(pair, (receipt, allowance));
        for amount in [allowance, MicroCny::new(-1), MicroCny::ZERO] {
            let json = serde_json::to_string(&amount).unwrap();
            assert_eq!(serde_json::from_str::<MicroCny>(&json).unwrap(), amount);
        }
        assert_eq!(serde_json::to_string(&receipt).unwrap(), "123");
        assert_eq!(serde_json::from_str::<ReceiptFen>("123").unwrap(), receipt);
        for invalid in ["0", "-1", "1.5", "9223372036854775808"] {
            assert!(serde_json::from_str::<ReceiptFen>(invalid).is_err());
        }
        assert!(db
            .query_row("SELECT -1", [], |r| r.get::<_, ReceiptFen>(0))
            .is_err());
        assert!(db
            .query_row("SELECT 1.5", [], |r| r.get::<_, MicroCny>(0))
            .is_err());
        assert_eq!(
            MicroCny::new(2).checked_sub(MicroCny::new(3)),
            Some(MicroCny::new(-1))
        );
        assert_eq!(MicroCny::new(i64::MAX).checked_add(MicroCny::new(1)), None);
        assert_eq!(MicroCny::new(i64::MIN).checked_sub(MicroCny::new(1)), None);
    }
}

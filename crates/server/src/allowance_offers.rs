//! Configured manual-payment offers. Reading or choosing an offer grants nothing.
use crate::{account_allowance::{AllowanceBasis, MicroCny, ReceiptFen}, admin_store::invalid, allowance_admin::storage};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Offer {
    pub offer_id: String,
    pub title: String,
    pub amount_fen: ReceiptFen,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_micro_cny: Option<MicroCny>,
    #[serde(default)]
    pub delta_milli_percent: i64,
    #[serde(default)]
    pub basis_version: String,
    pub instructions: String,
    pub enabled: bool,
}
impl Offer {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        if let Some(amount) = self.budget_micro_cny {
            self.basis_version = AllowanceBasis::for_budget(amount)?.version;
            self.delta_milli_percent = 100_000;
        }
        for text in [&mut self.offer_id, &mut self.title, &mut self.basis_version, &mut self.instructions] {
            *text = text.trim().to_owned();
            if text.is_empty() { return Err(invalid()); }
        }
        if self.delta_milli_percent <= 0 { return Err(invalid()); }
        Ok(())
    }
    pub(crate) fn apply(&self, db: &Connection, now: i64) -> Result<Value, ToolError> {
        if let Some(amount) = self.budget_micro_cny {
            let basis = AllowanceBasis::for_budget(amount)?;
            db.execute("INSERT OR IGNORE INTO allowance_bases(version,basis_micro_cny) VALUES(?,?)", params![basis.version, amount]).map_err(storage)?;
        }
        let amount: Option<MicroCny> = db.query_row("SELECT basis_micro_cny FROM allowance_bases WHERE version=?",[&self.basis_version],|r|r.get(0)).optional().map_err(storage)?;
        if self.budget_micro_cny.is_some_and(|budget| Some(budget) != amount) {
            return Err(crate::user_storage_paths::error("ALLOWANCE_BASIS_VERSION_CONFLICT", "conflict", "额度换算配置发生冲突"));
        }
        let basis = AllowanceBasis {version:self.basis_version.clone(),basis_micro_cny:amount.ok_or_else(||crate::user_storage_paths::error("ALLOWANCE_BASIS_UNBOUND","conflict","请先配置额度基准，再保存补充方案"))?};
        basis.grant(self.delta_milli_percent)?;
        db.execute("INSERT INTO allowance_offers VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(offer_id) DO UPDATE SET title=excluded.title,amount_fen=excluded.amount_fen,delta_milli_percent=excluded.delta_milli_percent,basis_version=excluded.basis_version,instructions=excluded.instructions,enabled=excluded.enabled,updated_at=excluded.updated_at",
            params![self.offer_id,self.title,self.amount_fen,self.delta_milli_percent,self.basis_version,self.instructions,self.enabled,now]).map_err(storage)?;
        Ok(json!({"allowance_offer":self}))
    }
}

pub(crate) fn list(db: &Connection, active_period: bool, reader: bool) -> Result<Value, ToolError> {
    let mut query = db.prepare("SELECT o.offer_id,o.title,o.amount_fen,o.delta_milli_percent,o.basis_version,o.instructions,o.enabled,b.basis_micro_cny FROM allowance_offers o JOIN allowance_bases b ON b.version=o.basis_version WHERE (?=0 OR (o.enabled=1 AND ?=1)) ORDER BY o.amount_fen,o.offer_id").map_err(storage)?;
    let mut items = query.query_map(params![reader,active_period],|r| Ok(json!({
        "offer_id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"amount_fen":r.get::<_,i64>(2)?,
        "delta_milli_percent":r.get::<_,i64>(3)?,"basis_version":r.get::<_,String>(4)?,
        "instructions":r.get::<_,String>(5)?,"enabled":r.get::<_,bool>(6)?,"basis_micro_cny":r.get::<_,i64>(7)?
    }))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    for item in &mut items {
        let basis = AllowanceBasis { version: item["basis_version"].as_str().unwrap().to_owned(), basis_micro_cny: MicroCny::new(item["basis_micro_cny"].as_i64().unwrap()) };
        item["budget_micro_cny"] = json!(basis.grant(item["delta_milli_percent"].as_i64().unwrap())?);
        item.as_object_mut().unwrap().remove("basis_micro_cny");
    }
    Ok(json!({"items":items}))
}

impl crate::admin_store::AdminStore {
    pub(crate) fn allowance_offers(&self) -> Result<Value,ToolError> {
        list(&self.control.lock().unwrap().connection,false,false)
    }
}

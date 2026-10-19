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
    pub delta_milli_percent: i64,
    pub basis_version: String,
    pub instructions: String,
    pub enabled: bool,
}
impl Offer {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        for text in [&mut self.offer_id, &mut self.title, &mut self.basis_version, &mut self.instructions] {
            *text = text.trim().to_owned();
            if text.is_empty() { return Err(invalid()); }
        }
        if self.delta_milli_percent <= 0 { return Err(invalid()); }
        Ok(())
    }
    pub(crate) fn apply(&self, db: &Connection, now: i64) -> Result<Value, ToolError> {
        let amount: Option<MicroCny> = db.query_row("SELECT basis_micro_cny FROM allowance_bases WHERE version=?",[&self.basis_version],|r|r.get(0)).optional().map_err(storage)?;
        let basis = AllowanceBasis {version:self.basis_version.clone(),basis_micro_cny:amount.ok_or_else(||crate::user_storage_paths::error("ALLOWANCE_BASIS_UNBOUND","conflict","请先配置额度基准，再保存补充方案"))?};
        basis.grant(self.delta_milli_percent)?;
        db.execute("INSERT INTO allowance_offers VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(offer_id) DO UPDATE SET title=excluded.title,amount_fen=excluded.amount_fen,delta_milli_percent=excluded.delta_milli_percent,basis_version=excluded.basis_version,instructions=excluded.instructions,enabled=excluded.enabled,updated_at=excluded.updated_at",
            params![self.offer_id,self.title,self.amount_fen,self.delta_milli_percent,self.basis_version,self.instructions,self.enabled,now]).map_err(storage)?;
        Ok(json!({"allowance_offer":self}))
    }
}

pub(crate) fn list(db: &Connection, basis_version: Option<&str>, reader: bool) -> Result<Value, ToolError> {
    let mut query = db.prepare("SELECT offer_id,title,amount_fen,delta_milli_percent,basis_version,instructions,enabled FROM allowance_offers WHERE (?=0 OR (enabled=1 AND basis_version=?)) ORDER BY amount_fen,offer_id").map_err(storage)?;
    let items = query.query_map(params![reader,basis_version],|r| Ok(json!({
        "offer_id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"amount_fen":r.get::<_,i64>(2)?,
        "delta_milli_percent":r.get::<_,i64>(3)?,"basis_version":r.get::<_,String>(4)?,
        "instructions":r.get::<_,String>(5)?,"enabled":r.get::<_,bool>(6)?
    }))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    Ok(json!({"items":items}))
}

impl crate::admin_store::AdminStore {
    pub(crate) fn allowance_offers(&self) -> Result<Value,ToolError> {
        list(&self.control.lock().unwrap().connection,None,false)
    }
}

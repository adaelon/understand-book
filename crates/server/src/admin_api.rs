//! Typed management requests after host authentication, Origin, CSRF and admin authorization.
use crate::{
    admin_store::{self, Command},
    auth::Principal,
    authorization::Authorization,
    published_library::PublishedBookRef,
};
use read_tools::ToolError;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    operation_id: String,
    user_id: String,
    password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Password {
    operation_id: String,
    password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    operation_id: String,
    disabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InviteBatch {
    operation_id: String,
    count: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct McpGift {
    operation_id: String,
    units: i64,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BookOperation {
    operation_id: String,
    published_book_ref: BookRef,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BookRef {
    book_id: String,
    publication_id: String,
}
fn parse<T: DeserializeOwned>(value: Value) -> Result<T, ToolError> {
    serde_json::from_value(value).map_err(|_| admin_store::invalid())
}
pub(crate) fn page(query: &HashMap<String, String>) -> Result<(u32, u32), ToolError> {
    if query
        .keys()
        .any(|k| !matches!(k.as_str(), "limit" | "offset"))
    {
        return Err(admin_store::invalid());
    }
    let number = |key, default| {
        query
            .get(key)
            .map(|v| v.parse::<u32>().map_err(|_| admin_store::invalid()))
            .unwrap_or(Ok(default))
    };
    let limit = number("limit", 50)?;
    if !(1..=100).contains(&limit) {
        return Err(admin_store::invalid());
    }
    Ok((limit, number("offset", 0)?))
}

pub(crate) fn charge_reference(query: &mut HashMap<String, String>, reader: bool) -> Result<Option<(&'static str, String)>, ToolError> {
    let mut scopes = Vec::new();
    for (key, kind) in [("parent_task_ref", "parent_task"), ("learning_workspace_id", "workspace")] {
        if let Some(value) = query.remove(key) { scopes.push((kind, value)); }
    }
    if let Some(value) = query.remove("account_level") {
        match value.as_str() {
            "true" => scopes.push(("account_level", String::new())),
            "false" => {},
            _ => return Err(admin_store::invalid()),
        }
    }
    if !reader {
        for (key, kind) in [("run_ref", "run"), ("task_ref", "task"), ("operator_task_ref", "operator_task")] {
            if let Some(value) = query.remove(key) { scopes.push((kind, value)); }
        }
    }
    if scopes.len() > 1 || scopes.iter().any(|(kind,value)| *kind != "account_level" && value.trim().is_empty()) {
        return Err(admin_store::invalid());
    }
    Ok(scopes.pop())
}

pub(crate) fn dispatch(
    access: &Arc<Authorization>,
    principal: &Principal,
    method: &str,
    path: &str,
    query: &HashMap<String, String>,
    input: Value,
    now: i64,
) -> Result<Value, ToolError> {
    let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
    if parts.as_slice() == ["admin", "operations"] {
        if method != "GET" || !query.is_empty() { return Err(admin_store::invalid()); }
        return crate::operations::report(access, now);
    }
    if parts.as_slice() == ["admin", "monitor"] {
        if method != "GET" || !query.is_empty() { return Err(admin_store::invalid()); }
        return access.monitor.snapshot(&access.admin, access.runs.limits.active_runs, now);
    }
    if matches!(parts.as_slice(),["admin","operator-builds",..]) {
        return crate::operator_builds::dispatch(access,principal.user_id(),method,&parts[2..],query,input,now);
    }
    if matches!(parts.as_slice(), ["admin", "workspace-releases", ..]) {
        if let ("POST",["admin","workspace-releases",release_id,"archive"])=(method,parts.as_slice()) {
            #[derive(Deserialize)] #[serde(deny_unknown_fields)] struct Archive {operation_id:String,archived:bool}
            if !query.is_empty(){return Err(admin_store::invalid());}
            let body:Archive=parse(input)?;
            return access.admin.apply(principal.user_id(),&body.operation_id,principal.user_id(),Command::WorkspaceReleaseArchive(crate::workspace_publication::Archive{release_id:(*release_id).into(),archived:body.archived}),now).map(|(value,_)|value);
        }
        if method == "POST" && parts.len() == 2 && query.is_empty() {
            let mut input = input;
            let id = input.as_object_mut().and_then(|value|value.remove("operation_id"))
                .and_then(|value|value.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
            return access.admin.apply(principal.user_id(), &id, principal.user_id(),
                Command::WorkspaceRelease(parse(input)?), now).map(|(receipt,_)|receipt);
        }
        if method != "GET" { return Err(admin_store::invalid()); }
        return match parts.as_slice() {
            ["admin", "workspace-releases"] => {
                let (limit,offset) = page(query)?;
                access.admin.workspace_releases(None,limit,offset,None)
            },
            ["admin", "workspace-releases", id, "versions", version] if query.is_empty() =>
                access.admin.workspace_release(None,id,version.parse().map_err(|_|admin_store::invalid())?,false),
            ["admin", "workspace-releases", id, "versions", version, "export"] if query.is_empty() =>
                access.admin.workspace_release(None,id,version.parse().map_err(|_|admin_store::invalid())?,true),
            _ => Err(crate::authorization::missing()),
        };
    }
    if method == "GET" && parts.as_slice() == ["admin", "cost-summary"] {
        let mut pagination = query.clone();
        let owner = pagination.remove("user_id");
        if let Some(owner) = &owner { crate::user_storage_paths::validate_user_id(owner)?; }
        let range = if pagination.contains_key("from") || pagination.contains_key("to") {
            Some(crate::admin_usage::range(&mut pagination,now)?)
        } else { None };
        let (limit,offset) = page(&pagination)?;
        return access.spend.cost_summary(owner.as_deref(),range.as_ref(),limit,offset,false,now);
    }
    if parts.as_slice() == ["admin", "allowance-offers"] {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        if method == "GET" { return access.admin.allowance_offers(); }
        if method != "POST" { return Err(admin_store::invalid()); }
        let mut input = input;
        let id = input.as_object_mut().and_then(|value|value.remove("operation_id"))
            .and_then(|value|value.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
        return access.admin.apply(principal.user_id(), &id, principal.user_id(),
            Command::AllowanceOffer(parse(input)?), now).map(|(receipt,_)|receipt);
    }
    if parts.as_slice() == ["admin", "cloud-plans"] {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        if method == "GET" { return access.admin.cloud_plan_catalog(); }
        if method != "POST" { return Err(admin_store::invalid()); }
        let mut input = input;
        let id = input.as_object_mut().and_then(|value|value.remove("operation_id"))
            .and_then(|value|value.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
        return access.admin.apply(principal.user_id(), &id, principal.user_id(),
            Command::CloudPlan(crate::cloud_plans::Mutation::Product(parse(input)?)), now).map(|(receipt,_)|receipt);
    }
    if let ("GET", ["admin", "users", owner, "cloud-plan-purchases"]) = (method, parts.as_slice()) {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        return access.admin.cloud_plan_summary(owner, now);
    }
    if matches!(parts.as_slice(), ["admin", "build-tasks", _, ..]) {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        return match (method, parts.as_slice()) {
            ("GET", ["admin", "build-tasks", task]) => access.spend.build_status(task, None),
            ("GET", ["admin", "build-tasks", task, "charges", call]) => access.spend.build_status(task, Some(call)),
            ("GET", ["admin", "build-tasks", task, "service-charges", usage]) => access.spend.service_charge(task, usage),
            ("POST", ["admin", "build-tasks", task, "service-admit"]) => access.spend.service_admit(task, parse(input)?, now),
            ("POST", ["admin", "build-tasks", task, "service-receipts"]) => access.spend.service_receipt(task, parse(input)?, now),
            ("POST", ["admin", "build-tasks", task, "budget"]) => access.spend.build_budget(principal.user_id(), task, parse(input)?, now),
            ("POST", ["admin", "build-tasks", task, "metered-admit"]) => access.spend.build_metered_admit(task, input, now),
            ("POST", ["admin", "build-tasks", task, "metered-receipts"]) => access.spend.build_metered_receipt(task, parse(input)?, now),
            ("POST", ["admin", "build-tasks", task, "admit"]) => access.spend.build_admit(task, parse(input)?, now),
            ("POST", ["admin", "build-tasks", task, "receipts"]) => access.spend.build_receipt(task, parse(input)?, now),
            _ => Err(crate::authorization::missing()),
        };
    }
    if let ("POST", ["admin", "service-usage", id, "reconcile"]) = (method, parts.as_slice()) {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        return access.spend.service_reconcile(principal.user_id(),id,parse(input)?,now);
    }
    if method == "GET" && matches!(parts.as_slice(), ["admin", "users", _, "mcp"]) {
        if !query.is_empty() { return Err(admin_store::invalid()); }
        return serde_json::to_value(access.mcp_usage.status(parts[2])?).map_err(|_|admin_store::invalid());
    }
    if method == "GET" && parts.as_slice() == ["admin", "invites"] {
        let mut pagination = query.clone();
        let state = pagination.remove("state");
        let (limit, offset) = page(&pagination)?;
        return access.admin.invites(state.as_deref(), limit, offset);
    }
    if method == "GET" && (parts.as_slice() == ["admin", "usage"] || matches!(parts.as_slice(), ["admin", "users", _, "usage"])) {
        let mut pagination = query.clone();
        let owner = if parts.len() == 4 { Some(parts[2].to_owned()) } else { pagination.remove("user_id") };
        let range = crate::admin_usage::range(&mut pagination, now)?;
        let (limit,offset) = page(&pagination)?;
        return access.admin.usage(owner.as_deref(), &range, limit, offset, now);
    }
    if method == "GET" && parts.as_slice() == ["admin", "users"] {
        let mut pagination = query.clone();
        let search = pagination.remove("search").unwrap_or_default();
        let disabled = match pagination.remove("disabled").as_deref() {
            None => None,
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => return Err(admin_store::invalid()),
        };
        let (limit, offset) = page(&pagination)?;
        return access.admin.users(limit, offset, search.trim(), disabled, now);
    }
    if method == "GET" && parts.as_slice() == ["admin", "charges"] {
        let mut pagination = query.clone();
        let owner = pagination.remove("user_id");
        if let Some(owner) = &owner { crate::user_storage_paths::validate_user_id(owner)?; }
        let pending = match pagination.remove("pending").as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(admin_store::invalid()),
        };
        let range = if pagination.contains_key("from") || pagination.contains_key("to") {
            Some(crate::admin_usage::range(&mut pagination,now)?)
        } else { None };
        let reference = charge_reference(&mut pagination, false)?;
        let cost_owner = pagination.remove("cost_owner");
        if cost_owner.as_deref().is_some_and(|value| !matches!(value,"account"|"platform")) { return Err(admin_store::invalid()); }
        let (limit,offset)=page(&pagination)?;
        return access.spend.charges(owner.as_deref(),pending,limit,offset,false,range.as_ref(),reference.as_ref().map(|(kind,value)|(*kind,value.as_str())),cost_owner.as_deref());
    }
    if method == "GET" {
        let (limit, offset) = page(query)?;
        return match parts.as_slice() {
            ["admin", "users", owner] => access.admin.user(owner, limit, offset),
            ["admin", "users", owner, "operations"] => access.admin.user_operations(owner, limit, offset),
            ["admin", "users", owner, kind @ ("allowance-periods"
            | "receipts"
            | "allowance-adjustments"
            | "receipt-corrections")] => access.admin.allowance_list(owner, kind, limit, offset),
            ["admin", "books"] => access.admin.books(limit, offset),
            ["admin", "charges", call] => access.spend.charge_detail(call,limit,offset),
            ["admin", "operations", id] if query.is_empty() => {
                access.admin.operation(principal.user_id(), id)
            }
            ["admin", "invite-batches", id] if query.is_empty() => {
                access.admin.invite_batch(principal.user_id(), id)
            }
            _ => Err(crate::authorization::missing()),
        };
    }
    if method != "POST" || !query.is_empty() {
        return Err(admin_store::invalid());
    }
    let (owner, id, command) = match parts.as_slice() {
        ["admin", "users", owner, "mcp", "gifts"] => {
            let body: McpGift = parse(input)?;
            return access.mcp_usage.gift(principal, &body.operation_id, owner, body.units, &body.reason);
        }
        ["admin", "users", owner, "mcp", action @ ("packs" | "corrections")] => {
            let mut input = input;
            let id = input.as_object_mut().and_then(|v|v.remove("operation_id"))
                .and_then(|v|v.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
            let mutation = if *action == "packs" {
                crate::mcp_pack::Mutation::Purchase(parse(input)?)
            } else {
                crate::mcp_pack::Mutation::Correction(parse(input)?)
            };
            ((*owner).to_owned(), id, Command::Mcp(mutation))
        }
        ["admin", "invite-batches"] => {
            let body: InviteBatch = parse(input)?;
            return access.admin.create_invites(principal.user_id(), &body.operation_id, body.count, now);
        }
        ["admin", "invites", id, "disable"] => {
            if !input.is_null() { let _: Empty = parse(input)?; }
            return access.admin.disable_invite(id, now);
        }
        ["admin", "charges", call, "reconcile"] => {
            let mut input = input;
            let id = input.as_object_mut().and_then(|v|v.remove("operation_id"))
                .and_then(|v|v.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
            let reconcile = parse(input)?;
            let detail = access.spend.charge_detail(call,1,0)?;
            let owner = detail["user_id"].as_str().ok_or_else(||crate::user_storage_paths::error("CHARGE_ACCOUNT_REQUIRED","conflict","Account reconciliation requires an account-owned charge"))?;
            (owner.to_owned(),id,Command::Reconcile((*call).into(),reconcile))
        }
        ["admin", "users"] => {
            let body: Create = parse(input)?;
            (
                body.user_id,
                body.operation_id,
                Command::Create(body.password),
            )
        }
        ["admin", "users", owner, action] => {
            let (id, command) = match *action {
                "password" => {
                    let body: Password = parse(input)?;
                    (body.operation_id, Command::Password(body.password))
                }
                "status" => {
                    let body: Status = parse(input)?;
                    (body.operation_id, Command::Status(body.disabled))
                }
                "revoke-sessions" => {
                    let body: Operation = parse(input)?;
                    (body.operation_id, Command::RevokeSessions)
                }
                "cloud-plan-purchases" => {
                    let mut input = input;
                    let id = input.as_object_mut().and_then(|value|value.remove("operation_id"))
                        .and_then(|value|value.as_str().map(str::to_owned)).ok_or_else(admin_store::invalid)?;
                    (id, Command::CloudPlan(crate::cloud_plans::Mutation::Purchase(parse(input)?)))
                }
                "book-grants" | "book-revocations" => {
                    let body: BookOperation = parse(input)?;
                    let reference = PublishedBookRef {
                        book_id: body.published_book_ref.book_id,
                        publication_id: body.published_book_ref.publication_id,
                    };
                    (
                        body.operation_id,
                        if *action == "book-grants" {
                            Command::Grant(reference)
                        } else {
                            Command::Revoke(reference)
                        },
                    )
                }
                "allowance-periods"
                | "allowance-basis"
                | "allowance-percentage-adjustments"
                | "percentage-receipts"
                | "percentage-receipt-corrections"
                | "allowance-adjustments"
                | "allowance-validity"
                | "receipts"
                | "receipt-corrections" => {
                    use crate::allowance_admin::Mutation;
                    let mut input = input;
                    let id = input
                        .as_object_mut()
                        .and_then(|v| v.remove("operation_id"))
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .ok_or_else(admin_store::invalid)?;
                    let mutation = match *action {
                        "allowance-periods" => Mutation::Period(parse(input)?),
                        "allowance-basis" => Mutation::Basis(parse(input)?),
                        "allowance-percentage-adjustments" => Mutation::PercentageAdjustment(parse(input)?),
                        "percentage-receipts" => Mutation::PercentageReceipt(parse(input)?),
                        "percentage-receipt-corrections" => Mutation::PercentageCorrection(parse(input)?),
                        "allowance-adjustments" => Mutation::Adjustment(parse(input)?),
                        "allowance-validity" => Mutation::Validity(parse(input)?),
                        "receipts" => Mutation::Receipt(parse(input)?),
                        _ => Mutation::Correction(parse(input)?),
                    };
                    (id, Command::Allowance(mutation))
                }
                _ => return Err(crate::authorization::missing()),
            };
            ((*owner).to_owned(), id, command)
        }
        _ => return Err(crate::authorization::missing()),
    };
    let (receipt, disable) = access
        .admin
        .apply(principal.user_id(), &id, &owner, command, now)?;
    if disable {
        access.runs.cancel_disabled_user(&owner);
    }
    Ok(receipt)
}

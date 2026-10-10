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
        let run = pagination.remove("run_ref");
        let task = pagination.remove("task_ref");
        if run.is_some() && task.is_some() { return Err(admin_store::invalid()); }
        let reference = run.as_deref().map(|v|("run",v)).or_else(||task.as_deref().map(|v|("task",v)));
        let (limit,offset)=page(&pagination)?;
        return access.spend.charges(owner.as_deref(),pending,limit,offset,false,range.as_ref(),reference);
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
            (detail["user_id"].as_str().unwrap().to_owned(),id,Command::Reconcile((*call).into(),reconcile))
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

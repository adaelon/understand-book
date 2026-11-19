//! Cloud Reader host and account/service operations.
use read_tools::{EvidenceRange, ToolError};
use runtime::{ModelAdapter, ProviderConfig, ProviderRegistry, Message};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub use reader_service::*;
pub mod workspace_registry;
mod workspace_client;
pub mod control_store;
pub mod learning_workspace;
pub mod account_allowance;
mod allowance_admin;
pub mod model_rates;
mod model_spend_store;
mod external_build_costs;
mod allowance_offers;
mod cloud_plans;
mod cloud_storage;
mod service_usage;
mod web_usage;
mod web_search;
mod course_access;
mod exam_sprints;
mod desktop_auth;
mod selection_translation;
mod charge_admin;
mod admin_api;
mod model_settings_api;
mod server_monitor;
mod operations;
mod beta_invites;
mod account_mail;
mod account_registration;
mod account_email;
mod account_password;
mod admin_store;
mod admin_usage;
mod cost_summary;
mod workspace_publication;
mod inquiry_course;
mod workspace_release_instances;
mod operator_builds;
pub mod reader_maintenance;
pub mod user_registry;
pub mod online_publication;
pub mod auth;
pub mod mcp_access;
pub mod mcp_read;
mod workspace_retrieval;
pub mod mcp_usage;
pub mod mcp_pack;
mod remote_mcp;
pub mod authorization;
pub mod multi_user_host;
mod multi_user_review;
mod run_admission;
mod personal_preparations;

pub mod host {
    pub use reader_service::host::*;
    pub use crate::multi_user_host::{start as start_multi_user_server, MultiUserConfig};
}

pub mod agent_run;

pub mod session_recap;

pub mod teaching_coach;
pub mod practice_check;
pub mod learning_map_build;
pub mod reading_guidance;
pub mod voice_input;
pub mod learning_task;
pub mod learning_record_api;
pub mod cross_book;
pub mod service_limits;

pub mod published_library;
pub mod video_media;
pub mod video_audio;
pub mod personal_chat;
pub mod mcp_output_schema;

#[cfg(test)]
mod tests;

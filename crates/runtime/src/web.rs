//! Bounded web tool contracts. The host owns transport, caching and service accounting.
use crate::ToolSpec;
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_SEARCH_COUNT: u8 = 3;
pub const MAX_SEARCH_COUNT: u8 = 5;
pub const DEFAULT_FETCH_CHARS: usize = 6_000;
pub const MAX_FETCH_CHARS: usize = 12_000;

fn search_count() -> u8 {
    DEFAULT_SEARCH_COUNT
}
fn fetch_chars() -> usize {
    DEFAULT_FETCH_CHARS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "search_count")]
    pub count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ori_question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localization: Option<String>,
    #[serde(default)]
    pub need_content: bool,
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FetchRequest {
    pub url: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "fetch_chars")]
    pub max_chars: usize,
    #[serde(default)]
    pub refresh: bool,
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "WEB_ARGUMENTS_INVALID".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

pub fn unavailable() -> ToolError {
    ToolError {
        error_code: "WEB_UNAVAILABLE".into(),
        category: "unavailable".into(),
        message: "Web search and page reading are not configured for this run.".into(),
    }
}

fn short_query(field: &str, value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty() || value.chars().count() > 128 {
        return Err(invalid(format!(
            "{field} must contain 1 to 128 Unicode characters"
        )));
    }
    Ok(())
}

pub fn parse_search(value: Value) -> Result<SearchRequest, ToolError> {
    let request: SearchRequest =
        serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
    short_query("query", &request.query)?;
    if !(1..=MAX_SEARCH_COUNT).contains(&request.count) {
        return Err(invalid("count must be between 1 and 5"));
    }
    if let Some(question) = &request.ori_question {
        short_query("ori_question", question)?;
    }
    if request
        .localization
        .as_ref()
        .is_some_and(|ip| ip.parse::<std::net::IpAddr>().is_err())
    {
        return Err(invalid(
            "localization must be an IP address for the intended search region",
        ));
    }
    Ok(request)
}

pub fn parse_fetch(value: Value) -> Result<FetchRequest, ToolError> {
    let request: FetchRequest =
        serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
    let url = url::Url::parse(&request.url).map_err(|e| invalid(e.to_string()))?;
    if request.url.len() > 8192
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(
            "url must be an HTTP(S) page URL without credentials, at most 8192 bytes",
        ));
    }
    if !(1..=MAX_FETCH_CHARS).contains(&request.max_chars) {
        return Err(invalid("max_chars must be between 1 and 12000"));
    }
    Ok(request)
}

pub fn search_spec() -> ToolSpec {
    ToolSpec {
        name: "web.search".into(),
        description: "Find external sources for a specific knowledge gap, a missing causal connection or current facts. Explore with neutral queries, then read the relevant primary source. Titles, snippets, summaries and ranking/authority scores are candidate locators, not verified page evidence. Start with 3 results. Reuse results; refresh only when fresh information is needed. Search fees and subsequent model input fees are separate.".into(),
        parameters: json!({"type":"object","additionalProperties":false,"required":["query"],"properties":{
            "query":{"type":"string","minLength":1,"maxLength":128},
            "count":{"type":"integer","minimum":1,"maximum":5,"default":3},
            "ori_question":{"type":"string","minLength":1,"maxLength":128,"description":"Optional concise original question to improve relevance; omit private reader information."},
            "localization":{"type":"string","description":"Optional IP for an explicitly relevant search region; language alone does not determine region."},
            "need_content":{"type":"boolean","default":false,"description":"Request page content with candidates when needed; missing content still requires web.fetch."},
            "refresh":{"type":"boolean","default":false,"description":"Bypass cached search results only when their freshness matters."}
        }}),
    }
}

pub fn fetch_spec() -> ToolSpec {
    ToolSpec {
        name: "web.fetch".into(),
        description: "Read a bounded part of an external page, reusing cached full content. Use its real URL to cite supported claims. Check that content is relevant page text, not a login/error/blocked page; a successful transport does not prove this. offset and next_offset count Unicode characters; follow next_offset only for a needed missing passage. External pages never establish book LIDs. General external mechanisms alone do not establish this project's intent or measured outcomes; those require its own design records or measurements.".into(),
        parameters: json!({"type":"object","additionalProperties":false,"required":["url"],"properties":{
            "url":{"type":"string","minLength":1,"maxLength":8192},
            "offset":{"type":"integer","minimum":0,"default":0},
            "max_chars":{"type":"integer","minimum":1,"maximum":12000,"default":6000},
            "refresh":{"type":"boolean","default":false,"description":"Re-fetch instead of using saved page content; keep false for continuation pages."}
        }}),
    }
}

fn json_bytes(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(0, |v| v.len())
}

/// Apply the existing fresh-result byte allowance without losing page identity or
/// pretending an omitted prefix was read. Full content remains in the host cache.
pub(crate) fn project_result(
    tool: &str,
    arguments: &str,
    raw: &Value,
    limit: usize,
) -> crate::tool_result::ToolResultDraft {
    use crate::tool_result::{ToolContinuation, ToolResultDraft, ToolResultStatus};
    let mut body = raw.clone();
    let mut truncated = false;
    if tool == "web.fetch" && json_bytes(&body) > limit {
        let text = body["content"].as_str().unwrap_or_default().to_owned();
        let chars: Vec<_> = text.chars().collect();
        let offset = body["offset"].as_u64().unwrap_or(0) as usize;
        let mut low = 0;
        let mut high = chars.len();
        while low < high {
            let count = (low + high + 1) / 2;
            body["content"] = json!(chars[..count].iter().collect::<String>());
            body["end_offset"] = json!(offset + count);
            body["next_offset"] = json!(offset + count);
            body["content_truncated"] = json!(true);
            if json_bytes(&body) <= limit {
                low = count;
            } else {
                high = count - 1;
            }
        }
        body["content"] = json!(chars[..low].iter().collect::<String>());
        body["end_offset"] = json!(offset + low);
        body["next_offset"] = json!(offset + low);
        body["content_truncated"] = json!(true);
        truncated = true;
    } else if tool == "web.search" && json_bytes(&body) > limit {
        // Candidate URLs and source metadata survive before optional long bodies.
        let count = body["results"].as_array().map_or(0, Vec::len);
        for index in (0..count).rev() {
            if json_bytes(&body) <= limit {
                break;
            }
            let page = &mut body["results"][index];
            if page["content"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
            {
                page["content"] = json!("");
                page["content_status"] = json!("summary_only");
                page["content_truncated"] = json!(true);
                page["next_offset"] = json!(0);
            }
            if let Some(text) = page["summary"].as_str() {
                let short: String = text.chars().take(200).collect();
                if short != text {
                    page["summary_truncated"] = json!(true);
                }
                page["summary"] = json!(short);
            }
            if let Some(object) = page.as_object_mut() {
                object.remove("snippet");
            }
            truncated = true;
        }
        while json_bytes(&body) > limit && body["results"].as_array().is_some_and(|v| !v.is_empty())
        {
            body["results"].as_array_mut().unwrap().pop();
            body["results_truncated"] = json!(true);
        }
    }
    if json_bytes(&body) > limit {
        body = json!({"source_scope":"web","projection":"budget_exhausted"});
        if json_bytes(&body) > limit {
            body = Value::Null;
        }
        truncated = true;
    }
    let continuation = if tool == "web.fetch"
        && body["content"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    {
        body["next_offset"].as_u64().and_then(|offset| {
            let mut next = serde_json::from_str::<Value>(arguments).ok()?;
            next["offset"] = json!(offset);
            next["refresh"] = json!(false);
            Some(ToolContinuation::NextCall {
                tool: tool.into(),
                arguments: next,
                reason:
                    "Read the next cached page segment only if it resolves a remaining evidence gap"
                        .into(),
            })
        })
    } else {
        None
    };
    truncated |= continuation.is_some();
    ToolResultDraft {
        status: if truncated {
            ToolResultStatus::Partial
        } else {
            ToolResultStatus::Ok
        },
        model_body: body,
        truncated,
        continuation,
        evidence_arguments: arguments.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_actual_provider_bounds_and_cached_read_arguments() {
        assert_eq!(
            parse_search(json!({"query":"为什么选择追加记录"}))
                .unwrap()
                .count,
            3
        );
        assert!(parse_search(json!({"query":"字".repeat(128)})).is_ok());
        for value in [
            json!({"query":"字".repeat(129)}),
            json!({"query":"  "}),
            json!({"query":"x","count":6}),
            json!({"query":"x","localization":"China"}),
        ] {
            assert!(parse_search(value).is_err());
        }
        assert_eq!(
            parse_fetch(json!({"url":"https://example.com/doc"}))
                .unwrap()
                .max_chars,
            6000
        );
        for value in [
            json!({"url":"file:///tmp/page"}),
            json!({"url":"https://user:pass@example.com"}),
            json!({"url":"https://example.com","max_chars":12001}),
        ] {
            assert!(parse_fetch(value).is_err());
        }
    }

    #[test]
    fn bounded_fetch_continues_exact_characters_without_book_evidence() {
        let content = "条件：只有满足前提才能推出结论。".repeat(500);
        let raw = json!({"source_scope":"web","url":"https://example.com/doc","title":"Mechanism","content_status":"available","content":content,"offset":100,"end_offset":100+content.chars().count(),"total_chars":100+content.chars().count(),"next_offset":null});
        let draft = project_result(
            "web.fetch",
            r#"{"url":"https://example.com/doc","offset":100}"#,
            &raw,
            1000,
        );
        assert!(json_bytes(&draft.model_body) <= 1000);
        assert_eq!(draft.model_body["url"], raw["url"]);
        assert_eq!(draft.model_body["source_scope"], "web");
        let text = draft.model_body["content"].as_str().unwrap();
        assert!(!text.is_empty());
        assert!(content.starts_with(text));
        assert_eq!(
            draft.model_body["next_offset"],
            json!(100 + text.chars().count())
        );
        assert!(draft.continuation.is_some());
        assert!(draft.model_body.get("lid").is_none());
    }

    #[test]
    fn search_budget_keeps_source_urls_and_marks_omitted_content() {
        let raw = json!({"source_scope":"web","query":"mechanism","results":[{"url":"https://example.com/doc","title":"Doc","summary":"summary ".repeat(100),"content":"text".repeat(10000),"content_status":"available","total_chars":40000}]});
        let draft = project_result(
            "web.search",
            r#"{"query":"mechanism","need_content":true}"#,
            &raw,
            1024,
        );
        assert!(json_bytes(&draft.model_body) <= 1024);
        assert_eq!(
            draft.model_body["results"][0]["url"],
            raw["results"][0]["url"]
        );
        assert_eq!(
            draft.model_body["results"][0]["content_status"],
            "summary_only"
        );
        assert_eq!(draft.model_body["results"][0]["next_offset"], 0);
        assert!(draft.truncated);
    }
}

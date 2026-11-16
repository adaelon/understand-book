use crate::tool_result::{ToolContinuation, ToolResultDraft, ToolResultStatus};
use serde_json::{json, Value};

fn size(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |v| v.len())
}
fn continuation(arguments: &Value, body: &Value) -> Option<ToolContinuation> {
    let offset = body["body"]["next_offset"].as_u64()?;
    let mut next = arguments.clone();
    next["offset"] = json!(offset);
    Some(ToolContinuation::ToolCursor {
        tool: "music.read".into(),
        arguments: next,
    })
}

/// Admit complete snapshots only. A later omitted source is never registered as
/// observed, and every returned citation retains its exact published target.
pub(crate) fn project_read(arguments: &str, raw: &Value, limit: usize) -> Option<ToolResultDraft> {
    let args: Value = serde_json::from_str(arguments).ok()?;
    if size(raw) <= limit {
        return Some(ToolResultDraft {
            status: ToolResultStatus::Ok,
            model_body: raw.clone(),
            truncated: false,
            continuation: continuation(&args, raw),
            evidence_arguments: arguments.into(),
        });
    }
    let offset = raw["body"]["offset"].as_u64()?;
    let total = raw["body"]["total"].as_u64()?;
    let mut selected = raw.clone();
    let mut accepted = None;
    if raw["body"]["view"] == "catalog" {
        let items = raw["body"]["items"].as_array()?;
        for count in 1..=items.len() {
            selected["body"]["items"] = json!(&items[..count]);
            selected["body"]["returned_count"] = json!(count);
            selected["body"]["next_offset"] =
                json!((offset + (count as u64) < total).then_some(offset + count as u64));
            if size(&selected) > limit {
                break;
            }
            accepted = Some((selected.clone(), count as u64));
        }
    } else if raw["sources"][0]["material"]["read"]["units"].is_array() {
        let read: read_tools::music::MusicReadResult = serde_json::from_value(raw.clone()).ok()?;
        let units = raw["sources"][0]["material"]["read"]["units"].as_array()?;
        for count in 1..=units.len() {
            let cropped = read_tools::music::crop_score_result(&read, count)?;
            let returned = cropped.body["returned_count"].as_u64()?;
            let value = serde_json::to_value(cropped).ok()?;
            if size(&value) > limit { break; }
            accepted = Some((value, returned));
        }
    } else {
        let sources = raw["sources"].as_array()?;
        let mut returned = 0;
        for (index, source) in sources.iter().enumerate() {
            returned += source["material"]["read"]["returned_count"].as_u64()?;
            selected["sources"] = json!(&sources[..=index]);
            selected["body"]["returned_count"] = json!(returned);
            selected["body"]["next_offset"] =
                json!((offset + returned < total).then_some(offset + returned));
            if size(&selected) > limit {
                break;
            }
            accepted = Some((selected.clone(), returned));
        }
    }
    let (body, returned) = accepted?;
    let mut evidence_args = args.clone();
    evidence_args["offset"] = json!(offset);
    evidence_args["limit"] = json!(returned);
    Some(ToolResultDraft {
        status: ToolResultStatus::Partial,
        continuation: continuation(&args, &body),
        model_body: body,
        truncated: true,
        evidence_arguments: evidence_args.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_paging_resumes_after_the_last_complete_entry() {
        let raw = json!({"body":{"view":"catalog","offset":3,"returned_count":2,"total":8,"next_offset":5,
            "items":[{"event_id":"a","title":"A".repeat(400)},{"event_id":"b","title":"B".repeat(400)}]},"sources":[]});
        let mut prefix = raw.clone();
        prefix["body"]["items"].as_array_mut().unwrap().truncate(1);
        prefix["body"]["returned_count"] = json!(1);
        prefix["body"]["next_offset"] = json!(4);
        let result = project_read(
            r#"{"view":"catalog","offset":3,"limit":2}"#,
            &raw,
            size(&prefix),
        )
        .unwrap();
        assert_eq!(result.model_body, prefix);
        assert!(result.truncated);
        assert_eq!(
            serde_json::from_str::<Value>(&result.evidence_arguments).unwrap()["limit"],
            1
        );
        assert!(
            project_read("{}", &raw, 10).is_none(),
            "Do not cut through an entry to make it fit"
        );
        let full = project_read("{}", &raw, size(&raw)).unwrap();
        assert!(!full.truncated);
        assert!(full.continuation.is_some());
    }
}

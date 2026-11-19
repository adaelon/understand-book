use serde_json::{json, Value};

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    reads_only: bool,
) -> Value {
    json!({
        "name":name, "description":description,
        "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
        "annotations":{"readOnlyHint":reads_only}
    })
}

pub(super) fn tools(write: bool, navigate: bool, closed_book: bool) -> Value {
    let string = json!({"type":"string","minLength":1});
    let offsets = json!({"type":"object","properties":{
        "start":{"type":"integer","minimum":0},"end":{"type":"integer","minimum":1}
    },"required":["start","end"],"additionalProperties":false});
    let at = json!({"type":"object","description":"Copy a source_ref from an actual read in the authorized book. Range offsets are paragraph-relative UTF-16 positions.",
        "properties":{"book_id":string,"source_revision":string,"lid":string,"range":offsets},
        "required":["book_id","source_revision","lid"],"additionalProperties":false});
    let mut tools = vec![
        tool(
            "material_list",
            "List the books shared with this personal connection.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "task_list",
            "Find unfinished learning tasks in the authorized books.",
            json!({"book_id":string}),
            &[],
            true,
        ),
    ];
    if !closed_book {
        tools.extend([
            tool("reader_scene","Read the current location in an authorized book.",json!({"book_id":string}),&["book_id"],true),
            tool("book_search_text","Locate literal text in one authorized book. Read the resulting paragraph for context.",json!({"book_id":string,"query":string,"cursor":string,"page_size":{"type":"integer","minimum":1,"maximum":50}}),&["book_id","query"],true),
            tool("book_text","Read a paragraph and get its versioned source_ref.",json!({"book_id":string,"lid":string,"range":offsets}),&["book_id","lid"],true),
            tool("book_paper_metadata","Read the prepared paper metadata and its evidence.",json!({"book_id":string}),&["book_id"],true),
            tool("note_list","List personal notes in one authorized book.",json!({"book_id":string,"before":string}),&["book_id"],true),
            tool("highlight_list","List saved highlights in one authorized book. Verified locations include a current source_ref.",json!({"book_id":string,"before":string}),&["book_id"],true),
            tool("note_read","Read a personal note, optionally at its recorded revision.",json!({"book_id":string,"note_id":string,"revision":{"type":"integer","minimum":1}}),&["book_id","note_id"],true),
        ]);
        if write {
            let op = json!({"type":"string","minLength":1,"description":"Choose a new identifier for each intentional write. Reuse it when retrying the same write."});
            tools.extend([
                tool("note_add","Save a personal note at an actual source location.",json!({"source_ref":at,"text":string,"operation_id":op}),&["source_ref","text","operation_id"],false),
                tool("highlight_add","Highlight a source selection.",json!({"source_ref":at,"operation_id":op}),&["source_ref","operation_id"],false),
                tool("task_save","Save the selected question to continue later in Reader.",json!({"source_ref":at,"text":string,"operation_id":op,"chat_session_id":string}),&["source_ref","text","operation_id"],false),
            ]);
        }
        if navigate {
            tools.extend([
                tool(
                    "reader_goto",
                    "Open a source at its original location.",
                    json!({"source_ref":at}),
                    &["source_ref"],
                    false,
                ),
                tool(
                    "reader_navigation_result",
                    "Check whether Reader displayed the requested sources.",
                    json!({"id":string}),
                    &["id"],
                    true,
                ),
                tool(
                    "reader_compare",
                    "Request two actual sources for comparison.",
                    json!({"source_ref":at,"other_source_ref":at}),
                    &["source_ref", "other_source_ref"],
                    false,
                ),
            ]);
        }
    }
    json!({"tools":tools})
}

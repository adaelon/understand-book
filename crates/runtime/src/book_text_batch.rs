//! Resident-only batching of known source locations; each item keeps the single-read contract.

use book_tool_contracts::{input_schema, validate_input, BookToolId, TextInput};
use read_tools::Book;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ToolSpec;

pub(crate) const MAX_RANGES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BatchTextInput {
    pub ranges: Vec<TextInput>,
}

pub(crate) fn spec() -> ToolSpec {
    ToolSpec {
        name: "book.text_batch".into(),
        description: "Read several known source locations together, preserving each range separately. Prefer one batch for multiple locations; use book.text for a single range. Only delivered text is evidence; unread continuations are not.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "ranges": {
                    "type": "array",
                    "description": "One to eight known source ranges, in requested order. Exact duplicate requests are read once.",
                    "minItems": 1,
                    "maxItems": MAX_RANGES,
                    "items": input_schema(BookToolId::Text)
                }
            },
            "required": ["ranges"],
            "additionalProperties": false
        }),
    }
}

pub(crate) fn parse(arguments: &str) -> Result<BatchTextInput, String> {
    let input: BatchTextInput =
        serde_json::from_str(arguments).map_err(|error| error.to_string())?;
    if !(1..=MAX_RANGES).contains(&input.ranges.len()) {
        return Err(format!(
            "ranges must contain between 1 and {MAX_RANGES} items"
        ));
    }

    let mut ranges = Vec::with_capacity(input.ranges.len());
    for (index, range) in input.ranges.into_iter().enumerate() {
        validate_input(BookToolId::Text, json!(range))
            .map_err(|error| format!("ranges[{index}]: {}", error.message))?;
        if !ranges.contains(&range) {
            ranges.push(range);
        }
    }
    Ok(BatchTextInput { ranges })
}

pub(crate) fn read(arguments: &str, book: &Book) -> String {
    let input = match parse(arguments) {
        Ok(input) => input,
        Err(message) => {
            return json!({
                "error_code": "BOOK_TOOL_INPUT_INVALID",
                "category": "validation",
                "message": message
            })
            .to_string();
        }
    };
    let items: Vec<Value> = input
        .ranges
        .into_iter()
        .map(|arguments| {
            let result = match book.read_text(&arguments) {
                Ok(result) => result,
                Err(error) => json!(error),
            };
            json!({"arguments": arguments, "result": result})
        })
        .collect();
    json!({"items": items}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_book() -> Book {
        Book::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../book-tool-contracts/fixtures/example-book"
        ))
        .unwrap()
    }

    #[test]
    fn parse_deduplicates_exact_ranges_in_first_seen_order() {
        let input = parse(
            &json!({"ranges": [
                {"lid": "1.3"},
                {"lid": "1.1"},
                {"lid": "1.3", "end_lid": null, "start_utf16": null},
                {"lid": "1.1", "end_lid": "1.2"},
                {"lid": "1.1", "start_utf16": 5}
            ]})
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            json!(input),
            json!({"ranges": [
                {"lid": "1.3"},
                {"lid": "1.1"},
                {"lid": "1.1", "end_lid": "1.2"},
                {"lid": "1.1", "start_utf16": 5}
            ]})
        );
    }

    #[test]
    fn parse_enforces_count_fields_and_shared_text_semantics() {
        let invalid = [
            json!({"ranges": []}),
            json!({"ranges": vec![json!({"lid": "1.1"}); MAX_RANGES + 1]}),
            json!({"ranges": [{"lid": "1.1"}], "other": true}),
            json!({"ranges": [{"lid": "1.1", "other": true}]}),
            json!({"ranges": [{"lid": " "}]}),
            json!({"ranges": [{"lid": "1.1", "end_lid": " "}]}),
            json!({"ranges": [{"lid": "1.1", "start_utf16": -1}]}),
        ];
        for input in invalid {
            assert!(parse(&input.to_string()).is_err(), "{input}");
        }
        assert!(parse("{").is_err());
        assert!(
            parse(&json!({"ranges": vec![json!({"lid": "1.1"}); MAX_RANGES]}).to_string()).is_ok()
        );
    }

    #[test]
    fn read_keeps_non_contiguous_ranges_separate_and_preserves_order() {
        let book = fixture_book();
        let arguments = json!({"ranges": [{"lid": "1.3"}, {"lid": "1.1"}, {"lid": "1.3"}]});
        let result: Value = serde_json::from_str(&read(&arguments.to_string(), &book)).unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["arguments"], json!({"lid": "1.3"}));
        assert_eq!(items[1]["arguments"], json!({"lid": "1.1"}));
        for item in items {
            let input: TextInput = serde_json::from_value(item["arguments"].clone()).unwrap();
            assert_eq!(item["result"], book.read_text(&input).unwrap());
            assert!(!item["result"]["text"]
                .as_str()
                .unwrap()
                .contains("beta threshold"));
        }
    }

    #[test]
    fn read_keeps_item_errors_without_losing_successful_reads() {
        let book = fixture_book();
        let arguments = json!({"ranges": [
            {"lid": "missing"},
            {"lid": "1.3", "end_lid": "1.1"},
            {"lid": "1.1"}
        ]});
        let result: Value = serde_json::from_str(&read(&arguments.to_string(), &book)).unwrap();
        assert_eq!(result["items"][0]["result"]["error_code"], "LID_NOT_FOUND");
        assert_eq!(result["items"][0]["result"]["category"], "not_found");
        assert_eq!(
            result["items"][1]["result"]["error_code"],
            "INVALID_SOURCE_RANGE"
        );
        assert!(result["items"][2]["result"]["text"]
            .as_str()
            .unwrap()
            .contains("alpha threshold"));
    }

    #[test]
    fn read_preserves_continuation_coordinates_and_validation_error_shape() {
        let book = fixture_book();
        let arguments = json!({"ranges": [{"lid": "1.1", "start_utf16": 4}, {"lid": "1.3"}]});
        let result: Value = serde_json::from_str(&read(&arguments.to_string(), &book)).unwrap();
        assert_eq!(result["items"][0]["result"]["source_range"]["start"], 4);
        assert_eq!(result["items"][0]["result"]["requested_range"]["start"], 0);
        assert!(result["items"][0]["result"]["next_start_utf16"].is_null());
        let invalid: Value = serde_json::from_str(&read("{\"ranges\":[]}", &book)).unwrap();
        assert_eq!(invalid["error_code"], "BOOK_TOOL_INPUT_INVALID");
        assert_eq!(invalid["category"], "validation");
        assert!(invalid.get("items").is_none());
    }
}

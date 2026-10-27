//! Per-turn UI language, independent of the language of source material and history.
use crate::context_fragment::{ContextFragment, FragmentScope, FragmentSensitivity};
use crate::Role;
use read_tools::ToolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseLanguage {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl ResponseLanguage {
    pub fn from_request(input: &serde_json::Value) -> Result<Option<Self>, ToolError> {
        input.get("response_language").filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()).map_err(|_| ToolError {
                error_code: "INVALID_RESPONSE_LANGUAGE".into(), category: "validation".into(),
                message: "response_language must be en or zh-CN".into(),
            })).transpose()
    }

    pub fn fragment(self) -> ContextFragment {
        let language = match self { Self::English => "English", Self::SimplifiedChinese => "Simplified Chinese" };
        ContextFragment::new("agent.response_language", FragmentScope::TurnFrozen, Role::System,
            format!("The reader selected {language} for this turn. Write all user-facing responses, progress explanations, teaching feedback, questions, suggested choices, titles, and generated presentation text in {language}. This applies even when the source material, user question, prior conversation, or tool results use another language. Preserve original quotations, code, mathematical notation, proper names, source identifiers, and protocol keys. An explicitly requested translation uses its requested target language. This turn's selection supersedes any language preference in earlier history."),
            FragmentSensitivity::Public)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_language_is_explicit_and_validated() {
        assert_eq!(ResponseLanguage::from_request(&json!({})).unwrap(), None);
        assert_eq!(ResponseLanguage::from_request(&json!({"response_language":"en"})).unwrap(), Some(ResponseLanguage::English));
        assert_eq!(ResponseLanguage::from_request(&json!({"response_language":"zh-CN"})).unwrap(), Some(ResponseLanguage::SimplifiedChinese));
        for value in [json!("fr"), json!(true), json!({"language":"en"})] {
            assert_eq!(ResponseLanguage::from_request(&json!({"response_language":value})).unwrap_err().error_code, "INVALID_RESPONSE_LANGUAGE");
        }
    }

    #[test]
    fn language_instruction_covers_generated_ui_and_preserves_sources() {
        let english = ResponseLanguage::English.fragment();
        assert_eq!(english.scope, FragmentScope::TurnFrozen);
        assert_eq!(english.role, Role::System);
        assert!(english.content.contains("generated presentation text in English"));
        assert!(english.content.contains("Preserve original quotations"));
        assert_ne!(english.content, ResponseLanguage::SimplifiedChinese.fragment().content);
    }
}

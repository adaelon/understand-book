//! ADM4 immutable pricing inputs. Rates are operator-supplied, never inferred
//! from payments. Only a Provider usage snapshot can establish a usage cost.
use crate::account_allowance::MicroCny;
use runtime::{model_spend::SpendStop, provider_stream::ModelUsage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RateSource {
    pub currency: String,
    /// Exact decimal/rational source quotes and CNY conversion, retained verbatim.
    pub original_prices: String,
    pub cny_conversion: String,
    pub checked_on: String,
    pub official_url: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageContract {
    DeepSeek,
    /// All input tokens have the same price; cache subdivision is not required.
    InputOutput,
    /// Total input includes the separately priced read/write cache buckets.
    CachedInputOutput,
    /// Cohere's billed token units, independent of its context token counts.
    CohereBilledUnits,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageMeter {
    Unsupported,
    DeepSeek1024,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelRateSnapshot {
    pub version: String,
    pub provider: String,
    pub model: String,
    /// Absolute UTC intervals deliberately cover peak/off-peak, weekdays and
    /// holiday overrides without guessing a tariff calendar. No match rejects.
    pub starts_at: i64,
    pub expires_at: i64,
    pub input_micro_cny_per_million: u64,
    pub cached_input_micro_cny_per_million: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_micro_cny_per_million: Option<u64>,
    pub output_micro_cny_per_million: u64,
    pub usage_contract: UsageContract,
    pub image_meter: ImageMeter,
    pub source: RateSource,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRates {
    pub rates: Vec<ModelRateSnapshot>,
}
impl ModelRates {
    pub fn load(path: &std::path::Path) -> Result<Self, SpendStop> {
        Self::load_diagnostic(path).map_err(|_| SpendStop::RateUnavailable)
    }
    pub(crate) fn load_diagnostic(path: &std::path::Path) -> Result<Self, &'static str> {
        let bytes = std::fs::read(path).map_err(|_| "MODEL_RATE_FILE_UNREADABLE")?;
        let rates: Self =
            serde_json::from_slice(&bytes).map_err(|_| "MODEL_RATE_FILE_INVALID_JSON")?;
        rates
            .validate()
            .map_err(|_| "MODEL_RATE_CONFIGURATION_INVALID")?;
        Ok(rates)
    }
    pub fn validate(&self) -> Result<(), SpendStop> {
        if self.rates.is_empty() {
            return Err(SpendStop::RateUnavailable);
        }
        for (i, r) in self.rates.iter().enumerate() {
            if r.starts_at >= r.expires_at
                || [
                    r.version.as_str(),
                    r.provider.as_str(),
                    r.model.as_str(),
                    &r.source.currency,
                    &r.source.original_prices,
                    &r.source.cny_conversion,
                    &r.source.checked_on,
                    &r.source.official_url,
                ]
                .iter()
                .any(|s| s.trim().is_empty())
            {
                return Err(SpendStop::RateUnavailable);
            }
            if r.usage_contract == UsageContract::InputOutput
                && (r.cached_input_micro_cny_per_million != r.input_micro_cny_per_million
                    || r.cache_creation_input_micro_cny_per_million
                        .is_some_and(|price| price != r.input_micro_cny_per_million))
            {
                return Err(SpendStop::RateUnavailable);
            }
            if self.rates[..i].iter().any(|p| {
                p.version == r.version
                    || (p.provider == r.provider
                        && p.model == r.model
                        && p.starts_at < r.expires_at
                        && r.starts_at < p.expires_at)
            }) {
                return Err(SpendStop::RateUnavailable);
            }
        }
        Ok(())
    }
    pub fn select(
        &self,
        provider: &str,
        model: &str,
        now: i64,
    ) -> Result<ModelRateSnapshot, SpendStop> {
        self.validate()?;
        self.rates
            .iter()
            .find(|r| {
                r.provider == provider
                    && r.model == model
                    && r.starts_at <= now
                    && now < r.expires_at
            })
            .cloned()
            .ok_or(SpendStop::RateUnavailable)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BillableUsage {
    pub uncached_input_tokens: u64,
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnknownUsage {
    MissingInput,
    MissingOutput,
    MissingCache,
    MissingCacheCreation,
    MissingBilledInput,
    MissingBilledOutput,
    UnsupportedCacheCreation,
    InconsistentBreakdown,
    CostOverflow,
}
impl ModelRateSnapshot {
    pub fn normalize(&self, usage: &ModelUsage) -> Result<BillableUsage, UnknownUsage> {
        if self.usage_contract == UsageContract::CohereBilledUnits {
            return Ok(BillableUsage {
                uncached_input_tokens: u64::from(
                    usage
                        .billed_input_tokens
                        .ok_or(UnknownUsage::MissingBilledInput)?,
                ),
                cached_input_tokens: 0,
                cache_creation_input_tokens: 0,
                output_tokens: u64::from(
                    usage
                        .billed_output_tokens
                        .ok_or(UnknownUsage::MissingBilledOutput)?,
                ),
            });
        }
        let input = usage.input_tokens.ok_or(UnknownUsage::MissingInput)?;
        let output = usage.output_tokens.ok_or(UnknownUsage::MissingOutput)?;
        if usage.reasoning_output_tokens.is_some_and(|v| v > output)
            || usage
                .total_tokens
                .is_some_and(|v| u64::from(v) != u64::from(input) + u64::from(output))
        {
            return Err(UnknownUsage::InconsistentBreakdown);
        }
        if self.usage_contract == UsageContract::InputOutput {
            if usage.cached_input_tokens.is_some_and(|v| v > input)
                || usage.cache_creation_input_tokens.is_some_and(|v| v > input)
                || usage
                    .cached_input_tokens
                    .zip(usage.cache_creation_input_tokens)
                    .is_some_and(|(read, write)| {
                        u64::from(read) + u64::from(write) > u64::from(input)
                    })
            {
                return Err(UnknownUsage::InconsistentBreakdown);
            }
            // This is a price bucket, not an invented cache-usage observation.
            // The chosen contract charges every input token at the same rate.
            return Ok(BillableUsage {
                uncached_input_tokens: u64::from(input),
                cached_input_tokens: 0,
                cache_creation_input_tokens: 0,
                output_tokens: u64::from(output),
            });
        }
        let cached = usage
            .cached_input_tokens
            .ok_or(UnknownUsage::MissingCache)?;
        let created = if self.usage_contract == UsageContract::CachedInputOutput
            && self.cache_creation_input_micro_cny_per_million.is_some()
        {
            usage
                .cache_creation_input_tokens
                .ok_or(UnknownUsage::MissingCacheCreation)?
        } else {
            if usage.cache_creation_input_tokens.is_some_and(|v| v > 0) {
                return Err(UnknownUsage::UnsupportedCacheCreation);
            }
            0
        };
        if u64::from(cached) + u64::from(created) > u64::from(input) {
            return Err(UnknownUsage::InconsistentBreakdown);
        }
        Ok(BillableUsage {
            uncached_input_tokens: u64::from(input - cached - created),
            cached_input_tokens: u64::from(cached),
            cache_creation_input_tokens: u64::from(created),
            output_tokens: u64::from(output),
        })
    }
    pub fn cost(&self, usage: &ModelUsage) -> Result<MicroCny, UnknownUsage> {
        self.price(&self.normalize(usage)?)
    }
    fn price(&self, usage: &BillableUsage) -> Result<MicroCny, UnknownUsage> {
        let numerator = [
            (
                usage.uncached_input_tokens,
                self.input_micro_cny_per_million,
            ),
            (
                usage.cached_input_tokens,
                self.cached_input_micro_cny_per_million,
            ),
            (
                usage.cache_creation_input_tokens,
                self.cache_creation_input_micro_cny_per_million
                    .unwrap_or(self.input_micro_cny_per_million),
            ),
            (usage.output_tokens, self.output_micro_cny_per_million),
        ]
        .into_iter()
        .try_fold(0u128, |sum, (tokens, rate)| {
            sum.checked_add(u128::from(tokens) * u128::from(rate))
        })
        .ok_or(UnknownUsage::CostOverflow)?;
        let rounded = numerator / 1_000_000 + u128::from(numerator % 1_000_000 != 0);
        i64::try_from(rounded)
            .map(MicroCny::new)
            .map_err(|_| UnknownUsage::CostOverflow)
    }
    pub fn estimate(&self, request: &Value) -> Result<ReservationEstimate, SpendStop> {
        let model = request["model"]
            .as_str()
            .ok_or(SpendStop::RateUnavailable)?;
        self.estimate_for_model(model, request)
    }

    /// Models carried by a route (Gemini/Bedrock) use the frozen send identity.
    /// A body model, when present, must still agree with that same identity.
    pub fn estimate_for_model(
        &self,
        model: &str,
        request: &Value,
    ) -> Result<ReservationEstimate, SpendStop> {
        if model != self.model
            || request
                .get("model")
                .is_some_and(|value| value.as_str() != Some(model))
        {
            return Err(SpendStop::RateUnavailable);
        }
        let (output, mut payload, legacy_messages) = if request.get("max_tokens").is_some()
            || request.get("max_completion_tokens").is_some()
        {
            if !request["messages"].is_array() {
                return Err(SpendStop::RateUnavailable);
            }
            (
                request
                    .get("max_tokens")
                    .or_else(|| request.get("max_completion_tokens"))
                    .unwrap(),
                json!({"system":request.get("system"),"messages":request["messages"],"tools":request.get("tools"),"response_format":request.get("response_format")}),
                true,
            )
        } else if request.get("max_output_tokens").is_some() {
            if !(request["input"].is_array() || request["input"].is_string()) {
                return Err(SpendStop::RateUnavailable);
            }
            (
                &request["max_output_tokens"],
                json!({"instructions":request.get("instructions"),"input":request["input"],"tools":request.get("tools"),"text":request.get("text")}),
                false,
            )
        } else if request.get("generationConfig").is_some() {
            if !request["contents"].is_array() {
                return Err(SpendStop::RateUnavailable);
            }
            (
                &request["generationConfig"]["maxOutputTokens"],
                json!({"systemInstruction":request.get("systemInstruction"),"contents":request["contents"],"tools":request.get("tools"),"generationConfig":request["generationConfig"]}),
                false,
            )
        } else if request.get("generation_config").is_some() {
            if !(request["input"].is_array() || request["input"].is_string()) {
                return Err(SpendStop::RateUnavailable);
            }
            (
                &request["generation_config"]["max_output_tokens"],
                json!({"system_instruction":request.get("system_instruction"),"input":request["input"],"tools":request.get("tools"),"generation_config":request["generation_config"],"response_format":request.get("response_format")}),
                false,
            )
        } else if request.get("inferenceConfig").is_some() {
            if !request["messages"].is_array() {
                return Err(SpendStop::RateUnavailable);
            }
            (
                &request["inferenceConfig"]["maxTokens"],
                json!({"system":request.get("system"),"messages":request["messages"],"toolConfig":request.get("toolConfig"),"outputConfig":request.get("outputConfig")}),
                false,
            )
        } else if request.get("options").is_some() {
            if !request["messages"].is_array() {
                return Err(SpendStop::RateUnavailable);
            }
            (
                &request["options"]["num_predict"],
                json!({"messages":request["messages"],"tools":request.get("tools"),"format":request.get("format")}),
                false,
            )
        } else {
            return Err(SpendStop::RateUnavailable);
        };
        let output = output
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or(SpendStop::RateUnavailable)?;
        let mut images = 0u64;
        let image_meter = legacy_messages
            && self.usage_contract == UsageContract::DeepSeek
            && self.image_meter == ImageMeter::DeepSeek1024;
        // Inspect actual content roots, not tools' arbitrary JSON Schema fields.
        for root in [
            "messages",
            "contents",
            "input",
            "system",
            "systemInstruction",
        ] {
            if let Some(value) = payload.get_mut(root) {
                scrub_content_media(value, image_meter, &mut images)?;
            }
        }
        // Same character heuristic as Runtime, expressed in quarter-token integer
        // units. Include the final tool schemas, protocol and reasoning context.
        let payload = payload.to_string();
        let quarters = payload
            .chars()
            .try_fold(0u64, |n, c| {
                n.checked_add(if c as u32 >= 0x2e80 { 4 } else { 1 })
            })
            .ok_or(SpendStop::RateUnavailable)?;
        let text_tokens = quarters / 4 + u64::from(quarters % 4 != 0);
        let image_tokens = images.checked_mul(1024).ok_or(SpendStop::RateUnavailable)?;
        let input = text_tokens
            .checked_add(image_tokens)
            .ok_or(SpendStop::RateUnavailable)?;
        let mut estimated_usage = BillableUsage {
            uncached_input_tokens: input,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            output_tokens: output,
        };
        // Cache creation can be more expensive than ordinary input. Reserve at
        // the highest configured input bucket; settle only the reported split.
        if self.usage_contract == UsageContract::CachedInputOutput {
            let created_rate = self.cache_creation_input_micro_cny_per_million.unwrap_or(0);
            if created_rate > self.input_micro_cny_per_million
                && created_rate >= self.cached_input_micro_cny_per_million
            {
                estimated_usage.uncached_input_tokens = 0;
                estimated_usage.cache_creation_input_tokens = input;
            } else if self.cached_input_micro_cny_per_million > self.input_micro_cny_per_million {
                estimated_usage.uncached_input_tokens = 0;
                estimated_usage.cached_input_tokens = input;
            }
        }
        let amount = self
            .price(&estimated_usage)
            .map_err(|_| SpendStop::RateUnavailable)?;
        Ok(ReservationEstimate {
            reserved_micro_cny: amount,
            text_tokens,
            image_tokens,
            output_tokens: output,
            source: if self.usage_contract == UsageContract::CachedInputOutput {
                "final_request_character_estimate_max_input_rate_v1"
            } else {
                "final_request_character_estimate_uncached_v1"
            }
            .into(),
            image_meter: self.image_meter,
        })
    }
}

/// Strip only a declared, priced image format. Other media lacks a tariff
/// contract and is rejected before sending; base64 is never priced as text.
fn scrub_content_media(
    value: &mut Value,
    deepseek_images: bool,
    images: &mut u64,
) -> Result<(), SpendStop> {
    if let Some(values) = value.as_array_mut() {
        for value in values {
            scrub_content_media(value, deepseek_images, images)?;
        }
        return Ok(());
    }
    if !value.is_object() {
        return Ok(());
    }
    match value["type"].as_str() {
        Some("image_url" | "image" | "input_image") => {
            if !deepseek_images || value["type"] == "input_image" {
                return Err(SpendStop::RateUnavailable);
            }
            *images = images.checked_add(1).ok_or(SpendStop::RateUnavailable)?;
            *value = json!({"type":value["type"]});
            return Ok(());
        }
        Some("audio" | "video" | "input_audio" | "input_video" | "input_file") => {
            return Err(SpendStop::RateUnavailable)
        }
        _ => (),
    }
    if value
        .get("images")
        .is_some_and(|images| images.as_array().is_some_and(|images| !images.is_empty()))
        || [
            "inlineData",
            "fileData",
            "inline_data",
            "file_data",
            "image",
            "audio",
            "video",
        ]
        .iter()
        .any(|key| value.get(*key).is_some_and(|value| !value.is_null()))
    {
        return Err(SpendStop::RateUnavailable);
    }
    for field in ["content", "parts"] {
        if let Some(content) = value.get_mut(field) {
            scrub_content_media(content, deepseek_images, images)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservationEstimate {
    pub reserved_micro_cny: MicroCny,
    pub text_tokens: u64,
    pub image_tokens: u64,
    pub output_tokens: u64,
    pub source: String,
    pub image_meter: ImageMeter,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rate() -> ModelRateSnapshot {
        ModelRateSnapshot {
            version: "test-peak".into(),
            provider: "fixture".into(),
            model: "fixture".into(),
            starts_at: 10,
            expires_at: 20,
            input_micro_cny_per_million: 200_000,
            cached_input_micro_cny_per_million: 20_000,
            cache_creation_input_micro_cny_per_million: None,
            output_micro_cny_per_million: 400_000,
            usage_contract: UsageContract::DeepSeek,
            image_meter: ImageMeter::DeepSeek1024,
            source: RateSource {
                currency: "CNY".into(),
                original_prices: "fixture only".into(),
                cny_conversion: "1/1".into(),
                checked_on: "2026-10-08".into(),
                official_url: "https://api-docs.deepseek.com/quick_start/pricing/".into(),
            },
        }
    }
    #[test]
    fn adm4_price_integer_rounding_cache_and_reasoning() {
        let r = rate();
        let usage = ModelUsage {
            input_tokens: Some(100),
            cached_input_tokens: Some(80),
            output_tokens: Some(30),
            reasoning_output_tokens: Some(13),
            total_tokens: Some(130),
            ..Default::default()
        };
        assert_eq!(r.cost(&usage).unwrap().value(), 18); // 4 + 1.6 + 12 -> ceil once
        let mut fractional = r.clone();
        fractional.input_micro_cny_per_million = 1;
        fractional.cached_input_micro_cny_per_million = 1;
        fractional.output_micro_cny_per_million = 1;
        assert_eq!(fractional.cost(&usage).unwrap().value(), 1);
        for (u, expected) in [
            (
                ModelUsage {
                    total_tokens: Some(17),
                    ..Default::default()
                },
                UnknownUsage::MissingInput,
            ),
            (
                ModelUsage {
                    cached_input_tokens: None,
                    ..usage.clone()
                },
                UnknownUsage::MissingCache,
            ),
            (
                ModelUsage {
                    cached_input_tokens: Some(101),
                    ..usage.clone()
                },
                UnknownUsage::InconsistentBreakdown,
            ),
            (
                ModelUsage {
                    cache_creation_input_tokens: Some(2),
                    ..usage.clone()
                },
                UnknownUsage::UnsupportedCacheCreation,
            ),
        ] {
            assert_eq!(r.cost(&u), Err(expected));
        }
        let mut huge = r;
        huge.output_micro_cny_per_million = u64::MAX;
        assert_eq!(
            huge.cost(&ModelUsage {
                input_tokens: Some(0),
                cached_input_tokens: Some(0),
                output_tokens: Some(u32::MAX),
                ..Default::default()
            }),
            Err(UnknownUsage::CostOverflow)
        );
    }
    #[test]
    fn adm4_rate_time_boundaries_snapshot_and_configuration_failures() {
        let mut next = rate();
        next.version = "test-offpeak".into();
        next.starts_at = 20;
        next.expires_at = 30;
        next.output_micro_cny_per_million = 1;
        let mut rates = ModelRates {
            rates: vec![rate(), next],
        };
        let frozen = rates.select("fixture", "fixture", 19).unwrap();
        assert_eq!(
            rates.select("fixture", "fixture", 20).unwrap().version,
            "test-offpeak"
        );
        assert!(rates.select("fixture", "fixture", 30).is_err());
        assert!(rates.select("fixture", "other", 15).is_err());
        rates.rates[0].output_micro_cny_per_million = 9;
        assert_eq!(frozen.output_micro_cny_per_million, 400_000);
        rates.rates[1].starts_at = 19;
        assert!(rates.validate().is_err());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("rates.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&ModelRates {
                rates: vec![rate()],
            })
            .unwrap(),
        )
        .unwrap();
        assert!(ModelRates::load(&path).is_ok());
        std::fs::write(&path, b"{}").unwrap();
        assert!(ModelRates::load(&path).is_err());
    }
    #[test]
    fn adm4_final_body_estimate_includes_tools_images_and_output_limit() {
        let r = rate();
        let mut request = json!({"model":"fixture","max_tokens":500,"messages":[{"role":"user","content":"中文 test"}]});
        let text = r.estimate(&request).unwrap();
        request["tools"] = json!([{"description":"a long schema description".repeat(20)}]);
        assert!(r.estimate(&request).unwrap().text_tokens > text.text_tokens);
        request["messages"][0]["content"] = json!([{"type":"text","text":"inspect"},{"type":"image_url","image_url":{"url":"data:image/png;base64,not-charged-as-text"}}]);
        let image = r.estimate(&request).unwrap();
        assert_eq!(image.image_tokens, 1024);
        assert_eq!(image.output_tokens, 500);
        request["messages"][0]["content"][1]["image_url"]["url"] = json!("x".repeat(10000));
        assert_eq!(
            r.estimate(&request).unwrap().reserved_micro_cny,
            image.reserved_micro_cny
        );
        let mut unsupported = r.clone();
        unsupported.image_meter = ImageMeter::Unsupported;
        assert!(unsupported.estimate(&request).is_err());
        request.as_object_mut().unwrap().remove("max_tokens");
        assert!(r.estimate(&request).is_err());
    }

    #[test]
    fn messages_continuation_reserves_reasoning_tools_system_and_native_images() {
        let r = rate();
        let mut request = json!({"model":"fixture","max_tokens":500,"messages":[
            {"role":"user","content":[{"type":"text","text":"read"}]},
            {"role":"assistant","content":[
                {"type":"thinking","thinking":"plan","signature":"opaque"},
                {"type":"redacted_thinking","data":"opaque"},
                {"type":"tool_use","id":"call1","name":"read","input":{"path":"chapter.md"}}
            ]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"call1","content":"chapter text"}]},
            {"role":"system","content":[
                {"type":"tool_addition","tool":{"type":"tool_reference","name":"search"}},
                {"type":"tool_removal","tool":{"type":"tool_reference","name":"read"}}
            ]}
        ]});
        let history = r.estimate(&request).unwrap();
        request["system"] = json!("system instruction ".repeat(100));
        let system = r.estimate(&request).unwrap();
        assert!(system.text_tokens > history.text_tokens);
        request["messages"][1]["content"][0]["thinking"] = json!("retained reasoning ".repeat(100));
        assert!(r.estimate(&request).unwrap().text_tokens > system.text_tokens);
        request["messages"][0]["content"].as_array_mut().unwrap().push(json!({"type":"image","source":{"type":"base64","media_type":"image/png","data":"a"}}));
        let image = r.estimate(&request).unwrap();
        assert_eq!(image.image_tokens, 1024);
        request["messages"][0]["content"][1]["source"]["data"] = json!("a".repeat(10000));
        assert_eq!(
            r.estimate(&request).unwrap().reserved_micro_cny,
            image.reserved_micro_cny
        );
        let mut unsupported = r.clone();
        unsupported.image_meter = ImageMeter::Unsupported;
        assert!(unsupported.estimate(&request).is_err());
    }

    #[test]
    fn provider_rates_uniform_input_does_not_require_cache_but_requires_complete_counts() {
        let mut rate = rate();
        rate.usage_contract = UsageContract::InputOutput;
        rate.cached_input_micro_cny_per_million = rate.input_micro_cny_per_million;
        let usage = ModelUsage {
            input_tokens: Some(100),
            output_tokens: Some(30),
            reasoning_output_tokens: Some(10),
            total_tokens: Some(130),
            ..Default::default()
        };
        assert_eq!(rate.cost(&usage).unwrap().value(), 32);
        assert_eq!(
            rate.cost(&ModelUsage {
                input_tokens: None,
                ..usage.clone()
            }),
            Err(UnknownUsage::MissingInput)
        );
        assert_eq!(
            rate.cost(&ModelUsage {
                output_tokens: None,
                ..usage.clone()
            }),
            Err(UnknownUsage::MissingOutput)
        );
        assert_eq!(
            rate.cost(&ModelUsage {
                total_tokens: Some(140),
                ..usage
            }),
            Err(UnknownUsage::InconsistentBreakdown)
        );
        assert!(ModelRates {
            rates: vec![rate.clone()]
        }
        .validate()
        .is_ok());
        rate.cached_input_micro_cny_per_million = 1;
        assert!(ModelRates { rates: vec![rate] }.validate().is_err());
    }

    #[test]
    fn provider_rates_cache_creation_is_priced_once_and_unknown_stays_pending() {
        let mut rate = rate();
        rate.usage_contract = UsageContract::CachedInputOutput;
        rate.cache_creation_input_micro_cny_per_million = Some(400_000);
        let usage = ModelUsage {
            input_tokens: Some(100),
            cached_input_tokens: Some(50),
            cache_creation_input_tokens: Some(20),
            output_tokens: Some(10),
            total_tokens: Some(110),
            ..Default::default()
        };
        // 30 ordinary input + 50 cache reads + 20 cache writes + 10 output.
        assert_eq!(rate.cost(&usage).unwrap().value(), 19);
        assert_eq!(
            rate.cost(&ModelUsage {
                cache_creation_input_tokens: None,
                ..usage.clone()
            }),
            Err(UnknownUsage::MissingCacheCreation)
        );
        assert_eq!(
            rate.cost(&ModelUsage {
                cache_creation_input_tokens: Some(51),
                ..usage
            }),
            Err(UnknownUsage::InconsistentBreakdown)
        );
        let reservation = rate.estimate(&json!({"model":"fixture","max_tokens":10,"messages":[{"role":"user","content":"hello"}]})).unwrap();
        let expected = (reservation.text_tokens * 400_000 + 10 * 400_000).div_ceil(1_000_000);
        assert_eq!(reservation.reserved_micro_cny.value(), expected as i64);
    }

    #[test]
    fn provider_rates_cohere_billed_units_do_not_use_context_tokens() {
        let mut rate = rate();
        rate.usage_contract = UsageContract::CohereBilledUnits;
        let usage = ModelUsage {
            input_tokens: Some(100),
            output_tokens: Some(30),
            total_tokens: Some(130),
            billed_input_tokens: Some(50),
            billed_output_tokens: Some(20),
            ..Default::default()
        };
        assert_eq!(rate.cost(&usage).unwrap().value(), 18);
        assert_eq!(
            rate.cost(&ModelUsage {
                billed_input_tokens: None,
                ..usage.clone()
            }),
            Err(UnknownUsage::MissingBilledInput)
        );
        assert_eq!(
            rate.cost(&ModelUsage {
                billed_output_tokens: None,
                ..usage.clone()
            }),
            Err(UnknownUsage::MissingBilledOutput)
        );
        assert_eq!(
            rate.cost(&ModelUsage {
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                ..usage
            })
            .unwrap()
            .value(),
            18
        );
    }

    #[test]
    fn provider_rates_all_wire_budgets_use_the_frozen_model_and_include_tools() {
        let rate = rate();
        let requests = [
            json!({"model":"fixture","max_tokens":80,"messages":[{"role":"user","content":"hello"}]}),
            json!({"model":"fixture","max_output_tokens":80,"input":[{"role":"user","content":[{"type":"input_text","text":"hello"}]}]}),
            json!({"generationConfig":{"maxOutputTokens":80},"contents":[{"role":"user","parts":[{"text":"hello"}]}]}),
            json!({"model":"fixture","generation_config":{"max_output_tokens":80},"input":[{"role":"user","content":[{"type":"text","text":"hello"}]}]}),
            json!({"model":"fixture","options":{"num_predict":80},"messages":[{"role":"user","content":"hello"}]}),
            json!({"inferenceConfig":{"maxTokens":80},"messages":[{"role":"user","content":[{"text":"hello"}]}]}),
        ];
        for mut request in requests {
            let estimate = rate.estimate_for_model("fixture", &request).unwrap();
            assert_eq!(estimate.output_tokens, 80);
            assert!(estimate.text_tokens > 0);
            let tools_key = if request.get("inferenceConfig").is_some() {
                "toolConfig"
            } else {
                "tools"
            };
            request[tools_key] = json!([{"description":"long tool schema ".repeat(100)}]);
            assert!(
                rate.estimate_for_model("fixture", &request)
                    .unwrap()
                    .text_tokens
                    > estimate.text_tokens
            );
            assert_eq!(
                rate.estimate_for_model("other", &request).unwrap_err(),
                SpendStop::RateUnavailable
            );
            request["model"] = json!("wrong-body-model");
            assert_eq!(
                rate.estimate_for_model("fixture", &request).unwrap_err(),
                SpendStop::RateUnavailable
            );
        }
    }

    #[test]
    fn provider_rates_new_protocol_media_is_not_estimated_as_base64_text() {
        let rate = rate();
        for request in [
            json!({"max_output_tokens":80,"input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/png;base64,YQ=="}]}]}),
            json!({"generationConfig":{"maxOutputTokens":80},"contents":[{"parts":[{"inlineData":{"mimeType":"image/png","data":"YQ=="}}]}]}),
            json!({"generation_config":{"max_output_tokens":80},"input":[{"type":"image","data":"YQ==","mime_type":"image/png"}]}),
            json!({"options":{"num_predict":80},"messages":[{"role":"user","content":"image","images":["YQ=="]}]}),
            json!({"inferenceConfig":{"maxTokens":80},"messages":[{"role":"user","content":[{"image":{"format":"png","source":{"bytes":"YQ=="}}}]}]}),
        ] {
            assert_eq!(
                rate.estimate_for_model("fixture", &request).unwrap_err(),
                SpendStop::RateUnavailable
            );
        }
        let mut new_contract = rate;
        new_contract.usage_contract = UsageContract::InputOutput;
        let chat_image = json!({"max_tokens":80,"messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,YQ=="}}]}]});
        assert_eq!(
            new_contract
                .estimate_for_model("fixture", &chat_image)
                .unwrap_err(),
            SpendStop::RateUnavailable
        );
    }
}

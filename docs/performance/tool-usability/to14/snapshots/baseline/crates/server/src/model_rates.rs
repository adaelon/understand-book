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
        let bytes = std::fs::read(path).map_err(|_| SpendStop::RateUnavailable)?;
        let rates: Self = serde_json::from_slice(&bytes).map_err(|_| SpendStop::RateUnavailable)?;
        rates.validate()?;
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
    pub output_tokens: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnknownUsage {
    MissingInput,
    MissingOutput,
    MissingCache,
    UnsupportedCacheCreation,
    InconsistentBreakdown,
    CostOverflow,
}
impl ModelRateSnapshot {
    pub fn normalize(&self, usage: &ModelUsage) -> Result<BillableUsage, UnknownUsage> {
        let input = usage.input_tokens.ok_or(UnknownUsage::MissingInput)?;
        let output = usage.output_tokens.ok_or(UnknownUsage::MissingOutput)?;
        let cached = usage
            .cached_input_tokens
            .ok_or(UnknownUsage::MissingCache)?;
        if usage.cache_creation_input_tokens.is_some_and(|v| v > 0) {
            return Err(UnknownUsage::UnsupportedCacheCreation);
        }
        if cached > input
            || usage.reasoning_output_tokens.is_some_and(|v| v > output)
            || usage
                .total_tokens
                .is_some_and(|v| u64::from(v) != u64::from(input) + u64::from(output))
        {
            return Err(UnknownUsage::InconsistentBreakdown);
        }
        Ok(BillableUsage {
            uncached_input_tokens: u64::from(input - cached),
            cached_input_tokens: u64::from(cached),
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
        if request["model"].as_str() != Some(self.model.as_str()) {
            return Err(SpendStop::RateUnavailable);
        }
        let output = request
            .get("max_tokens")
            .or_else(|| request.get("max_completion_tokens"))
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
            .ok_or(SpendStop::RateUnavailable)?;
        let mut messages = request["messages"]
            .as_array()
            .cloned()
            .ok_or(SpendStop::RateUnavailable)?;
        let mut images = 0u64;
        for message in &mut messages {
            if let Some(parts) = message["content"].as_array_mut() {
                for part in parts {
                    match part["type"].as_str() {
                        Some("image_url") => {
                            if self.image_meter != ImageMeter::DeepSeek1024 {
                                return Err(SpendStop::RateUnavailable);
                            }
                            images = images.checked_add(1).ok_or(SpendStop::RateUnavailable)?;
                            *part = json!({"type":"image_url"});
                        }
                        Some("text") => (),
                        _ => return Err(SpendStop::RateUnavailable),
                    }
                }
            }
        }
        // Same character heuristic as Runtime, expressed in quarter-token integer
        // units. Include the final tool schemas, protocol and reasoning context.
        let payload=json!({"messages":messages,"tools":request.get("tools"),"response_format":request.get("response_format")}).to_string();
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
        let amount = self
            .price(&BillableUsage {
                uncached_input_tokens: input,
                cached_input_tokens: 0,
                output_tokens: output,
            })
            .map_err(|_| SpendStop::RateUnavailable)?;
        Ok(ReservationEstimate {
            reserved_micro_cny: amount,
            text_tokens,
            image_tokens,
            output_tokens: output,
            source: "final_request_character_estimate_uncached_v1".into(),
            image_meter: self.image_meter,
        })
    }
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
}

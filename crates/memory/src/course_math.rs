//! Authored mathematics experiments. These observations are not mastery evidence.
use super::{decimal, invalid};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum CourseMathState {
    ConvexChord {
        function: CourseMathFunction,
        #[serde(deserialize_with = "decimal")] a: f64,
        #[serde(deserialize_with = "decimal")] b: f64,
        #[serde(deserialize_with = "decimal")] t: f64,
    },
    GradientDescent {
        #[serde(deserialize_with = "decimal")] step_size: f64,
        steps: u32,
    },
    FeasibleDirections {
        #[serde(deserialize_with = "decimal")] position: f64,
        #[serde(deserialize_with = "decimal")] angle: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseMathFunction { Square, Cube }

impl CourseMathState {
    pub fn validate(&self) -> Result<(), ToolError> {
        let valid = match self {
            Self::ConvexChord { a, b, t, .. } => (-2.0..=2.0).contains(a) && (-2.0..=2.0).contains(b) && (0.0..=1.0).contains(t),
            Self::GradientDescent { step_size, steps } => (0.0..=0.16).contains(step_size) && (1..=8).contains(steps),
            Self::FeasibleDirections { position, angle } => (-0.5..=2.5).contains(position) && (-180.0..=180.0).contains(angle),
        };
        if valid { Ok(()) } else { Err(invalid("数学实验参数超出本课范围")) }
    }
    pub fn same_experiment(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

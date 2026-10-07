//! Pure presentation guidance selection; RunContext owns the current selection.
use crate::InstructionModule;

pub const REVISION: &str = "ex13.v2";
const COMMON: &str = include_str!("../../../../skills/presentation/SKILL.md");
const ENGINEERING: &str = include_str!("../../../../skills/presentation/engineering.md");
const DIRECTORY: &str = include_str!("../../../../skills/presentation/capabilities.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationPhase {
    Global,
    Local,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationNeed {
    Editing,
    StaticPlot,
    State,
    ContinuousScene,
    Konva,
    Manim,
}

/// Guidance selection only: framework/focus are task data, not instruction text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationGuidance {
    pub phase: PresentationPhase,
    pub needs: Vec<PresentationNeed>,
}

const REFERENCES: &[(PresentationNeed, &str, &str)] = &[
    (
        PresentationNeed::Editing,
        "editing",
        include_str!("../../../../skills/presentation/references/editing.md"),
    ),
    (
        PresentationNeed::StaticPlot,
        "static-plot",
        include_str!("../../../../skills/presentation/references/static-plot.md"),
    ),
    (
        PresentationNeed::State,
        "state",
        include_str!("../../../../skills/presentation/references/state.md"),
    ),
    (
        PresentationNeed::ContinuousScene,
        "continuous-scene",
        include_str!("../../../../skills/presentation/references/continuous-scene.md"),
    ),
    (
        PresentationNeed::Konva,
        "konva",
        include_str!("../../../../skills/presentation/references/konva.md"),
    ),
    (
        PresentationNeed::Manim,
        "manim",
        include_str!("../../../../skills/presentation/references/manim.md"),
    ),
];

fn common_body(source: &str) -> &str {
    source
        .split_once("\n---\n")
        .or_else(|| source.split_once("\r\n---\r\n"))
        .expect("presentation skill frontmatter must close")
        .1
        .trim()
}

pub(super) fn modules(guidance: Option<&PresentationGuidance>) -> Vec<InstructionModule> {
    let mut modules = common_modules();
    modules.extend(selected_modules(guidance));
    modules
}

pub(crate) fn is_sampling_module(module: &InstructionModule) -> bool {
    module.asset_id.starts_with("resident-agent.presentation.phase.")
        || module.asset_id.starts_with("resident-agent.presentation.reference.")
}

fn common_modules() -> Vec<InstructionModule> {
    vec![
        InstructionModule::new(
            "resident-agent.skill.presentation-method",
            REVISION,
            common_body(COMMON),
        ),
        InstructionModule::new(
            "resident-agent.policy.presentation-authoring",
            REVISION,
            ENGINEERING.trim(),
        ),
        InstructionModule::new(
            "resident-agent.presentation.capabilities",
            REVISION,
            DIRECTORY.trim(),
        ),
    ]
}

pub(crate) fn selected_modules(guidance: Option<&PresentationGuidance>) -> Vec<InstructionModule> {
    let mut modules = Vec::new();
    if let Some(guidance) = guidance {
        let (name, text) = match guidance.phase {
            PresentationPhase::Global => (
                "global",
                include_str!("../../../../skills/presentation/phases/global.md"),
            ),
            PresentationPhase::Local => (
                "local",
                include_str!("../../../../skills/presentation/phases/local.md"),
            ),
            PresentationPhase::Review => (
                "review",
                include_str!("../../../../skills/presentation/phases/review.md"),
            ),
        };
        modules.push(InstructionModule::new(
            format!("resident-agent.presentation.phase.{name}"),
            REVISION,
            text.trim(),
        ));
        // Global design considers capabilities, not their implementation contracts.
        if guidance.phase == PresentationPhase::Global {
            return modules;
        }
    }

    for &(need, name, text) in REFERENCES {
        // An unspecified selection retains all technical contracts. Resident passes
        // an explicit selection (global by default) for every sampling.
        let selected = guidance.is_none_or(|g| {
            g.needs.contains(&need)
                || (need == PresentationNeed::State
                    && g.needs.iter().any(|n| {
                        matches!(
                            n,
                            PresentationNeed::ContinuousScene | PresentationNeed::Manim
                        )
                    }))
                || (need == PresentationNeed::ContinuousScene
                    && g.needs.contains(&PresentationNeed::Manim))
        });
        if selected {
            modules.push(InstructionModule::new(
                format!("resident-agent.presentation.reference.{name}"),
                REVISION,
                text.trim(),
            ));
        }
    }
    modules
}

#[cfg(test)]
mod tests;

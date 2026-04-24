//! Built-in demo scenarios.
//!
//! Scenarios are grouped into smaller modules so simple onboarding flows and the
//! larger sandbox topology are easy to navigate independently.

mod complex;
mod simple;

use crate::model::ScenarioDefinition;

/// Returns all built-in demo scenarios.
pub fn built_in_scenarios() -> Vec<ScenarioDefinition> {
    let mut scenarios = simple::scenarios();
    scenarios.extend(complex::scenarios());
    scenarios
}

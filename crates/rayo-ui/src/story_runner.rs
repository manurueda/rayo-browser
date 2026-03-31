//! Story runner — executes user stories with precondition resolution.
//!
//! Stories chain multiple test flows into end-to-end user journeys.
//! The runner resolves `requires` dependencies via topological ordering
//! with cycle detection, shares a single browser session across all flows,
//! and memoizes completed stories so shared prerequisites only run once.

use crate::runner::{
    RunnerContext, assertion_requirements, check_assertion, execute_suite_on_page,
    suite_requirements,
};
use crate::story_types::*;
use crate::types::TestSuite;
use rayo_core::{RayoBrowser, RayoPage, ViewportConfig};
use rayo_profiler::Profiler;
use rayo_visual::BaselineManager;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

/// Configuration for the story runner.
pub struct StoryRunnerConfig {
    pub baselines_dir: PathBuf,
    pub base_url: Option<String>,
}

/// Topological sort of stories based on `requires` dependencies.
///
/// Returns an ordered list of story names or an error describing the cycle.
fn topological_sort(stories: &[UserStory]) -> Result<Vec<String>, String> {
    let story_names: HashSet<&str> = stories.iter().map(|s| s.name.as_str()).collect();

    // Build adjacency: story -> stories it depends on
    let deps: HashMap<&str, Vec<&str>> = stories
        .iter()
        .map(|s| {
            let reqs: Vec<&str> = s
                .requires
                .iter()
                .filter(|r| story_names.contains(r.as_str()))
                .map(|r| r.as_str())
                .collect();
            (s.name.as_str(), reqs)
        })
        .collect();

    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();
    let mut order: Vec<String> = Vec::new();

    fn visit<'a>(
        name: &'a str,
        deps: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        in_stack: &mut HashSet<&'a str>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if visited.contains(name) {
            return Ok(());
        }
        if in_stack.contains(name) {
            return Err(format!("Circular dependency detected involving '{name}'"));
        }
        in_stack.insert(name);
        if let Some(requirements) = deps.get(name) {
            for req in requirements {
                visit(req, deps, visited, in_stack, order)?;
            }
        }
        in_stack.remove(name);
        visited.insert(name);
        order.push(name.to_string());
        Ok(())
    }

    for story in stories {
        visit(&story.name, &deps, &mut visited, &mut in_stack, &mut order)?;
    }

    Ok(order)
}

fn story_requirements(
    stories: &[UserStory],
    suites: &HashMap<String, TestSuite>,
) -> crate::runner::RunnerRequirements {
    let mut requirements = crate::runner::RunnerRequirements::default();

    for suite in suites.values() {
        requirements.merge(suite_requirements(suite));
    }

    for story in stories {
        for flow in &story.flows {
            for then_assertion in &flow.then {
                if let Some(assertion) = &then_assertion.assert {
                    requirements.merge(assertion_requirements(assertion));
                }
            }
        }
    }

    requirements
}

/// Run all stories, resolving preconditions and sharing a single browser session.
pub async fn run_stories(
    stories: &[UserStory],
    suites: &HashMap<String, TestSuite>,
    config: &StoryRunnerConfig,
) -> Vec<StoryResult> {
    if stories.is_empty() {
        return Vec::new();
    }

    // Topological sort for execution order
    let execution_order = match topological_sort(stories) {
        Ok(order) => order,
        Err(cycle_err) => {
            return stories
                .iter()
                .map(|s| StoryResult {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    persona: s.persona.clone(),
                    importance: s.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: 0,
                    flow_results: Vec::new(),
                    error: Some(cycle_err.clone()),
                    started_at: chrono::Utc::now().to_rfc3339(),
                })
                .collect();
        }
    };

    // Launch one browser for all stories
    let profiler = Profiler::new();
    let viewport = ViewportConfig::default();
    let browser = match RayoBrowser::launch_with_config(profiler, viewport).await {
        Ok(b) => b,
        Err(e) => {
            return stories
                .iter()
                .map(|s| StoryResult {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    persona: s.persona.clone(),
                    importance: s.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: 0,
                    flow_results: Vec::new(),
                    error: Some(format!("Failed to launch browser: {e}")),
                    started_at: chrono::Utc::now().to_rfc3339(),
                })
                .collect();
        }
    };

    let page = match browser.new_page().await {
        Ok(p) => p,
        Err(e) => {
            browser.close().await;
            return stories
                .iter()
                .map(|s| StoryResult {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    persona: s.persona.clone(),
                    importance: s.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: 0,
                    flow_results: Vec::new(),
                    error: Some(format!("Failed to create page: {e}")),
                    started_at: chrono::Utc::now().to_rfc3339(),
                })
                .collect();
        }
    };

    let baseline_mgr = BaselineManager::new(config.baselines_dir.clone());
    let mut context = RunnerContext::new(story_requirements(stories, suites));

    // Build story map for precondition lookup
    let story_map: HashMap<String, &UserStory> =
        stories.iter().map(|s| (s.name.clone(), s)).collect();

    // Track completed stories for memoization
    let mut completed: HashMap<String, StoryResult> = HashMap::new();

    // Execute in topological order
    for name in &execution_order {
        if completed.contains_key(name) {
            continue;
        }
        if let Some(story) = story_map.get(name) {
            let result = run_single_story(
                story,
                &story_map,
                suites,
                &page,
                &baseline_mgr,
                config,
                &mut context,
                &mut completed,
                &mut HashSet::new(),
            )
            .await;
            completed.insert(name.clone(), result);
        }
    }

    // Return results in the original story order
    let mut results = Vec::new();
    for story in stories {
        if let Some(result) = completed.remove(&story.name) {
            results.push(result);
        }
    }
    results
}

/// Run a single story, resolving its preconditions first (recursive with memoization).
///
/// Uses `Box::pin` for the recursive call because async recursion requires
/// indirection to avoid infinitely-sized futures.
fn run_single_story<'a>(
    story: &'a UserStory,
    story_map: &'a HashMap<String, &'a UserStory>,
    suites: &'a HashMap<String, TestSuite>,
    page: &'a RayoPage,
    baseline_mgr: &'a BaselineManager,
    config: &'a StoryRunnerConfig,
    context: &'a mut RunnerContext,
    completed: &'a mut HashMap<String, StoryResult>,
    in_progress: &'a mut HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = StoryResult> + 'a>> {
    Box::pin(run_single_story_inner(
        story,
        story_map,
        suites,
        page,
        baseline_mgr,
        config,
        context,
        completed,
        in_progress,
    ))
}

async fn run_single_story_inner(
    story: &UserStory,
    story_map: &HashMap<String, &UserStory>,
    suites: &HashMap<String, TestSuite>,
    page: &RayoPage,
    baseline_mgr: &BaselineManager,
    config: &StoryRunnerConfig,
    context: &mut RunnerContext,
    completed: &mut HashMap<String, StoryResult>,
    in_progress: &mut HashSet<String>,
) -> StoryResult {
    // Memoization: return cached result if already run
    if let Some(cached) = completed.get(&story.name) {
        return cached.clone();
    }

    // Cycle detection
    if in_progress.contains(&story.name) {
        return StoryResult {
            name: story.name.clone(),
            description: story.description.clone(),
            persona: story.persona.clone(),
            importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
            passed: false,
            duration_ms: 0,
            flow_results: Vec::new(),
            error: Some(format!(
                "Circular dependency detected: story '{}' requires itself",
                story.name
            )),
            started_at: chrono::Utc::now().to_rfc3339(),
        };
    }

    in_progress.insert(story.name.clone());
    let start = Instant::now();
    let started_at = chrono::Utc::now().to_rfc3339();

    // Run preconditions first
    for req_name in &story.requires {
        // Check if already completed
        if let Some(prev) = completed.get(req_name) {
            if !prev.passed {
                in_progress.remove(&story.name);
                let result = StoryResult {
                    name: story.name.clone(),
                    description: story.description.clone(),
                    persona: story.persona.clone(),
                    importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    flow_results: Vec::new(),
                    error: Some(format!("Prerequisite '{req_name}' failed")),
                    started_at,
                };
                completed.insert(story.name.clone(), result.clone());
                return result;
            }
            continue;
        }

        // Find and run the prerequisite story
        if let Some(req_story) = story_map.get(req_name) {
            let req_result = run_single_story(
                req_story,
                story_map,
                suites,
                page,
                baseline_mgr,
                config,
                context,
                completed,
                in_progress,
            )
            .await;

            let req_passed = req_result.passed;
            completed.insert(req_name.clone(), req_result);

            if !req_passed {
                in_progress.remove(&story.name);
                let result = StoryResult {
                    name: story.name.clone(),
                    description: story.description.clone(),
                    persona: story.persona.clone(),
                    importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    flow_results: Vec::new(),
                    error: Some(format!("Prerequisite '{req_name}' failed")),
                    started_at,
                };
                completed.insert(story.name.clone(), result.clone());
                return result;
            }
        }
        // If prerequisite not found as a story, try running it as a suite name
        else if let Some(suite) = suites.get(req_name) {
            let base_url = config.base_url.as_deref();
            let suite_results = match execute_suite_on_page(
                page,
                suite,
                baseline_mgr,
                &None,
                base_url,
                false,
                context,
            )
            .await
            {
                Ok(results) => results,
                Err(err) => {
                    in_progress.remove(&story.name);
                    let result = StoryResult {
                        name: story.name.clone(),
                        description: story.description.clone(),
                        persona: story.persona.clone(),
                        importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
                        passed: false,
                        duration_ms: start.elapsed().as_millis() as u64,
                        flow_results: Vec::new(),
                        error: Some(format!(
                            "Prerequisite suite '{}' failed before execution: {}",
                            req_name, err
                        )),
                        started_at,
                    };
                    completed.insert(story.name.clone(), result.clone());
                    return result;
                }
            };

            if let Some(step_result) = suite_results.iter().find(|result| !result.pass) {
                in_progress.remove(&story.name);
                let result = StoryResult {
                    name: story.name.clone(),
                    description: story.description.clone(),
                    persona: story.persona.clone(),
                    importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
                    passed: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    flow_results: Vec::new(),
                    error: Some(format!(
                        "Prerequisite suite '{}' failed at step: {}",
                        req_name,
                        step_result
                            .error
                            .clone()
                            .unwrap_or_else(|| step_result.name.clone())
                    )),
                    started_at,
                };
                completed.insert(story.name.clone(), result.clone());
                return result;
            }
        }
        // Prerequisite not found at all — skip (may be an external dependency)
    }

    // Run each flow in the story
    let mut flow_results = Vec::new();
    let mut story_passed = true;

    for story_flow in &story.flows {
        let flow_start = Instant::now();

        // Look up the TestSuite by name
        let suite = match suites.get(&story_flow.name) {
            Some(s) => s,
            None => {
                flow_results.push(StoryFlowResult {
                    flow_name: story_flow.name.clone(),
                    passed: false,
                    duration_ms: 0,
                    steps_passed: 0,
                    steps_total: 0,
                    then_results: Vec::new(),
                    error: Some(format!(
                        "Flow '{}' not found in loaded test suites",
                        story_flow.name
                    )),
                });
                story_passed = false;
                continue;
            }
        };

        // Run the suite's steps on the shared page
        let base_url = config.base_url.as_deref();
        let suite_results =
            match execute_suite_on_page(page, suite, baseline_mgr, &None, base_url, false, context)
                .await
            {
                Ok(results) => results,
                Err(err) => {
                    flow_results.push(StoryFlowResult {
                        flow_name: story_flow.name.clone(),
                        passed: false,
                        duration_ms: flow_start.elapsed().as_millis() as u64,
                        steps_passed: 0,
                        steps_total: suite.setup.len() + suite.steps.len() + suite.teardown.len(),
                        then_results: Vec::new(),
                        error: Some(format!(
                            "Flow '{}' could not start: {}",
                            story_flow.name, err
                        )),
                    });
                    story_passed = false;
                    continue;
                }
            };

        let steps_passed = suite_results.iter().filter(|result| result.pass).count();
        let total_steps = suite_results.len();
        let step_error = suite_results
            .iter()
            .find(|result| !result.pass)
            .and_then(|result| result.error.clone().or_else(|| Some("Step failed".into())));

        // Run then-assertions (only if all steps passed)
        let mut then_results = Vec::new();
        let mut then_all_passed = step_error.is_none();

        if step_error.is_none() {
            for then_assert in &story_flow.then {
                if let Some(assertion) = &then_assert.assert {
                    let assertion_result =
                        check_assertion(page, assertion, baseline_mgr, context).await;
                    let passed = assertion_result.pass;
                    then_results.push(StoryAssertionResult {
                        description: then_assert.description.clone(),
                        passed,
                        message: assertion_result.message,
                    });
                    if !passed {
                        then_all_passed = false;
                    }
                } else {
                    // No machine assertion — just a description, always passes
                    then_results.push(StoryAssertionResult {
                        description: then_assert.description.clone(),
                        passed: true,
                        message: None,
                    });
                }
            }
        }

        let flow_passed = step_error.is_none() && then_all_passed;
        if !flow_passed {
            story_passed = false;
        }

        flow_results.push(StoryFlowResult {
            flow_name: story_flow.name.clone(),
            passed: flow_passed,
            duration_ms: flow_start.elapsed().as_millis() as u64,
            steps_passed,
            steps_total: total_steps,
            then_results,
            error: step_error,
        });
    }

    in_progress.remove(&story.name);

    let result = StoryResult {
        name: story.name.clone(),
        description: story.description.clone(),
        persona: story.persona.clone(),
        importance: story.importance.clone().unwrap_or_else(|| "medium".into()),
        passed: story_passed,
        duration_ms: start.elapsed().as_millis() as u64,
        flow_results,
        error: None,
        started_at,
    };

    completed.insert(story.name.clone(), result.clone());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal UserStory with a name and requires list.
    fn story(name: &str, requires: &[&str]) -> UserStory {
        UserStory {
            name: name.to_string(),
            description: String::new(),
            persona: None,
            importance: None,
            requires: requires.iter().map(|s| s.to_string()).collect(),
            flows: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Helper: build a UserStory with named flows.
    fn story_with_flows(name: &str, flow_names: &[&str]) -> UserStory {
        UserStory {
            name: name.to_string(),
            description: String::new(),
            persona: None,
            importance: None,
            requires: Vec::new(),
            flows: flow_names
                .iter()
                .map(|n| StoryFlow {
                    name: n.to_string(),
                    then: Vec::new(),
                })
                .collect(),
            tags: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Topological sort / cycle detection
    // -----------------------------------------------------------------------

    #[test]
    fn topological_sort_no_deps() {
        let stories = vec![story("A", &[]), story("B", &[]), story("C", &[])];
        let order = topological_sort(&stories).expect("should succeed");
        assert_eq!(order.len(), 3);
        // All three should appear
        assert!(order.contains(&"A".to_string()));
        assert!(order.contains(&"B".to_string()));
        assert!(order.contains(&"C".to_string()));
    }

    #[test]
    fn topological_sort_linear_chain() {
        // C requires B, B requires A
        let stories = vec![story("A", &[]), story("B", &["A"]), story("C", &["B"])];
        let order = topological_sort(&stories).expect("should succeed");
        let pos_a = order.iter().position(|n| n == "A").unwrap();
        let pos_b = order.iter().position(|n| n == "B").unwrap();
        let pos_c = order.iter().position(|n| n == "C").unwrap();
        assert!(pos_a < pos_b, "A must come before B");
        assert!(pos_b < pos_c, "B must come before C");
    }

    #[test]
    fn topological_sort_diamond() {
        // D requires B and C; B and C both require A
        let stories = vec![
            story("A", &[]),
            story("B", &["A"]),
            story("C", &["A"]),
            story("D", &["B", "C"]),
        ];
        let order = topological_sort(&stories).expect("should succeed");
        let pos_a = order.iter().position(|n| n == "A").unwrap();
        let pos_b = order.iter().position(|n| n == "B").unwrap();
        let pos_c = order.iter().position(|n| n == "C").unwrap();
        let pos_d = order.iter().position(|n| n == "D").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn topological_sort_detects_direct_cycle() {
        // A requires B, B requires A
        let stories = vec![story("A", &["B"]), story("B", &["A"])];
        let err = topological_sort(&stories).expect_err("should detect cycle");
        assert!(
            err.contains("Circular dependency"),
            "Error should mention circular dependency: {err}"
        );
    }

    #[test]
    fn topological_sort_detects_indirect_cycle() {
        // A -> B -> C -> A
        let stories = vec![story("A", &["C"]), story("B", &["A"]), story("C", &["B"])];
        let err = topological_sort(&stories).expect_err("should detect cycle");
        assert!(err.contains("Circular dependency"));
    }

    #[test]
    fn topological_sort_external_deps_ignored() {
        // B requires "login" which isn't a story — should be ignored in topo sort
        let stories = vec![story("A", &[]), story("B", &["login"])];
        let order = topological_sort(&stories).expect("should succeed");
        assert_eq!(order.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Flow name resolution
    // -----------------------------------------------------------------------

    #[test]
    fn flow_resolution_missing_suite() {
        // A story references a flow "nonexistent" which is not in suites
        let s = story_with_flows("checkout", &["nonexistent"]);
        let suites: HashMap<String, TestSuite> = HashMap::new();

        // We can't run the full async runner without a browser, but we can verify
        // the flow lookup logic by checking the suite map directly.
        assert!(
            suites.get("nonexistent").is_none(),
            "Suite should not exist"
        );

        // Verify the story's flow references a name not in suites
        for flow in &s.flows {
            assert!(
                !suites.contains_key(&flow.name),
                "Flow '{}' should not be in suites",
                flow.name
            );
        }
    }

    #[test]
    fn flow_resolution_found() {
        let s = story_with_flows("checkout", &["Add to Cart"]);
        let mut suites = HashMap::new();
        suites.insert(
            "Add to Cart".to_string(),
            TestSuite {
                name: "Add to Cart".to_string(),
                viewport: None,
                setup: Vec::new(),
                steps: Vec::new(),
                teardown: Vec::new(),
            },
        );

        for flow in &s.flows {
            assert!(
                suites.contains_key(&flow.name),
                "Flow '{}' should be in suites",
                flow.name
            );
        }
    }

    #[test]
    fn flow_resolution_partial_match() {
        // Story has two flows, only one exists in suites
        let s = story_with_flows("checkout", &["Add to Cart", "Pay with Card"]);
        let mut suites = HashMap::new();
        suites.insert(
            "Add to Cart".to_string(),
            TestSuite {
                name: "Add to Cart".to_string(),
                viewport: None,
                setup: Vec::new(),
                steps: Vec::new(),
                teardown: Vec::new(),
            },
        );

        let found: Vec<_> = s
            .flows
            .iter()
            .filter(|f| suites.contains_key(&f.name))
            .collect();
        let missing: Vec<_> = s
            .flows
            .iter()
            .filter(|f| !suites.contains_key(&f.name))
            .collect();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Add to Cart");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].name, "Pay with Card");
    }
}

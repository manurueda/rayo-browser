//! Story discovery — chains detected flows into user stories.
//!
//! Analyzes the relationship between flows (auth gates, URL paths, page transitions)
//! to compose them into end-to-end user stories.

use crate::discover::flows::{DetectedFlow, FlowType};
use crate::discover::generator::ExploredPage;
use crate::discover::generator::PageStatus;
use crate::story_types::{StoryAssertion, StoryFlow, UserStory};
use crate::types::Assertion;

/// Discover user stories from detected flows and explored pages.
///
/// Groups flows by auth dependency and URL paths, chains them into
/// end-to-end journeys with human-readable descriptions.
pub fn discover_stories(
    flows: &[DetectedFlow],
    explored_pages: &[ExploredPage],
    _base_url: &str,
) -> Vec<UserStory> {
    let mut stories = Vec::new();

    // Classify flows
    let auth_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| f.flow_type == FlowType::Auth)
        .collect();
    let search_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| f.flow_type == FlowType::Search)
        .collect();
    let form_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| f.flow_type == FlowType::Form)
        .collect();
    let crud_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| f.flow_type == FlowType::Crud)
        .collect();
    let nav_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| f.flow_type == FlowType::Navigation)
        .collect();

    // Detect which pages are auth-gated
    let auth_gated_urls: Vec<&str> = explored_pages
        .iter()
        .filter(|p| p.status == PageStatus::AuthGated)
        .map(|p| p.url.as_str())
        .collect();

    // Story 1: Guest browsing (no auth required)
    let guest_flows: Vec<&DetectedFlow> = flows
        .iter()
        .filter(|f| {
            f.flow_type != FlowType::Auth
                && !auth_gated_urls.iter().any(|gated| f.url.starts_with(gated))
        })
        .collect();

    if guest_flows.len() >= 2 {
        let mut story_flows = Vec::new();

        // Add navigation first
        for f in &nav_flows {
            if !is_auth_gated(&f.url, &auth_gated_urls) {
                story_flows.push(flow_to_story_flow(f));
            }
        }

        // Then search
        for f in &search_flows {
            if !is_auth_gated(&f.url, &auth_gated_urls) {
                story_flows.push(flow_to_story_flow(f));
            }
        }

        if !story_flows.is_empty() {
            stories.push(UserStory {
                name: "Guest can browse and search".into(),
                description: "Visitor explores the site and uses search without logging in".into(),
                persona: Some("guest".into()),
                importance: Some("medium".into()),
                requires: Vec::new(),
                flows: story_flows,
                tags: vec!["guest".into(), "browsing".into()],
            });
        }
    }

    // Story 2: Authenticated session (login + auth-gated pages)
    if !auth_flows.is_empty() {
        let auth_flow = &auth_flows[0];

        // Find flows that are on auth-gated pages
        let gated_flows: Vec<&DetectedFlow> = flows
            .iter()
            .filter(|f| {
                f.flow_type != FlowType::Auth
                    && is_auth_gated(&f.url, &auth_gated_urls)
            })
            .collect();

        if !gated_flows.is_empty() {
            let mut story_flows = vec![flow_to_story_flow(auth_flow)];

            for f in &gated_flows {
                story_flows.push(flow_to_story_flow(f));
            }

            stories.push(UserStory {
                name: "User can log in and access protected pages".into(),
                description: "Authenticated user logs in and accesses gated content".into(),
                persona: Some("user".into()),
                importance: Some("critical".into()),
                requires: Vec::new(),
                flows: story_flows,
                tags: vec!["auth".into(), "protected".into()],
            });
        } else {
            // Even without gated pages, login itself is a story
            stories.push(UserStory {
                name: "User can log in".into(),
                description: "User enters credentials and authenticates successfully".into(),
                persona: Some("user".into()),
                importance: Some("critical".into()),
                requires: Vec::new(),
                flows: vec![flow_to_story_flow(auth_flow)],
                tags: vec!["auth".into()],
            });
        }
    }

    // Story 3: Data management (login + CRUD)
    if !auth_flows.is_empty() && !crud_flows.is_empty() {
        let auth_flow_name = auth_flows[0].name.clone();
        let mut story_flows = Vec::new();

        for f in &crud_flows {
            story_flows.push(flow_to_story_flow(f));
        }

        stories.push(UserStory {
            name: "User can manage data".into(),
            description: "Authenticated user creates and manages records".into(),
            persona: Some("user".into()),
            importance: Some("high".into()),
            requires: vec![auth_flow_name],
            flows: story_flows,
            tags: vec!["crud".into(), "data".into()],
        });
    }

    // Story 4: Form submissions (non-auth forms)
    let standalone_forms: Vec<&DetectedFlow> = form_flows
        .iter()
        .filter(|f| !is_auth_gated(&f.url, &auth_gated_urls))
        .copied()
        .collect();

    if !standalone_forms.is_empty() {
        let mut story_flows = Vec::new();
        for f in &standalone_forms {
            story_flows.push(flow_to_story_flow(f));
        }

        stories.push(UserStory {
            name: "Visitor can submit forms".into(),
            description: "Guest user fills out and submits public forms".into(),
            persona: Some("guest".into()),
            importance: Some("medium".into()),
            requires: Vec::new(),
            flows: story_flows,
            tags: vec!["forms".into()],
        });
    }

    // Story 5: Full journey (login + search + form/checkout if available)
    if !auth_flows.is_empty() && (!search_flows.is_empty() || !form_flows.is_empty()) {
        let auth_flow_name = auth_flows[0].name.clone();
        let mut story_flows = Vec::new();

        for f in &search_flows {
            story_flows.push(flow_to_story_flow(f));
        }
        for f in &form_flows {
            if is_auth_gated(&f.url, &auth_gated_urls) {
                story_flows.push(flow_to_story_flow(f));
            }
        }

        if !story_flows.is_empty() {
            stories.push(UserStory {
                name: "User completes full journey".into(),
                description: "Authenticated user searches, browses, and completes actions".into(),
                persona: Some("user".into()),
                importance: Some("high".into()),
                requires: vec![auth_flow_name],
                flows: story_flows,
                tags: vec!["e2e".into(), "journey".into()],
            });
        }
    }

    stories
}

/// Check if a URL is behind an auth gate.
fn is_auth_gated(url: &str, auth_gated_urls: &[&str]) -> bool {
    auth_gated_urls.iter().any(|gated| url.starts_with(gated))
}

/// Convert a DetectedFlow to a StoryFlow with default then-assertions.
fn flow_to_story_flow(flow: &DetectedFlow) -> StoryFlow {
    let mut then = Vec::new();

    // Add human-readable then-assertions based on flow type
    match flow.flow_type {
        FlowType::Auth => {
            then.push(StoryAssertion {
                description: "User is logged in successfully".into(),
                assert: Some(Assertion {
                    page_map_contains: None,
                    text_contains: None,
                    screenshot: None,
                    network_called: None,
                    js_eval: None,
                    element_state: None,
                    no_console_errors: Some(true),
                    cookie_contains: None,
                }),
            });
        }
        FlowType::Search => {
            then.push(StoryAssertion {
                description: "Search results are displayed".into(),
                assert: Some(Assertion {
                    page_map_contains: Some(crate::types::PageMapAssertion {
                        selector: None,
                        text: None,
                        role: None,
                        tag: None,
                    }),
                    text_contains: None,
                    screenshot: None,
                    network_called: None,
                    js_eval: None,
                    element_state: None,
                    no_console_errors: None,
                    cookie_contains: None,
                }),
            });
        }
        FlowType::Form => {
            then.push(StoryAssertion {
                description: "Form submitted successfully".into(),
                assert: Some(Assertion {
                    page_map_contains: None,
                    text_contains: None,
                    screenshot: None,
                    network_called: None,
                    js_eval: None,
                    element_state: None,
                    no_console_errors: Some(true),
                    cookie_contains: None,
                }),
            });
        }
        FlowType::Crud => {
            then.push(StoryAssertion {
                description: "Data operation completed".into(),
                assert: Some(Assertion {
                    page_map_contains: None,
                    text_contains: None,
                    screenshot: None,
                    network_called: None,
                    js_eval: None,
                    element_state: None,
                    no_console_errors: Some(true),
                    cookie_contains: None,
                }),
            });
        }
        FlowType::Navigation => {
            then.push(StoryAssertion {
                description: "All pages load correctly".into(),
                assert: Some(Assertion {
                    page_map_contains: Some(crate::types::PageMapAssertion {
                        selector: None,
                        text: None,
                        role: None,
                        tag: None,
                    }),
                    text_contains: None,
                    screenshot: None,
                    network_called: None,
                    js_eval: None,
                    element_state: None,
                    no_console_errors: None,
                    cookie_contains: None,
                }),
            });
        }
    }

    StoryFlow {
        name: flow.name.clone(),
        then,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::flows::FlowStep;

    fn make_flow(name: &str, url: &str, flow_type: FlowType) -> DetectedFlow {
        DetectedFlow {
            name: name.into(),
            flow_type,
            steps: vec![FlowStep {
                action: "navigate".into(),
                selector: None,
                value: None,
                url: Some(url.into()),
                assertion: None,
            }],
            url: url.into(),
        }
    }

    fn make_page(url: &str, status: PageStatus) -> ExploredPage {
        ExploredPage {
            url: url.into(),
            status,
            console_errors: 0,
            has_elements: true,
        }
    }

    #[test]
    fn test_discover_stories_with_auth_and_gated_pages() {
        let flows = vec![
            make_flow("login Login Flow", "http://localhost:3000/login", FlowType::Auth),
            make_flow("dashboard Navigation", "http://localhost:3000/dashboard", FlowType::Navigation),
            make_flow("home Navigation", "http://localhost:3000/", FlowType::Navigation),
            make_flow("home Search", "http://localhost:3000/", FlowType::Search),
        ];

        let pages = vec![
            make_page("http://localhost:3000/", PageStatus::Ok),
            make_page("http://localhost:3000/login", PageStatus::Ok),
            make_page("http://localhost:3000/dashboard", PageStatus::AuthGated),
        ];

        let stories = discover_stories(&flows, &pages, "http://localhost:3000");

        // Should discover at least a guest browsing story and an auth story
        assert!(!stories.is_empty(), "Should discover at least one story");

        // Should have an auth-related story
        let auth_story = stories.iter().find(|s| s.tags.contains(&"auth".into()));
        assert!(auth_story.is_some(), "Should have an auth story");

        // The auth story should include the auth-gated dashboard flow
        let auth = auth_story.unwrap();
        assert!(auth.importance == Some("critical".into()));
    }

    #[test]
    fn test_discover_stories_guest_only() {
        let flows = vec![
            make_flow("home Navigation", "http://localhost:3000/", FlowType::Navigation),
            make_flow("home Search", "http://localhost:3000/", FlowType::Search),
            make_flow("about Navigation", "http://localhost:3000/about", FlowType::Navigation),
        ];

        let pages = vec![
            make_page("http://localhost:3000/", PageStatus::Ok),
            make_page("http://localhost:3000/about", PageStatus::Ok),
        ];

        let stories = discover_stories(&flows, &pages, "http://localhost:3000");

        let guest_story = stories.iter().find(|s| s.persona == Some("guest".into()));
        assert!(guest_story.is_some(), "Should have a guest browsing story");
    }

    #[test]
    fn test_discover_stories_empty_flows() {
        let stories = discover_stories(&[], &[], "http://localhost:3000");
        assert!(stories.is_empty(), "No flows should produce no stories");
    }

    #[test]
    fn test_discover_stories_crud() {
        let flows = vec![
            make_flow("login Login Flow", "http://localhost:3000/login", FlowType::Auth),
            make_flow("users CRUD", "http://localhost:3000/admin/users", FlowType::Crud),
        ];

        let pages = vec![
            make_page("http://localhost:3000/login", PageStatus::Ok),
            make_page("http://localhost:3000/admin/users", PageStatus::Ok),
        ];

        let stories = discover_stories(&flows, &pages, "http://localhost:3000");

        let crud_story = stories.iter().find(|s| s.tags.contains(&"crud".into()));
        assert!(crud_story.is_some(), "Should have a data management story");

        let crud = crud_story.unwrap();
        assert_eq!(crud.requires.len(), 1, "CRUD story should require auth");
        assert!(crud.requires[0].contains("Login"));
    }

    #[test]
    fn test_flow_to_story_flow_auth() {
        let flow = make_flow("login Login Flow", "http://localhost:3000/login", FlowType::Auth);
        let sf = flow_to_story_flow(&flow);

        assert_eq!(sf.name, "login Login Flow");
        assert!(!sf.then.is_empty());
        assert_eq!(sf.then[0].description, "User is logged in successfully");
    }

    #[test]
    fn test_is_auth_gated() {
        let gated = vec!["http://localhost:3000/dashboard", "http://localhost:3000/admin"];

        assert!(is_auth_gated("http://localhost:3000/dashboard", &gated));
        assert!(is_auth_gated("http://localhost:3000/dashboard/settings", &gated));
        assert!(is_auth_gated("http://localhost:3000/admin", &gated));
        assert!(!is_auth_gated("http://localhost:3000/", &gated));
        assert!(!is_auth_gated("http://localhost:3000/login", &gated));
    }
}

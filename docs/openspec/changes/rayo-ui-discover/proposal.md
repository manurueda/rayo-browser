# rayo-ui discover: Auto-discovery and test generation

## Why
Writing E2E tests by hand is slow and incomplete. Developers either skip tests or write them after the fact, missing edge cases. rayo already has the fastest browser automation layer and a test runner — adding auto-discovery closes the loop: point rayo at your app, get complete test coverage automatically. Code analysis reveals intent (routes, forms, endpoints). Browser exploration with page maps reveals reality (what actually renders). The delta between the two is where bugs live.

## Solution
New `rayo-ui discover` CLI command that combines static code analysis with live browser exploration to auto-generate YAML test files. Framework-specific analyzers parse source code to extract routes, forms, and API endpoints. The browser explorer visits each route using rayo-core page maps (<1ms per page) and detects interactive flows (forms, auth, CRUD, navigation, search). The YAML generator produces test files with steps and assertions derived from live exploration. A discovery report summarizes coverage and health score.

## Architecture

```
  rayo-ui discover <url>
       │
       ▼
  FrameworkDetector
       │
       ├── NextJsAnalyzer     → app router / pages router
       ├── ExpressAnalyzer     → routes, middleware
       ├── RailsAnalyzer       → config/routes.rb
       ├── DjangoAnalyzer      → urls.py
       ├── FastAPIAnalyzer     → decorators
       ├── StaticHtmlAnalyzer  → HTML files, links
       └── GenericAnalyzer     → sitemap.xml, robots.txt, link crawling
       │
       ▼
  BrowserExplorer (rayo-core)
       │
       ├── Visit each route → page_map (<1ms)
       ├── Detect flows (forms, auth, CRUD, navigation, search)
       ├── Capture console errors
       └── Capture screenshot baselines
       │
       ▼
  FlowDetector
       │
       ├── FormFlow       → input fields + submit button → type + click steps
       ├── AuthFlow        → login/signup page patterns → credential entry + redirect
       ├── CrudFlow        → list + create/edit/delete patterns → multi-step sequences
       ├── NavigationFlow  → menu/nav links → click + assert destination
       └── SearchFlow      → search input + results → type + assert results
       │
       ▼
  YamlGenerator
       │
       ├── Per-flow test files → .rayo/tests/<flow>.test.yaml
       ├── Smoke test         → .rayo/tests/_smoke.test.yaml (visit every page)
       └── Discovery report   → .rayo/discover-report.md (health score 0-100)
```

## Scope
- Framework-specific code analyzers (Next.js, Express, Rails, Django, FastAPI, static HTML, generic)
- Browser exploration using rayo-core page maps
- Flow detection engine (forms, auth, CRUD, navigation, search)
- YAML test file generation with assertions from live data
- Console error detection during exploration
- Screenshot baseline auto-capture during discovery
- Diff-aware mode (`--diff` flag) for PR-scoped discovery using git diff
- Discovery report with health score (0-100)
- Smoke test generation (visit every route, assert loads)
- CLI integration: `rayo-ui discover <url> [--diff]`

## Not in scope
- Custom framework plugins (Phase 2)
- AI-powered flow inference beyond pattern matching (Phase 2)
- Multi-language code analysis beyond framework conventions (Phase 2)
- Parallel exploration across multiple browser tabs (Phase 2)

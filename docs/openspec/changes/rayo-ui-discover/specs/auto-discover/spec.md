# Auto-discover

## ADDED Requirements

### Requirement: Basic discovery navigates routes and generates YAML tests

The discover command SHALL accept a URL, explore the application using rayo-core page maps, and generate YAML test files in `.rayo/tests/` for every discovered flow. A smoke test covering all routes SHALL always be generated.

#### Scenario: Discovery generates test files from a running app
Given a web application running at http://localhost:3000 with 5 routes
When `rayo-ui discover http://localhost:3000` is executed
Then YAML test files are generated in .rayo/tests/
And a _smoke.test.yaml is generated that visits all 5 routes
And each generated test file is valid YAML parseable by the rayo-ui runner

#### Scenario: Discovery visits all routes using page maps
Given a web application with routes /home, /about, /contact
When discovery explores the application
Then each route is visited via rayo-core navigate
And a page map is captured for each route in under 1ms
And interactive elements from page maps are used to build test steps

### Requirement: Framework detection identifies the application framework

The discover command SHALL auto-detect the application framework from project files (package.json, Gemfile, requirements.txt, etc.) and use framework-specific analyzers to extract routes from source code.

#### Scenario: Next.js app router is detected and routes extracted
Given a project with package.json containing "next" dependency and an app/ directory
When framework detection runs
Then "Next.js (app router)" is identified
And routes are extracted from the app/ directory structure

#### Scenario: Express routes are extracted from source
Given a project with package.json containing "express" dependency
When the Express analyzer runs
Then routes defined via app.get/post/put/delete patterns are extracted

#### Scenario: Unknown framework falls back to generic discovery
Given a project with no recognized framework files
When framework detection runs
Then the generic analyzer is used
And routes are discovered via sitemap.xml, robots.txt, and link crawling

### Requirement: Flow detection identifies interactive flows from page maps

The flow detector SHALL analyze page maps to identify form flows, auth flows, CRUD flows, navigation flows, and search flows. Each detected flow generates a multi-step test with appropriate assertions.

#### Scenario: Form flow is detected from page map with inputs and submit
Given a page map containing input elements and a submit button
When flow detection analyzes the page
Then a FormFlow is detected
And a test is generated with type steps for each input and a click step for submit

#### Scenario: Auth flow is detected from login page patterns
Given a page at /login with email input, password input, and sign-in button
When flow detection analyzes the page
Then an AuthFlow is detected
And a test is generated with credential entry steps and a redirect assertion

### Requirement: Diff-aware mode scopes discovery to changed routes

When the `--diff` flag is provided, discovery SHALL use git diff to identify changed source files, determine which routes are affected, and only explore and generate tests for those routes.

#### Scenario: Diff-aware mode only discovers changed routes
Given a git branch with changes to files affecting routes /settings and /profile
When `rayo-ui discover http://localhost:3000 --diff` is executed
Then only /settings and /profile routes are explored
And test files are generated only for flows on those routes
And unchanged routes are not visited

#### Scenario: Diff-aware mode with no route changes produces no tests
Given a git branch with changes only to non-route files (e.g., README.md)
When `rayo-ui discover http://localhost:3000 --diff` is executed
Then no routes are explored
And a message indicates no route changes were detected

### Requirement: Console errors are detected during exploration

During browser exploration, the discover command SHALL capture any console errors emitted by the application and include them in the discovery report.

#### Scenario: Console errors are captured and reported
Given a web application that logs 2 console errors on the /dashboard route
When discovery explores /dashboard
Then both console errors are captured
And the discovery report includes the errors with the route where they occurred

### Requirement: Discovery report summarizes coverage and health

After discovery completes, a report SHALL be written to `.rayo/discover-report.md` containing: framework detected, routes from code, routes explored, flows detected (by type), console errors found, generated test files, test pass/fail counts, and a health score from 0 to 100.

#### Scenario: Discovery report is generated with health score
Given discovery completes with 20 routes, 12 flows, and 1 console error
When the report is generated
Then .rayo/discover-report.md is written
And it contains the framework name, route counts, flow breakdown by type, console error count, and a health score
And the health score is between 0 and 100

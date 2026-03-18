# rayo-ui discover: Tasks

## Tasks

- [ ] Framework analyzers: Next.js app router and pages router route extraction
- [ ] Framework analyzers: Express route extraction from app.get/post/put/delete patterns
- [ ] Framework analyzers: Rails route extraction from config/routes.rb
- [ ] Framework analyzers: Django route extraction from urls.py
- [ ] Framework analyzers: FastAPI route extraction from decorator patterns
- [ ] Framework analyzers: Static HTML file and link discovery
- [ ] Framework analyzers: Generic fallback (sitemap.xml, robots.txt, link crawling)
- [ ] Framework detection: auto-detect framework from package.json, Gemfile, requirements.txt, etc.
- [ ] Flow detection engine: FormFlow — detect input fields + submit buttons, generate type + click steps
- [ ] Flow detection engine: AuthFlow — detect login/signup page patterns, generate credential entry + redirect steps
- [ ] Flow detection engine: CrudFlow — detect list + create/edit/delete patterns, generate multi-step sequences
- [ ] Flow detection engine: NavigationFlow — detect menu/nav links, generate click + assert destination steps
- [ ] Flow detection engine: SearchFlow — detect search input + results, generate type + assert results steps
- [ ] YAML generator: produce per-flow .rayo/tests/<flow>.test.yaml files with steps and assertions
- [ ] YAML generator: produce _smoke.test.yaml that visits every discovered route and asserts page loads
- [ ] Discovery report: generate .rayo/discover-report.md with route counts, flow counts, console errors, health score (0-100)
- [ ] Diff-aware mode: use git diff to identify changed files, scope framework analysis to affected routes only
- [ ] Console error detection: capture browser console errors during exploration, include in report
- [ ] Screenshot baseline auto-capture: save baselines during discovery for visual regression
- [ ] CLI integration: wire `rayo-ui discover <url> [--diff]` subcommand into rayo-ui binary
- [ ] Integration tests: discover against test fixture apps, verify generated YAML is valid and runnable

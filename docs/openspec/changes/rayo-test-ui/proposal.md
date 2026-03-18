# rayo-ui-ui: Web UI for visual testing

## Why
A web UI transforms rayo from a CLI tool into a visual testing platform. Engineers need to see test results — page maps, screenshots, diffs, timing — in a visual, interactive format. The UI also enables live test execution monitoring via WebSocket. This is what Momentic charges enterprise prices for.

## Solution
Next.js app in `/ui` directory using shadcn/ui + magic ui + Tailwind CSS. Communicates with rayo-ui's axum server via REST API and WebSocket. Served by rayo-ui when `rayo-ui ui` is run.

## Pages

1. **Dashboard** — health overview, last run summary, trend charts, speed metrics
2. **Suite List** — all test suites with pass/fail, duration, history sparklines
3. **Test Detail** — step-by-step with page maps, screenshots, assertions, timing, diff overlays
4. **Live Runner** — real-time step execution with WebSocket updates
5. **Settings** — viewport config, diff thresholds, baseline management

## Scope
- Next.js app with shadcn/ui + magic ui + Tailwind
- 5 pages (dashboard, suites, detail, live, settings)
- WebSocket integration for live updates
- Diff overlay viewer
- CI-friendly: static export option for embedding in CI artifacts

## Not in scope
- Authentication (local tool, no auth needed)
- Cloud deployment (local-first)
- Mobile app (web responsive is sufficient)

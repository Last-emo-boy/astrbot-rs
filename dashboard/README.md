# AstrBot RS Dashboard

This dashboard is a static, no-build frontend that mirrors the AstrBot dashboard information architecture while targeting the Rust management API.

## Runtime

- Entry: `index.html` loads `app.js` as an ES module.
- Static server: `astrbot-web` serves `/app.js`, `/styles.css`, and nested `/src/**/*.js` files directly.
- Local check: `cargo run -p astrbot-cli -- run .workflow/.scratchpad/dashboard-smoke-6211.json`.
- If the management router is started with auth, set the Bearer token in Settings or in the error panel shown after a 401 response.
- Settings reads `/api/management/api-keys` for API key catalog, issue, and revoke flows. Issued secrets are displayed only in the issue response.
- OpenAPI chat clients can POST to `/api/openapi/chat` with `Authorization: Bearer <api-key>` or `x-api-key`; the key must include the `openapi.chat` scope.
- SubAgent reads `/api/management/subagents` for configured agents and handoff preview metadata. It does not execute delegated agents from the dashboard.

## Module Boundaries

- `src/routes.js`: navigation model and page metadata.
- `src/state.js`: shared UI state.
- `src/api.js`: JSON fetch helpers.
- `src/loaders.js`: API reads and route preloading.
- `src/render/*.js`: page renderers grouped by domain.
- `src/actions/*.js`: user actions grouped by domain.
- `src/dom.js`: DOM helpers, escaping, toast, and connection state.

Keep new frontend work in the matching domain module. Do not add large page/action logic back into `app.js`.

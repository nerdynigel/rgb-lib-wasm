# Workflows

## ci.yml — Lint, Check & Test

Runs formatting, clippy, cargo check, docs, native tests, wasm-pack tests, and integration tests.

- **Trigger**: manual (`workflow_dispatch`) or `pull_request: [labeled]` with the `ci:run` label. No `push` or ordinary PR event runs this workflow.
- **Job gate**: every job has `if: github.event_name == 'workflow_dispatch' || github.event.label.name == 'ci:run'`, so an ordinary PR `opened`/`synchronize`/`reopened` event starts no runner job.
- **What it does**: fmt → clippy → check → doc → native tests → wasm tests → integration tests (Docker)

## release.yml — Production Release

Builds WASM package and publishes to npm as `@utexo/rgb-lib-wasm`.

- **Trigger**: manual (`workflow_dispatch`)
- **Input**: `version` (e.g. `0.3.0-beta.4`)
- **What it does**: `wasm-pack build` → set version → `npm publish` → GitHub Release
- **Intended branch**: `master`
- **npm tag**: `latest` (default)

## release-dev.yml — Dev Release

Same as production but with a custom suffix for testing.

- **Trigger**: manual (`workflow_dispatch`)
- **Inputs**: `version` (e.g. `0.3.0-beta.4`) + `suffix` (e.g. `test1`)
- **Published version**: `0.3.0-beta.4.test1`
- **npm tag**: value of `suffix` (not `latest`)
- **Intended branch**: `dev` or any feature branch

### Example

Run from `dev` with version `0.3.0-beta.4` and suffix `wasm-idb`:

```bash
npm install @utexo/rgb-lib-wasm@0.3.0-beta.4.wasm.idb
```

## Required secrets

- `NPM_TOKEN` — npm access token for publishing `@utexo/rgb-lib-wasm`

## CI trigger policy (THE-398)

GitHub-hosted runners are opt-in only (Board policy, 30 Sep 2026). The authoritative gate is host-executed CI plus independent verification; GitHub runners are used to test a PR immediately before merge, only when explicitly requested.

- Manual route: `gh workflow run ci.yml` (Actions → CI → Run workflow).
- Label route: add the `ci:run` label to a PR. Adding the label fires the `pull_request: [labeled]` event; the job gate then runs the jobs.
- Ordinary `push` and PR `opened`/`synchronize`/`reopened` events run nothing.
- Release workflows (`release.yml`, `release-dev.yml`) remain `workflow_dispatch` only.

### Before → after (all workflows in this repository)

| Workflow | Triggers before | Triggers after | Job gate |
|---|---|---|---|
| `.github/workflows/ci.yml` | `push` to dev/stage/master; `pull_request` to dev/stage/master | `workflow_dispatch`; `pull_request: [labeled]` | `if: github.event_name == 'workflow_dispatch' \|\| github.event.label.name == 'ci:run'` on all 7 jobs |
| `.github/workflows/release.yml` | `workflow_dispatch` | `workflow_dispatch` (unchanged) | n/a (manual only) |
| `.github/workflows/release-dev.yml` | `workflow_dispatch` | `workflow_dispatch` (unchanged) | n/a (manual only) |

No `repository_dispatch` trigger exists in this repository. No `schedule`-only job exists.

### Rollback

To revert this hygiene change before it is adopted on the default branch, restore the previous `ci.yml` trigger block and remove the per-job `if:` lines:

```yaml
on:
  push:
    branches: [dev, stage, master]
  pull_request:
    branches: [dev, stage, master]
```

`git revert <commit>` on the task branch (or dropping the branch before merge) fully restores the previous behavior. No product, test, dependency, spending or secret configuration is touched.

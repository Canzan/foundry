# Slice 02: build stamp (US-RVF-02)

**Feature:** release-version-footer · **Story:** US-RVF-02 · **job_id:** `job-confirm-release-serving`
**Persona:** Priya Raman (`persona-instance-operator`) · **Effort:** ≤ 1 day · **Walking skeleton:** no
**Spec:** `../feature-delta.md` → "Wave: DISCUSS — increment 2026-10-04 (build stamp)", D5-D14, AC-4..AC-10

## Outcome

Today every full page ends with `Foundry v0.7.0`. After this slice it ends with
`Foundry v0.7.0 · 2026-10-04`, and the same `<footer>` carries `data-commit="817c16d"`. From one
`/sign-in` load Priya can tell which commit is serving, and can spot a stale `:main` image or an
unstamped dev build.

## Learning hypothesis

We believe that a commit date in the footer, plus the short SHA in `data-commit`, will let Priya
confirm a rollout landed the commit she pushed, not just the release number, without `kubectl` or
Argo CD.
We will know it worked when the first image published after this slice shows a real (not `unknown`)
date and SHA on both https://foundry.unintelligent-design.us/sign-in and
https://foundry.jeffbailey.us/sign-in, and those values match the pushed commit.
The riskiest assumption is that the container build sees the build-args. canzan-lift shipped
`unknown · unknown` to production until its workflow passed them (its
`fix-build-stamp-unknown-in-container` RCA). AC-8 is the check.

## IN

- A new `crates/foundry-app/build.rs` that emits `FOUNDRY_BUILD_SHA` and `FOUNDRY_BUILD_DATE`.
  Precedence: the `FOUNDRY_STAMP_*` input (trimmed, blank counts as absent), then `git`, then
  `unknown` (D9). Nothing in it can fail the build (D10). Rerun directives keep the stamp fresh
  (D11).
- `base.html` footer: `Foundry v{{ RELEASE_VERSION }} · {{ date }}`, with `data-commit` on the same
  element (D5, D8). The version is still `CARGO_PKG_VERSION` (D6). The degraded form is
  `· unknown` / `data-commit="unknown"`, never blank (AC-6).
- A pure, unit-testable precedence function and a pure label/placeholder function, mirroring
  canzan-lift's `stamp` and `answered_or_unknown`.
- Dockerfile: `ARG FOUNDRY_STAMP_SHA=` / `ARG FOUNDRY_STAMP_DATE=` in the builder stage,
  immediately before the `cargo build` RUN (D12).
- `.forgejo/workflows/build-and-publish.yml` (production path) and `.github/workflows/release.yml`:
  compute `git rev-parse --short=7 HEAD` and `git log -1 --format=%cd --date=short`, fail the job if
  either is empty, and pass both as build-args, on `main` and tag builds (D12).
- Amend the `@rvf` exact-text oracle to the D5 shape and add the `data-commit` assertion (AC-4).
  The oracle reads git and `Cargo.toml` and never reads the production constant.
- RELEASING.md "Verifying a deployment": check the date and `data-commit`, not only the version
  (AC-10).

## OUT

- `foundry --version` (does not exist), `/healthz`, `/readyz`, the metrics listener, and any version
  API (D13).
- A visible SHA, the branch name, or a dirty flag. A tag-derived version, or `+<sha>` on `main`
  builds (OQ-2).
- `docker-compose.yml` and local image builds. They may read `unknown`, which is honest.
- CSS and layout changes. If one is forced, it carries the D18 re-hash (D14).
- Re-stamping already-published images.

## Acceptance (summary; full text in feature-delta)

| AC | Check | Lane / evidence |
|---|---|---|
| AC-4 | sign-in and board footer = `Foundry v<crate> · <date>`, `data-commit` = compiled SHA | `@rvf` HTTP lane |
| AC-5 | explicit input wins; blank = absent; no git → `unknown` | unit examples on the pure function |
| AC-6 | degraded → `Foundry v<crate> · unknown`, `data-commit="unknown"`, escaped | unit example |
| AC-7 | a commit, or toggling an input, re-stamps an incremental build | manual demo, recorded |
| AC-8 | workflows pass args, fail on empty; prod footer matches the tag commit | workflow review + post-tag operator check |
| AC-9 | once per page, next sibling, no scrollbar, no fragment footer, one line at 320px | `@rvf`, canzan-theme-system, pwa-mobile-rendering, blr |
| AC-10 | runbook names the date and SHA check | RELEASING.md diff |

## Constraints / NFR

- Zero new crates, and no `[build-dependencies]` (D10).
- The build never fails because of the stamp (D10).
- No stale stamp. `rerun-if-changed` registers only paths that exist (D11).
- The footer stays the immediate next sibling of `{% block content %}` (D14).

## Pre-requisites / risks

- P1: resolved 2026-10-04. `job-confirm-release-serving` is registered in `docs/product/jobs.yaml`
  and the persona. Ships in v0.8.0 (OQ-3). `main` builds are not version-marked (OQ-2).
- Risk: the Dockerfile's `/work/target` cache mount keeps a build script's output between builds.
  `rerun-if-env-changed` on both inputs is what forces a re-stamp there, so D11 is not optional.
- Risk: an `ARG` declared too early busts cached layers on every commit. Declare the ARGs right
  before the build RUN.
- Memory: acceptance CLI lanes use a prebuilt binary. `@rvf` is the in-process HTTP lane, so it
  sees the freshly compiled stamp.

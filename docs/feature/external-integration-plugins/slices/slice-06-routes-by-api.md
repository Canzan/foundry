# Slice 06: Routes are configurable as code through /api/v1

Stories: US-EIP-12 | Estimate: 2 days | job_id: `job-grant-custom-integration-access` | Deliverable C (first slice)

## Goal

Sven lists, creates, pauses and deletes Identity Platform's routes with a machine
token acting as Lena, and lists the workspace's connections, with no secret in any
answer. Every new operation is in `/api/v1/openapi.json`, and CI fails if it is not.

## IN

- `GET /api/v1/integrations` (workspace connections: kind, name, status).
- `GET/POST /api/v1/teams/{team_slug}/projects/{project_slug}/routes`, `PATCH/DELETE …/routes/{route_id}`: events, lanes, content level, pause and resume, for Slack, Matrix and (after slice 07) webhook routes.
- Stable error codes `destination_unknown`, `event_unknown`, `lane_unknown`, `content_unknown`; uniform 404 for tokens without Manage.
- Audit "via API"; `openapi.json` updated in the same change.

## OUT

- Connecting Slack or Matrix by API (Slack's approval needs a browser).
- Scoped tokens (slice 08).

## Learning Hypothesis

- **Disproves if it fails**: that one route resource describes every kind with one `destination` shape. If kinds need different shapes, the API exposes the plugin contract and webhook routes must be designed with it.
- **Confirms if it succeeds**: Sven recreates the dev routes from a script after a database rebuild (dogfood).

## Acceptance Criteria

- [ ] List shows routes without secrets; create answers 201 and the route works; unknown destination answers 422 `destination_unknown`.
- [ ] Pause then delete answer as documented (delete 204).
- [ ] A token acting as a plain member gets the uniform 404.
- [ ] `cargo xtask check-arch` (`api-openapi-operations`, `api-openapi-statuses`) passes with the new operations described.

## Dependencies

- Slices 01-03 (and 04 for Matrix routes). The shipped drift check.
- OD-14 confirmed for webhook secrets returned on create (if slice 07 lands first).

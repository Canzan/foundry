-- 0030_integrations.sql — external-integration-plugins slice 01 (DESIGN
-- [REF] Persistence Model, DDR-4, DDR-5, DDR-9, DDR-17, DDR-19, DDR-21,
-- DDR-27; ADR-EIP-002, ADR-EIP-004).
--
-- New tables only: no `projects` column (so `live_projects` is not recreated)
-- and no change to any shipped table. Every Rust reader of these rows selects
-- projects through `live_projects`; route writers open with lock_live_project.
-- Not added to TENANT_TABLES in v1 (DD-5).
--
-- Undo (drops every connection, route and delivery):
--   DROP TABLE IF EXISTS integration_key_checks, integration_audit_events,
--       integration_deliveries, integration_route_lanes,
--       integration_destination_pacing, integration_routes,
--       integration_connections, integration_worker_heartbeats;

-- Dispatcher (and sender, sweeper) liveness for the stall gauge.
CREATE TABLE IF NOT EXISTS integration_worker_heartbeats (
    role     TEXT NOT NULL,
    replica  TEXT NOT NULL,
    beat_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (role, replica)
);

-- One Slack or Matrix connection per workspace and kind (D-7). The sealed
-- credential is the only secret anywhere (DDR-9); a disconnected connection
-- holds none, and only a disconnected one.
CREATE TABLE IF NOT EXISTS integration_connections (
    id                   UUID PRIMARY KEY,
    workspace_id         UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind                 TEXT NOT NULL CHECK (kind IN ('slack', 'matrix')),
    state                TEXT NOT NULL CHECK (state IN ('connected', 'lost', 'disconnected')),
    external_id          TEXT NOT NULL,
    app_id               TEXT NULL,
    bot_user_id          TEXT NULL,
    label                TEXT NOT NULL,
    detail               TEXT NULL,
    sealed_credential    BYTEA NULL,
    credential_saved_at  TIMESTAMPTZ NULL,
    lost_reason          TEXT NULL,
    lost_at              TIMESTAMPTZ NULL,
    connected_by         UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    connected_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT integration_connections_one_per_kind UNIQUE (workspace_id, kind),
    CONSTRAINT integration_connections_credential_shape
        CHECK ((state = 'disconnected') = (sealed_credential IS NULL))
);

-- A route: one project's chosen events to one destination. A webhook route
-- (deliverable C) has no connection, and only a webhook route. `events` is
-- non-empty and within the kind's eligible events (D-2).
CREATE TABLE IF NOT EXISTS integration_routes (
    id                 UUID PRIMARY KEY,
    workspace_id       UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id         UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    connection_id      UUID NULL REFERENCES integration_connections(id),
    kind               TEXT NOT NULL CHECK (kind IN ('slack', 'matrix', 'webhook')),
    destination_ref    TEXT NOT NULL,
    destination_label  TEXT NOT NULL,
    events             TEXT[] NOT NULL,
    content_level      TEXT NOT NULL
        CHECK (content_level IN ('link_only', 'summary', 'summary_with_comment')),
    shared_confirmed   BOOLEAN NOT NULL DEFAULT FALSE,
    receives_from      TIMESTAMPTZ NOT NULL,
    paused_at          TIMESTAMPTZ NULL,
    paused_by          UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    problem_reason     TEXT NULL,
    problem_at         TIMESTAMPTZ NULL,
    created_by         UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    created_via        TEXT NOT NULL CHECK (created_via IN ('web', 'api')),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT integration_routes_connection_shape
        CHECK ((kind = 'webhook') = (connection_id IS NULL)),
    CONSTRAINT integration_routes_events_eligible CHECK (
        cardinality(events) > 0
        AND events <@ CASE WHEN kind = 'webhook'
            THEN ARRAY['issue.created', 'issue.moved', 'issue.deleted', 'comment.added',
                       'comment.edited', 'comment.deleted']
            ELSE ARRAY['issue.created', 'issue.moved', 'issue.deleted', 'comment.added']
        END)
);
CREATE INDEX IF NOT EXISTS idx_integration_routes_project ON integration_routes (project_id);

-- Pacing per destination (NFR-EIP-3): two routes to one channel share a row.
CREATE TABLE IF NOT EXISTS integration_destination_pacing (
    connection_id    UUID NOT NULL REFERENCES integration_connections(id) ON DELETE CASCADE,
    destination_ref  TEXT NOT NULL,
    not_before       TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (connection_id, destination_ref)
);

-- A route's lane filter. NO foreign key to `lanes`, on purpose: a deleted
-- lane must stay detectable (US-EIP-03 example 5), not cascade into "all lanes".
CREATE TABLE IF NOT EXISTS integration_route_lanes (
    route_id       UUID NOT NULL REFERENCES integration_routes(id) ON DELETE CASCADE,
    lane_id        UUID NOT NULL,
    label_at_save  TEXT NOT NULL,
    PRIMARY KEY (route_id, lane_id)
);

-- The work queue (ADR-EIP-002). One row per route (or, slice 05, member) and
-- event or burst; `give_up_at` is fixed at dispatch and never NULL (DDR-5);
-- `claim_id` fences the record and the sweeps (DDR-4). A finished row has a
-- finish time, and only a finished row.
CREATE TABLE IF NOT EXISTS integration_deliveries (
    id                  BIGSERIAL PRIMARY KEY,
    workspace_id        UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    route_id            UUID NULL REFERENCES integration_routes(id) ON DELETE CASCADE,
    recipient_user_id   UUID NULL REFERENCES users(id) ON DELETE CASCADE,
    event_kind          TEXT NOT NULL CHECK (event_kind IN (
                            'issue.created', 'issue.moved', 'issue.deleted', 'comment.added',
                            'comment.edited', 'comment.deleted')),
    event_id            UUID NOT NULL,
    outbox_ids          BIGINT[] NOT NULL CHECK (cardinality(outbox_ids) > 0),
    issue_id            UUID NULL,
    issue_key           TEXT NULL,
    occurred_at         TIMESTAMPTZ NOT NULL,
    status              TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending', 'delivered', 'failed', 'skipped')),
    attempts            INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    first_attempt_at    TIMESTAMPTZ NULL,
    give_up_at          TIMESTAMPTZ NOT NULL,
    next_attempt_at     TIMESTAMPTZ NULL,
    claim_id            UUID NULL,
    lease_until         TIMESTAMPTZ NULL,
    last_vendor_reason  TEXT NULL,
    final_reason        TEXT NULL,
    slowed_down         BOOLEAN NOT NULL DEFAULT FALSE,
    finished_at         TIMESTAMPTZ NULL,
    CONSTRAINT integration_deliveries_one_target
        CHECK ((route_id IS NULL) <> (recipient_user_id IS NULL)),
    CONSTRAINT integration_deliveries_finish_shape
        CHECK ((status = 'pending') = (finished_at IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_integration_deliveries_route_pending
    ON integration_deliveries (route_id, id) WHERE status = 'pending';

-- Append-only audit (DDR-21): who did what, via which door, when. No foreign
-- key to projects or teams (the lifecycle-events precedent); never a secret.
CREATE TABLE IF NOT EXISTS integration_audit_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID NULL,
    actor_id      UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    via           TEXT NOT NULL CHECK (via IN ('web', 'api', 'slack', 'system')),
    action        TEXT NOT NULL CHECK (action IN (
                      'connection_connected', 'connection_reconnected', 'connection_lost',
                      'connection_disconnected', 'route_created', 'route_edited',
                      'route_paused', 'route_resumed', 'route_deleted',
                      'email_choices_saved', 'email_stopped', 'secrets_rotated')),
    kind          TEXT NOT NULL CHECK (kind IN ('slack', 'matrix', 'webhook', 'email')),
    subject       TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_integration_audit_events_workspace
    ON integration_audit_events (workspace_id, created_at);

-- One sealed canary per key id ever used (DDR-9): the boot key check.
CREATE TABLE IF NOT EXISTS integration_key_checks (
    key_id         TEXT PRIMARY KEY,
    sealed_canary  BYTEA NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

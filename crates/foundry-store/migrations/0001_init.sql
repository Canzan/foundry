-- crates/foundry-store/migrations/0001_init.sql
-- Slice 1 base schema for Foundry MVP.
--
-- UUIDv7: time-ordered, cache-friendly inserts. The application generates
-- UUIDs (uuid crate v7) and passes them as parameters. We do NOT install
-- the uuid-ossp extension (superuser required).

CREATE TABLE workspaces (
    id           UUID PRIMARY KEY,
    name         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- I-W1: at most one workspace per instance.
CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true));

CREATE TABLE users (
    id                 UUID PRIMARY KEY,
    email_lower        TEXT NOT NULL UNIQUE,
    email_display      TEXT NOT NULL,
    display_name       TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 64),
    password_hash      TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE workspace_memberships (
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);

CREATE TABLE teams (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    slug          TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, slug)
);

CREATE TABLE team_memberships (
    team_id     UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('lead', 'member')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE projects (
    id                 UUID PRIMARY KEY,
    team_id            UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    workspace_id       UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    slug               TEXT NOT NULL,
    key_prefix         TEXT NOT NULL CHECK (key_prefix ~ '^[A-Z]{2,6}$'),
    next_issue_number  INTEGER NOT NULL DEFAULT 1 CHECK (next_issue_number >= 1),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (team_id, slug),
    UNIQUE (workspace_id, key_prefix)
);

CREATE TABLE issues (
    id                  UUID PRIMARY KEY,
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id        UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    number              INTEGER NOT NULL,
    title               TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 256),
    description_md      TEXT NOT NULL DEFAULT '' CHECK (length(description_md) <= 262144),
    state               TEXT NOT NULL DEFAULT 'backlog'
                        CHECK (state IN ('backlog','todo','in_progress','done','cancelled')),
    priority            TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority IN ('urgent','high','medium','low','no_priority')),
    assignee_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    author_id           UUID NOT NULL REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, number)
);

CREATE INDEX idx_issues_project_state ON issues (project_id, state);
CREATE INDEX idx_issues_assignee      ON issues (assignee_id) WHERE assignee_id IS NOT NULL;

CREATE TABLE bootstrap_tokens (
    id          UUID PRIMARY KEY,
    token_hash  BYTEA NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE invites (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invitee_email   TEXT,
    created_by      UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,
    used_by         UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE outbox (
    id            BIGSERIAL PRIMARY KEY,
    event_type    TEXT NOT NULL,
    payload       JSONB NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    notified_at   TIMESTAMPTZ
);
CREATE INDEX idx_outbox_pending ON outbox (id) WHERE notified_at IS NULL;

CREATE TABLE signin_attempts (
    email_lower    TEXT NOT NULL,
    attempt_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    success        BOOLEAN NOT NULL
);
CREATE INDEX idx_signin_attempts_email_time ON signin_attempts (email_lower, attempt_at);

-- (tower-sessions table is created by tower-sessions-sqlx-store's own migrator,
--  invoked separately after this one in slice 1+.)

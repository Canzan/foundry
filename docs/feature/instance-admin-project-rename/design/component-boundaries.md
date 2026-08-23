# Component Boundaries — instance-admin-project-rename

Which crate/module owns what, and the exact port-signature deltas. Internal
structure below these signatures (helper decomposition, private functions) is
the software-crafter's to decide during GREEN/REFACTOR.

## Ownership map

| Crate / module | Owns (this feature) | Must NOT do |
|---|---|---|
| `foundry-core` | `pub fn slugify(&str) -> String` — the single production slug-derivation rule (moved from foundry-app; the two app-local copies delegate or are deleted) | Persist, validate against siblings, or know about renames |
| `foundry-services::projects` (NEW module) | `rename_project` use-case: defence-in-depth `is_instance_admin`, trim, no-op detection, D4 validation (empty / length / duplicate), the name update | Render HTML, own user-facing copy, mint sessions, check CSRF |
| `foundry-store` | Instance-wide listing read, rename-context read, sibling read, name update — SQL only, typed rows out | Validate business rules, derive slugs |
| `foundry-app::instance_admin` | Route handlers: session gate, uuid parse (fail→uniform 404), form parse, error→copy mapping, fragment/page rendering | Business validation logic, direct SQL |
| `foundry-app::views` + templates | `InstanceProjectRowView`, extended `InstanceWorkspaceRow`, row partial, error-fragment reuse | Query or compute — pure render models |
| `foundry-app::projects` | D2 fix: thread request-path slugs into `build_board_page`; create path keeps minting via `foundry_core::slugify` | Re-derive any slug from a stored name at render time (build-gated) |
| `static/js/form-errors.js` | Nothing new — reused byte-unchanged | — |

Dependency direction (unchanged, enforced by check-arch + deny.toml):
`app → services → store → core`; `app → core`.

## Port signatures

### foundry-core (domain)

```rust
/// URL-safe slug derivation. Single production definition (D2 enforcement:
/// check-arch forbids `fn slugify(` under crates/foundry-app/src).
/// Moved verbatim from foundry-app/src/projects.rs (behavior + unit tests).
pub fn slugify(input: &str) -> String;
```

### foundry-store (driven port)

```rust
/// One row of the INSTANCE-WIDE project listing (deliberately cross-tenant:
/// consumed only by the LAYER-1e allow-listed instance-admin surface).
#[derive(Debug, Clone)]
pub struct InstanceProjectRow {
    pub workspace_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub name: String,
    pub key_prefix: String,
    pub team_name: String,
}

/// Every project in the instance, ordered by project name (the handler groups
/// by workspace_id; per-workspace name order falls out of the query order).
/// SELECT p.workspace_id, p.id, p.name, p.key_prefix, t.name
///   FROM projects p JOIN teams t ON p.team_id = t.id ORDER BY p.name
pub async fn list_projects_for_instance(
    &self,
) -> Result<Vec<InstanceProjectRow>, StoreError>;

/// What the rename use-case needs to know about the target before validating.
#[derive(Debug, Clone)]
pub struct ProjectRenameContext {
    pub team_id: uuid::Uuid,
    pub current_name: String,
    pub slug: String,
}

/// SELECT team_id, name, slug FROM projects WHERE id = $1
pub async fn project_rename_context(
    &self,
    project_id: uuid::Uuid,
) -> Result<Option<ProjectRenameContext>, StoreError>;

/// The D4 uniqueness comparison set: (name, slug) of every OTHER project in
/// the same team. Comparison happens app-side (case-insensitive name match +
/// foundry_core::slugify collision) because slug derivation is domain code,
/// not SQL.
/// SELECT name, slug FROM projects WHERE team_id = $1 AND id <> $2
pub async fn list_team_sibling_projects(
    &self,
    team_id: uuid::Uuid,
    exclude_project_id: uuid::Uuid,
) -> Result<Vec<(String, String)>, StoreError>;

/// UPDATE projects SET name = $2 WHERE id = $1 — name ONLY (D1: slug,
/// key_prefix, next_issue_number untouched). Returns rows_affected so the
/// caller can map a vanished project (0) to the non-enumerable NotFound.
pub async fn update_project_name(
    &self,
    project_id: uuid::Uuid,
    name: &str,
) -> Result<u64, StoreError>;
```

`list_projects_for_workspace` is left byte-untouched (shipped consumer, distinct
tenant-scoping contract).

### foundry-services (use-case seam)

```rust
pub mod projects {
    pub struct RenameProjectRequest<'a> {
        /// Session-resolved actor — re-gated by is_instance_admin inside
        /// (defence-in-depth, mirrors provisioning::provision_workspace).
        pub acting_user_id: uuid::Uuid,
        pub project_id: uuid::Uuid,
        /// Raw form input; trimmed inside the use-case.
        pub new_name: &'a str,
    }

    pub enum RenameOutcome {
        /// Name persisted. Carries the trimmed stored name for the fragment.
        Renamed { name: String },
        /// Trimmed input byte-equal to the current name — nothing written (D4).
        NoOp { name: String },
    }

    pub enum RenameProjectError {
        /// Actor is not an instance admin → handler renders uniform 404.
        Forbidden,
        /// Unknown project id (or lost race with a delete) → uniform 404.
        NotFound,
        /// Trimmed name empty → 422 "Project name must not be empty".
        EmptyName,
        /// > 256 chars (Unicode scalar count, mirroring the issues.title
        /// CHECK semantics) → 422 "Project name must be at most 256 characters".
        NameTooLong,
        /// Case-insensitive name match OR slugify(new) == sibling stored slug,
        /// self excluded → 422 "Project name must be unique within the team".
        DuplicateName,
        Store(foundry_store::StoreError),
    }

    /// Order of checks (pins the observable 422 precedence):
    /// is_instance_admin → context fetch → trim → no-op → empty → length →
    /// duplicate → update. Check-then-write; TOCTOU accepted (data-models §4).
    pub async fn rename_project(
        store: &foundry_store::Store,
        request: RenameProjectRequest<'_>,
    ) -> Result<RenameOutcome, RenameProjectError>;
}

impl Services {
    /// Delegates to projects::rename_project (the provisioning idiom).
    pub async fn rename_project(
        &self,
        request: projects::RenameProjectRequest<'_>,
    ) -> Result<projects::RenameOutcome, projects::RenameProjectError>;
}
```

Duplicate rule detail (D4): with `t = trimmed new name`, refuse when any sibling
`(name, slug)` satisfies `t.to_lowercase() == name.to_lowercase()` **or**
`foundry_core::slugify(t) == slug`. Self is excluded by the query, so a
case-only rename of a project onto itself ("sandbox" → "Sandbox") is a valid
rename, and an exact-match self rename is the earlier NoOp.

### foundry-app (driving adapter)

```rust
// instance_admin.rs
#[derive(Debug, Deserialize)]
pub struct RenameForm {
    pub name: String,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>, // enforced by csrf_middleware before the handler
}

/// POST /admin/instance/projects/{project_id}/rename
/// Path is Path<String>, parsed to Uuid IN the handler: a malformed id renders
/// the SAME uniform 404 as a non-admin — no 400-vs-404 enumeration oracle
/// (axum's default Path<Uuid> rejection would leak a 400).
pub async fn submit_project_rename(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    session: Session,
    headers: HeaderMap,
    axum::extract::Form(form): axum::extract::Form<RenameForm>,
) -> Response;
```

```rust
// views.rs
/// One project row on the instance dashboard — rendered by the dashboard loop
/// AND returned verbatim as the rename-success fragment (one-partial rule).
#[derive(Debug, Clone, Template)]
#[template(path = "partials/instance_project_row.html")]
pub struct InstanceProjectRowView {
    pub project_id: String,   // data-project-id + the form's action URL
    pub name: String,         // visible + pre-fills the rename input (escaped)
    pub key_prefix: String,
    pub team_name: String,
    pub csrf: String,         // hidden _csrf in the row's rename form
}

pub struct InstanceWorkspaceRow {
    pub workspace_id: String,
    pub name: String,
    pub projects: Vec<InstanceProjectRowView>, // NEW; empty ⇒ data-project-empty
}
```

Route registration (lib.rs, inside the existing HTML mount — under
`csrf_middleware` + `session_layer`, mirroring `/admin/tokens/{jti}/revoke`):

```rust
.route(
    "/admin/instance/projects/{project_id}/rename",
    post(instance_admin::submit_project_rename),
)
```

Row form markup contract (row partial; the acceptance scraper seam):

```html
<li data-project-row data-project-id="{{ project_id }}">
  <span data-project-name>{{ name }}</span> ({{ key_prefix }}) — team {{ team_name }}
  <form hx-post="/admin/instance/projects/{{ project_id }}/rename"
        hx-target="closest [data-project-row]" hx-swap="outerHTML">
    <input type="hidden" name="_csrf" value="{{ csrf }}">
    <label>Rename <input type="text" name="name" value="{{ name }}" required></label>
    <div data-error-slot></div>
    <button type="submit">Rename</button>
  </form>
</li>
```

# ADR-002: Archive container + manifest header format (OD-PWB-3)

## Status
Accepted

## Context
The export needs a portable container holding one workspace's rows across 10 tables, with a
self-describing header so `verify-export <path>` round-trips using ONLY the path (NFR-PWB-INT-01).
The whole-instance backup uses `pg_dump -Fc` (`admin_cli.rs::run_backup_verify`), but `pg_dump`
cannot scope by a column — it dumps whole tables. A per-workspace export is therefore necessarily a
logical, SELECT-based serialization, and the container shape is ours to define. The ACs assert
"a file exists at <path>" and atomic writes via rename.

## Decision
A **single tar file** containing:
- `manifest.json` — the self-describing header (read first by verify).
- `tables/<table>.jsonl` — one JSON object per row via the shipped `to_jsonb(t.*)::text` idiom
  (slice-05 `snapshot_tenant_tables`), one file per tenant table.

`manifest.json` schema: `{ format_version, declared_workspace_id, declared_workspace_name,
exported_at, tenant_tables[], row_counts{} }`. `declared_workspace_id` is what verify reads to know
which workspace to check isolation against — no out-of-band argument.

## Alternatives Considered
1. **Scoped `pg_dump` variant.** Rejected: `pg_dump` cannot filter by a `workspace_id` column;
   a `--table` selection still dumps ALL rows of that table (every tenant). No isolation possible
   without post-filtering, which defeats the guarantee.
2. **A bare directory of JSONL files (no tar).** Rejected: a directory is not "a file at <path>"
   (the ACs), and a directory tree cannot be created atomically with a single rename — the
   atomicity guarantee (NFR-PWB-ATOM-01) needs a single inode to rename.
3. **A single combined JSON document (all tables in one object).** Rejected: forces the whole
   export into memory and loses the per-table streaming that lets verify count lines cheaply against
   the manifest `row_counts`. JSONL streams row-by-row.
4. **SQLite file as the container.** Rejected: introduces a new dependency and a second schema to
   keep in sync with Postgres; JSONL via `to_jsonb` reuses the EXACT shipped idiom with zero schema
   duplication.

## Consequences
- Positive: reuses the slice-05 whole-row idiom verbatim; export and the migration-guarantee proof
  speak the same row language.
- Positive: self-describing header -> path-only verify (NFR-PWB-INT-01).
- Positive: single tar inode -> atomic rename (NFR-PWB-ATOM-01).
- Negative: introduces a tar dependency (the `tar` crate, MIT/Apache — OSS, mature). Software-crafter
  picks the exact crate; design pins the SHAPE.
- Negative: `exported_at` makes byte-equality unstable — gold tests assert on counts/tables/isolation,
  not manifest bytes (noted in wave-decisions.md residual #3).

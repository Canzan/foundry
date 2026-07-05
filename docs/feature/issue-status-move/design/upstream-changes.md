# Upstream Changes — issue-status-move (DESIGN)

No DISCUSS assumption contradicted. ODD-1..4 (open FOR DESIGN) are resolved (ADR-001/002, user-ratified). Two
small refinements surfaced, recorded in architecture.md: (1) the board card gains a stable `id="issue-{key}"`
so the dialog's OOB `delete` can target the old card during a column move; (2) the card exposes its state-post
URL/slug via `data-*` for the DnD handler — both reuse the slugs+number already surfaced by issue-edit-dialog.
Neither changes a DISCUSS acceptance criterion.

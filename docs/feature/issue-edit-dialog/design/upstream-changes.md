# Upstream Changes — issue-edit-dialog (DESIGN)

No DISCUSS assumptions were contradicted by DESIGN. The DISCUSS ODD-1..4 were left open FOR DESIGN and are
now resolved (ADR-001/002, user-ratified). One refinement surfaced: the board card render must expose the
issue `number` (not just key+title) so the edit URL can be built — analogous to the board-slug surfacing in
board-new-issue. This is a small card-view field addition, recorded in `architecture.md` § Web; it does not
change any DISCUSS acceptance criterion.

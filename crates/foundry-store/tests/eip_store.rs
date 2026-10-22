//! external-integration-plugins — store gold tests (DESIGN [REF] Driven Ports
//! and Contract Shapes, ADR-EIP-002; DEVOPS eip-store, C-9, C-12, U-D7, U-D12).
//! Pending stubs written by DISTILL 2026-10-21 (ADR-025 scaffolded RED): each
//! is `#[ignore]`d with its DELIVER step id and panics with the RED marker until
//! DELIVER replaces its body (testcontainers Postgres, like the neighbouring
//! store tests; Docker required once un-ignored).

macro_rules! pending {
    ($name:ident, $step:literal, $what:literal) => {
        #[test]
        #[ignore = concat!("pending: external-integration-plugins ", $step, " ", $what)]
        fn $name() {
            panic!(concat!(
                "Not yet implemented -- RED scaffold (external-integration-plugins ",
                $step,
                ": ",
                $what,
                ")"
            ));
        }
    };
}

// Migrations 0028-0030 under rolling upgrade (slice 01).
pending!(board_writes_continue_while_0028_applies, "01-02", "U-D12: concurrent old-writer outbox inserts stall at most lock_timeout + 1 s while 0028 applies");
pending!(
    rows_from_before_0028_are_never_dispatched,
    "01-02",
    "rows at or below integration_dispatch_start and NULL event_id rows never become deliveries"
);
pending!(old_writer_rows_after_0028_get_event_and_txid, "01-02", "C-12: a pre-0028 writer's row after 0028 has non-NULL event_id and source_txid and is dispatched; R-6's UPDATE marks exactly those");
pending!(
    previous_release_migrator_accepts_0028_to_0031,
    "01-02",
    "C-9: the rollback floor; an older binary's Store::migrate boots on the newer schema"
);
pending!(
    probe_refuses_a_schema_without_0028_to_0030,
    "01-02",
    "Store::probe fails readiness until the new columns and tables exist"
);
pending!(
    invalid_0029_index_is_reported_at_boot,
    "01-02",
    "U-D7: a failed CREATE INDEX CONCURRENTLY leaves indisvalid = false and boot warns"
);

// Dispatch and claims (slice 01).
pending!(
    rolled_back_board_transaction_yields_no_delivery,
    "01-02",
    "SC-2: an outbox row from an aborted transaction is never dispatched"
);
pending!(
    dispatch_rerun_is_a_no_op,
    "01-02",
    "a second dispatcher tick over the same rows inserts nothing and changes no dispatched_at"
);
pending!(
    dispatch_never_splits_a_source_transaction,
    "01-02",
    "all rows of one source_txid are expanded in one dispatch transaction"
);
pending!(
    claim_compare_and_set_misses_a_changed_row,
    "01-02",
    "DeliveryClaims::claim applies claimable_head's id only if the row is unchanged"
);
pending!(
    late_recorder_writes_nothing,
    "01-02",
    "a record whose lease was taken over (claim_id moved on) changes no row"
);
pending!(sweeps_apply_exactly_the_plan_ids, "03-01", "expire, cancel and prune apply one compare-and-set over exactly the ids their plan function returned");
pending!(route_and_choice_writers_take_the_live_project_lock, "01-02", "route and email-choice writers open with lock_live_project; a deleted project refuses the write");
pending!(
    rotate_secrets_reseals_every_credential_or_nothing,
    "01-07",
    "C-11: plaintexts unchanged after open, row count unchanged, a mid-way failure changes nothing"
);

//! Postgres timestamp precision for acceptance assertions.

use time::OffsetDateTime;

/// Truncate `t` to the microsecond resolution of Postgres `timestamptz`.
///
/// A value round-tripped through the database loses its sub-microsecond
/// nanoseconds (sqlx truncates, it does not round), while an in-memory clock
/// value keeps them on Linux. Compare the two only after passing the in-memory
/// side through this. Same rule as the store-test fix in 6b6bc0c; see
/// `docs/feature/fix-invite-ttl-precision/rca.md`.
pub fn to_pg_micros(t: OffsetDateTime) -> OffsetDateTime {
    t.replace_nanosecond(t.microsecond() * 1000)
        .expect("microsecond*1000 is a valid nanosecond value")
}

# RCA — invite "valid for 7 days" assertion fails on Linux CI

**Symptom** (CI run 36276820914, commit 0cd73c3): `feature_member_invites.rs`
`sees_confirmation_link_7_days` — `expires_at = …29.817272`, `issued_at =
…29.817272728`, `ttl_days = 6`.

**Root cause chain**
1. `ttl.whole_days() == 6`: `expires_at − issued_at = 7d − 728ns`, floored.
2. `expires_at` is read back from Postgres `timestamptz` (µs; sqlx truncates),
   `issued_at` is the in-memory `MockClock` value (ns).
3. The Background seeds the clock from `OffsetDateTime::now_utc()`: ns on Linux,
   whole µs on macOS, so it never fails locally.
4. The assertion compares two precisions and floors the difference.
5. The µs round-trip rule was fixed once (6b6bc0c, store test) but never
   generalised to the acceptance steps. CI has been red for months (1 green of
   the last 200 runs), so the failure was never triaged.

**Production:** not affected. The invite HMAC signs whole seconds
(`foundry-auth/src/lib.rs:388`); the expiry check `expires_at > now` is off by
<1µs at most.

**Fix (user-approved 2026-09-26, option A):** truncate `issued_at` to
microseconds in the step via a shared acceptance helper and assert the exact
7-day TTL. Regression: seed the Background clock with deliberate sub-µs nanos so
the scenario fails on every OS before the fix.

**Follow-up (user-approved):** after this lands, investigate CI health (45-min
timeout cancellations, `us-r05-attachments.feature:55`) as a separate bugfix.

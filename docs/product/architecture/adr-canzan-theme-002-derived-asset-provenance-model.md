# ADR-CANZAN-THEME-002: A derived asset records its recipe as provenance, and its two verifiable claims are separated

## Status

Accepted (canzan-theme-system DESIGN wave, 2026-08-29)

## Context

`crates/foundry-app/static/VENDOR.md` makes one integrity promise, and it is
stated in terms of byte-identity with an upstream release (`:5-12`):

> Each blob below is the upstream-published, pre-minified artifact, committed
> verbatim and served by the binary via `tower_http::services::ServeDir` […]
> An auditor (or an air-gapped operator) can verify each file matches the named
> upstream release by re-computing its sha256 and comparing the value recorded
> here.

Two row shapes exist today and both satisfy it. **Vendored-verbatim**
(`vendor/htmx.min.js`, `:16`) records version + upstream URL + retrieval date +
sha256; the audit is "re-download, re-hash, compare". **Authored-in-tree**
(`css/foundry.8ce38566.css`, `:17`) records `hand-authored (this repo)` and a
sha256; there is no upstream, and the hash is also the filename, so the audit is
"re-hash, compare with the name".

ADR-CANZAN-THEME-001 introduces a shape neither covers: a blob that **is**
derived from a named upstream release but is **not byte-identical** to it. A
subsetted, axis-instanced woff2 will never match any sha256 Google or the
foundry publishes. Recomputing it and comparing against upstream fails by
design. The promise at `:9-12` would be false for three of the repo's assets,
and — worse than a false promise — it would be *silently* false, since nothing
tells the auditor which model applies to which row.

This matters more than usual here. `VENDOR.md`'s audit story is one of the
project's stated quality attributes (air-gap-friendly, self-hosted, single
operator), and feature-delta's Shared Artifacts table rates font provenance
**HIGH** risk: *"VENDOR.md's entire purpose. An unrecorded blob breaks the audit
silently."*

A complication discovered while pinning the upstreams: **Bricolage Grotesque has
no releases and no tags.** The authoritative repo
(`github.com/ateliertriay/bricolage`) is dormant, HEAD still at commit
`84745e5b96261ae5f8c6c856e262fe78d1d6efdd` (2023-07-19). There is no "version" to
record in the `Version` column. Public Sans (`v2.001`, 2022-05-11) and JetBrains
Mono (`v2.304`, 2023-01-14) do have tags.

A second complication: the mirrored copies under `google/fonts` are accompanied
by `upstream_info.md` files that are themselves **AI-generated audit notes** (one
is headed "Model: Claude Opus 4.6"). They are not a provenance source.

## Decision

Add a **third row shape** to `VENDOR.md` — *derived* — and state plainly, in the
document's own preamble, that the file now carries three shapes with three
different audit procedures. A derived asset's provenance is **the recipe**, not
the byte.

**The derived row records seven things**, in a per-blob block beneath the table
(the recipe does not fit a table cell, and pretending it does is how recipes
drift from reality):

1. **Family and licence** — `OFL-1.1`, plus the in-tree path of the licence text
   shipped with it.
2. **Pinned input** — the *authoritative* upstream repository at an **immutable
   ref**: a release tag where one exists (`uswds/public-sans@v2.001`,
   `JetBrains/JetBrainsMono@v2.304`) and a **commit sha** where none does
   (`ateliertriay/bricolage@84745e5b96261ae5f8c6c856e262fe78d1d6efdd`). Never a
   branch name; never the `google/fonts` mirror.
3. **sha256 of the input**, as downloaded from that ref.
4. **Tool and exact version** — `fonttools 4.63.0`, `brotli`, `python 3.14.7` —
   because woff2 output is compressor-dependent and a recipe without a pinned
   compressor is not a recipe.
5. **The exact command line**, verbatim, every flag, the full unicode-range.
6. **sha256 of the intermediate** instanced-and-subset TTF, *before* woff2
   flavouring (see below — this is the load-bearing one).
7. **sha256 of the committed output** woff2.

**The recipe is executable, and the executable is the source of truth.** It lives
at `tools/fonts/derive-fonts.sh` with a pinned `tools/fonts/requirements.txt`,
outside the cargo workspace and outside `static/` (a served directory must not
contain a shell script). `VENDOR.md` names the script and transcribes the
resulting hashes; the script is what an auditor runs. Prose alone drifts from
reality; a script alone is undiscoverable. This is **not a build step** — it is
never invoked by `cargo build`, by `xtask ci`, or by CI, and runs only when a
maintainer adds or bumps a font (`VENDOR.md:3-4`, assets.md DB6 preserved).

**The two claims are separated, and only one of them is promised.**

> **Tier 1 — Integrity. Unconditional, machine-checked, always true.**
> The committed blob's sha256 equals the value recorded here. This answers *"is
> the file this binary serves the file that was reviewed?"* and it is the claim
> that protects the operator. It is enforced by `cargo xtask check-arch` rule R3
> (ADR-CANZAN-THEME-003), so a row that stops being true reds CI.
>
> **Tier 2 — Provenance. Conditional, human-run, best-effort.**
> Re-deriving from the pinned input with the pinned tool *should* reproduce the
> blob. This answers *"is this blob really Bricolage Grotesque, and only that?"*
> **Byte-identical re-derivation is expected but NOT guaranteed** across brotli
> builds, platforms and fonttools patch releases. We do not claim it.

Because Tier 2 can legitimately fail on an honest auditor's machine, the audit
procedure is **three-step, with a documented fallback** — which is what the
intermediate TTF hash (item 6) exists to enable:

1. Download the input from the pinned ref; verify its sha256 against item 3. *If
   this fails, stop — the upstream moved or the record is wrong.*
2. Run the instancing and subsetting **without** `--flavor=woff2`; compare
   against item 6. This step is brotli-independent and therefore far more stable
   than comparing compressed output. **A match here proves the font content is
   exactly what was recorded** — same glyphs, same axes, same tables.
3. Flavour to woff2 and compare against item 7. A match is full byte-level
   provenance. **A mismatch after step 2 passed is a compressor difference, not a
   provenance failure**, and is explicitly recorded as such rather than treated
   as tampering.

**And the reproducibility assumption is itself probed, not assumed.** DELIVER
re-runs `derive-fonts.sh` in a **second, materially different environment** and
records in `VENDOR.md`, as measured fact, whether steps 2 and 3 reproduced
byte-for-byte.

"Materially different" means *the compiled brotli and the libc differ* — that is
the variable under test, not the physical host. **A container is sufficient and
is the expected form**: `docker run --rm -v "$PWD:/w" -w /w python:3.14-slim`
followed by the same `requirements.txt` install and the same script. This matters
because a one-person homelab team does not have a second workstation on demand,
and a probe that is impractical is a probe that will be skipped — which would
leave the assumption exactly as unexamined as if it had never been written down.
A second physical machine is *better* (it also varies the CPU and filesystem) but
is not required, and requiring it would trade a real measurement for an ideal one
that never happens.

If step 3 varies, that is written down beside the row so the next auditor is not
alarmed by it. If **step 2** varies, the intermediate hash is not the stable
anchor this ADR assumes and the model must be revisited — that is the finding
worth stopping for. Either way the document states what was observed, not what
was hoped.

**Licence text ships with the fonts.** OFL-1.1 clause 2 requires the licence to
accompany redistribution. Each family's `OFL.txt` is committed as
`crates/foundry-app/static/fonts/OFL-<family>.txt` and named in its row. Serving
it is the most literal compliance and costs nothing on a cold page load, since
nothing references it. All three families were verified to carry **no Reserved
Font Name** (ADR-CANZAN-THEME-001 § Context), so the derived blobs keep the
upstream family names in `@font-face` without a renaming obligation.

## Consequences

- Positive: `VENDOR.md` stops making one promise it cannot keep for three assets
  and starts making two promises it can, each labelled with its strength. The
  weaker promise is the one that was previously implicit and unexamined.
- Positive: Tier 1 becomes *stronger* than today for every row, not just the new
  ones — R3 machine-checks that recorded hashes recompute, converting a DoD
  checklist line ("every blob's recorded sha256 recomputes") into a CI gate.
- Positive: the model generalises. Any future derived asset — a subset icon font,
  a pre-compressed blob, a regenerated sprite — has a shape to use, and the
  "which audit applies here?" question is answerable from the row itself.
- Negative: a derived row is roughly ten lines instead of one table row, and
  `VENDOR.md` grows a per-blob section beneath its table. Accepted: the
  alternative is a recipe that does not fit and is therefore abbreviated into
  uselessness.
- Negative: a full Tier 2 audit now requires a Python toolchain at pinned
  versions, which an air-gapped operator may not have. This is a real reduction
  in audit accessibility versus a verbatim blob, and it is the price
  ADR-CANZAN-THEME-001 incurs. Mitigated, not removed: Tier 1 needs only
  `shasum` and is what an air-gapped operator actually needs to detect tampering;
  Tier 2 is an origin question, answerable once by anyone, not per-deployment.
- Negative: Bricolage's pin is a commit sha on a dormant repository with no
  release process. If that repo disappears, the input is unverifiable and only
  Tier 1 survives. Recorded as a standing risk; the mitigation if it fires is
  ADR-CANZAN-THEME-001 alternative B (drop the display face).
- **Stated dependency: the intermediate anchor assumes `--flavor=woff2` changes
  only compression, never font content.** The three-step audit rests on this — if
  woff2 flavouring could alter glyph structure, tables or hinting, then a matching
  intermediate hash would no longer prove the shipped blob's content and step 2
  would be a weaker claim than advertised. The assumption holds for `fonttools
  4.63.0`, where flavouring is a container/compression operation, and the version
  is pinned in `tools/fonts/requirements.txt` precisely so it cannot drift
  underneath the model. It is named here rather than left implicit because a
  future tool bump could invalidate it silently. Mitigations, in order: the pinned
  version; the second-environment probe, which would surface a step-2 divergence
  as the loud failure this ADR already says is "worth stopping for"; and the
  standing rule that bumping the toolchain means re-running the audit, not
  assuming it still holds. If a future fonttools release changes what flavouring
  does, this ADR is revisited — not worked around.
- Negative: three hashes per blob are three more hand-transcribed values. R3
  checks item 7 (the committed output) automatically; items 3 and 6 name
  artefacts that do not exist in the tree and so cannot be machine-checked here.
  They are verifiable only by running the audit — which is the point, but it
  means they can rot silently. Recommended follow-up: have `derive-fonts.sh`
  *emit* the row block rather than have a human transcribe it.

## Alternatives considered

- **A. Record only the output sha256 and drop the provenance claim** — Rejected.
  It is honest and it is what Tier 1 already gives, so it costs nothing to
  implement. But it reduces a font to an anonymous 30 KB binary blob: an auditor
  could verify the file had not changed since review while having no way to
  establish what it *is*. For a project whose vendoring document exists
  specifically so "an auditor or an air-gapped operator" can answer that
  question, deleting the question is not an answer.
- **B. Commit the upstream source TTFs alongside the derived woff2** — Rejected,
  though it is the most audit-friendly option and was seriously considered: it
  makes item 3 checkable entirely in-tree and would suit the air-gap posture. It
  costs ~850 KB of repository weight (Bricolage's source TTF alone is 408,496 B)
  for assets never served. Decisive objection: it does **not** actually achieve
  offline audit, because step 2 still requires the pinned fonttools toolchain —
  a far larger download than the fonts. It would buy 850 KB of repo for a
  fraction of one step of the procedure.
- **C. Claim byte-reproducible builds and require step 3 to match** — Rejected.
  It is the cleanest-sounding model and it is what a reader would assume from the
  existing `VENDOR.md` wording. Rejected because we have not demonstrated it, and
  a verification step that fails for honest reasons trains auditors to ignore
  failures — which is strictly worse than a weaker claim honestly labelled. The
  ADR replaces the assumption with a measurement (the second-machine probe); if
  that measurement comes back byte-identical on both machines, the claim can be
  strengthened later on evidence rather than asserted now on hope.
- **D. Fold derived blobs into the existing "authored-in-tree" shape** — Rejected.
  It requires no new shape and no new procedure, which is its entire appeal. But
  "hand-authored (this repo)" is false for a font nobody here drew, it discards
  the upstream pin, and it would place three OFL-licensed third-party works under
  a label that implies foundry authored them — a licensing misstatement, not just
  an imprecision.
- **E. Regenerate the fonts in CI from the pinned upstream, committing no blob** —
  Rejected outright. It is a build step (DB6, permanently out of scope), it makes
  every build depend on network reachability of a dormant GitHub repo, and it
  makes the served bytes a function of whichever toolchain the runner resolved —
  destroying the integrity claim it was meant to strengthen.

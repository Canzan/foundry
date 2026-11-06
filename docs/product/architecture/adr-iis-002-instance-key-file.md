# ADR-IIS-002: Without an operator instance key, Foundry makes one key file in its data directory, create-if-absent, and never overwrites it

## Status

Proposed (instance-integration-setup DESIGN, 2026-11-05). The options were chosen by the user on
2026-11-05 (`docs/feature/instance-integration-setup/design/options.md`, decisions 1b, 1c with its
refinement, 1d, and 7's key-file item). Builds on ADR-IIS-001.

## Context

- D6: a standalone deploy (compose or single binary) with no operator key still never stores keys
  in plaintext. Foundry makes a random instance key file outside the database, readable only by its
  user. Servers starting together on a shared volume make exactly one. An unwritable directory
  degrades (I-27). A server whose file differs says so (I-26).
- AGENTS.md "Deployment independence": the fallback must work on any standalone deploy and name no
  environment.
- The image is distroless and runs as `nonroot` with `WORKDIR /app` (root-owned). The k8s example
  runs 3 replicas with `readOnlyRootFilesystem` and a per-pod `emptyDir`. A per-pod file would give
  each replica a different key.
- Known substrate lies: overlayfs `fsync` no-ops, network volumes with weak locks, short reads
  during a concurrent write.

## Decision

1. **Location.**
   - `FOUNDRY_DATA_DIR`, default `/var/lib/foundry`. File `instance.key`, mode 0600.
   - Content is one line of base64url of 32 random bytes, the operator-key format, so an operator
     can later copy it into `FOUNDRY_INSTANCE_KEY` unchanged.
   - Generated through `ring`'s `SystemRandom`, like every Foundry key.
2. **Create-if-absent.**
   - Open a temp file `instance.key.tmp.{random}` in the same directory with
     `O_CREAT|O_EXCL|O_WRONLY` and mode 0600 **at open**, so the key is never readable by another
     user, not even briefly.
   - Write the key, `fsync` the file, then `link(2)` it to `instance.key`. `EEXIST` means another
     server won.
   - Then `fsync` the directory, remove the temp file, and read back whichever file exists.
   - POSIX, atomic, never exposes a partial file.
   - At boot, before creating, remove any leftover `instance.key.tmp.*` from a crashed start.
     `instance.key` itself is never touched.
   - **`link(2)` unsupported.** On a filesystem without hard links, `link` fails with `EPERM` or
     `ENOTSUP`. That is treated as "cannot be written": mode `NoInstanceKey(Unwritable)`, I-27, and
     the temp file is removed. The warn line names the error kind, never a key.
   - The probe asserts, before use, that `instance.key` is a regular file of mode 0600 (no group or
     other bits) owned by the running user, and that it parses.
     - A file this server just created that fails the check is `Unwritable` (I-27).
     - A pre-existing file with wider bits is used but logged as a warning, and is never chmod-ed or
       rewritten.
3. **Do not mint a useless key.**
   - No file is created when the database already holds a secrets key, whatever wrapped it, or any
     value sealed directly under an instance key. A new file could never open them.
   - The server reports `Unopenable` (I-26) or I-4 instead. Per-replica volumes therefore fail
     loudly on every replica but the first.
   - The file is read or created **at boot only**. Reload reuses the held key, so a file deleted
     while the server runs is never re-minted.
4. **Never destroy a key.**
   - A file that exists but cannot be read or parsed is never overwritten.
   - Mode `NoInstanceKey(Unreadable)` shows new copy I-31.
   - An unwritable directory gives `NoInstanceKey(Unwritable)` and shows I-27.
   - The board serves in both cases; only a malformed operator setting refuses start.
5. **Operator key wins.** With an operator key set and a readable file present:
   - the file's key is held as a previous instance key;
   - at boot, a compare-and-set on `wrapped_by_key_id` re-wraps a file-wrapped secrets key under
     the operator key and stores the file path in `rewrapped_from_path`;
   - the replica that wins records the I-18 system line and bumps the config version;
   - the page shows I-30 while `rewrapped_from_path` is set and the operator key is in effect.

   A replica still on the file keeps the secrets key it already holds. A reload never re-unwraps a
   key whose id is unchanged (ADR-IIS-004).
6. **One adapter.**
   - `integrations/key_file.rs` is the only code that reads `FOUNDRY_DATA_DIR` or touches the file.
   - Its `probe()` writes, links, reads back and parses before the key is used.
   - Fault-injection gold tests cover (in addition to the cases below: a leftover temp file, the
     `link` `EPERM` path, and the mode assertion):
     - two servers racing on one directory;
     - a read-only directory;
     - a short read;
     - a corrupt file;
     - an existing file with the wrong mode, which is used but logged as a warning, never
       rewritten.

## Enforcement

- check-arch `integrations-key-file-seam`: `FOUNDRY_DATA_DIR`, and filesystem calls on the key
  path, appear only in `crates/foundry-app/src/integrations/key_file.rs`. An injected-violation gold
  test plants a read in a handler.
- The adapter's probe and its fault-injection tests (rule 6) are the behavioural layer.

## Alternatives considered

- **A relative `./data` default** (1b-B). Rejected: the image's `/app` is root-owned, and the
  meaning depends on the launch directory.
- **No default, file only when the setting is set** (1b-C). Rejected: breaks D6 and KPI-IIS-6.
- **`O_CREAT|O_EXCL`, then write** (1c-B). Rejected: a racing reader can see an empty or partial
  file.
- **`flock` a lock file** (1c-C). Rejected: unreliable on network volumes.
- **Re-wrap lazily on the first page view** (1d-B). Rejected: leaves a mixed state with no fixed
  moment for the system line.

## Consequences

- Positive: zero-config standalone deploys get integrations with one click. Moving to an operator
  key loses nothing.
- Negative: an operator who recreates the volume loses the key. I-25 says to back up the file
  apart from the database.
- DEVOPS:
  - The image creates `/var/lib/foundry` owned by `nonroot` and declares it a volume.
  - `docker-compose.yml` mounts a named volume there.
  - The k8s example and its README say to set `FOUNDRY_INSTANCE_KEY` in the Secret (or give every
    replica one shared volume), because a per-pod volume gives each pod its own key.

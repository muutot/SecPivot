# KeePass dependency patch

SecPivot keeps `keepass = 0.13.22` and applies a local Cargo patch from
`keepass-0.13.22/`. The complete published package and its MIT license remain
present. It is the crates.io archive with SHA-256
`a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e`, matching
the previously locked registry checksum. The original source is available at
<https://static.crates.io/crates/keepass/keepass-0.13.22.crate>.

Only these upstream files are changed:

- `src/db/types/entry.rs`: lease attachments until tracked snapshots are appended,
  remove only the addressed current/history reverse reference, and reindex after
  snapshot insertion. Entry removal honors prepared merge leases and visits each
  binary ID once even when filenames alias identical payloads. Dedicated tests
  cover shared attachments and guard unwind. After a tracked removal, dropping
  the snapshot lease collects only its released, unowned binary IDs. Recursive
  current/history owners and remaining leases protect shared payloads; unrelated
  orphan headers are preserved. `content_eq` excludes timestamps,
  history and current/previous parent bookkeeping; callers check expiration
  separately when it forms part of version identity.
- `src/db/merge.rs`: merge newer same-parent location time and previous-parent
  provenance independently of content. Relocations and newly created groups keep
  the source move's previous parent, including after cycle-safe group staging.
  Content equality ignores both parent locations; true equal-time content
  conflicts still fail. Different-parent moves with missing timestamps retain
  the existing destination and never borrow a time for an unapplied move.
  Group deletion is evaluated after entry and group relocation, from children
  to ancestors. Only an empty group may be removed; surviving entries/groups
  restore their required ancestors. Each object's own deletion timestamp is
  independent, retained ancestors lose obsolete markers, and Deleted events
  describe actual removals. Equal/missing-time policy is otherwise preserved.
- `src/db/types/mod.rs`: track ephemeral leases separately from persisted data,
  clear leases on database clone, rebuild current/history ownership, and collect
  only payloads affected by an explicit entry/history removal. Loading preserves
  otherwise unreferenced header data. It also prepares canonical binary IDs for
  merge, applies actual entry winners, copies complete recycle-bin version maps
  and rejects missing payloads before mutation. Exact bytes and protection flags
  determine identity. Prepared leases protect pending merge owners until winner
  maps are installed; existing leases reject preparation. History::from_entries
  constructs an ordered snapshot list once and strips nested histories, avoiding
  repeated front insertion in the first-party history union.
- `src/format/xml_db/mod.rs`: reconstruct attachment ownership on load and map
  every current/history `Binary Ref` to the exact dense inner-header position on
  save. Missing references fail before destination publication.

The other 56 upstream files are byte-for-byte unchanged. The provenance JSON
records each original hash and the four reviewed patched hashes. Run
`npm run verify:keepass-patch` after any patch edit; it verifies source hashes,
Cargo wiring and formatting of the patched Rust files. Update the four patched
hashes only after reviewing the changes against the original archive. The
ordinary `npm run verify` includes this gate.

`keepass-local.patch` is a zero-context diff against the original package.
Applying it with `git -c core.autocrlf=false apply --unidiff-zero` inside a fresh
original package reconstructs all 60 locally verified files. The explicit Git
option preserves the upstream LF bytes on Windows; automatic CRLF conversion
does not satisfy the hash check. The provenance check remains the source of
truth for byte identity.

Native application regressions cover replacement/removal, history deletion and
retention, lease lifetime, sparse IDs, rejected writes and KDBX save/reopen.
The fourteen dependency-specific tests can be run separately:

```powershell
cargo test -j 1 --manifest-path src-tauri/vendor/keepass-0.13.22/Cargo.toml --target-dir src-tauri/target --locked --features _merge,save_kdbx4 --lib secpivot_
```

An upgrade alone is not a verified fix: the inspected 0.13.25 and 0.17.0 packages
retain the relevant old attachment-removal and tracking logic. Remove this local
patch only after an upstream version passes the same integrity regressions.
History restoration of the whole attachment set is a separate application
contract and must not be inferred from this ownership fix.

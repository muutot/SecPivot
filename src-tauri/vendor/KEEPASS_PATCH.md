# KeePass dependency patch

SecPivot keeps `keepass = 0.13.22` and applies a local Cargo patch from
`keepass-0.13.22/`. The complete published package and its MIT license remain
present. It is the crates.io archive with SHA-256
`a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e`, matching
the previously locked registry checksum. The original source is available at
<https://static.crates.io/crates/keepass/keepass-0.13.22.crate>.

Only these upstream files are changed:

- `src/crypt/kdf.rs`: reject unrepresentable Argon2 iteration and KiB memory
  values with checked u32 conversions before derivation. Existing Argon2 errors
  carry the failure instead of silently reducing the cost by integer truncation.
  Direct tests cover Argon2d and Argon2id; normal parameters retain their values.
  `KdfConfig::benchmark()` times the same seeded KDF transform with fixed synthetic
  key/salt and black-boxes its result. This exposes credential-free calibration
  without duplicating cryptography; the additional test executes AES/Argon2d/id
  and verifies invalid cost propagation.

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
  maps are installed; existing leases reject preparation. Preparation returns an
  opaque state capturing the prepared pool and original destination orphan IDs.
  Finalization after the caller's history/bin restoration collects only unowned
  prepared IDs, preserving original orphan headers and later-added binaries.
  Active leases or missing final owner payloads fail before collection.
  History::from_entries
  constructs an ordered snapshot list once and strips nested histories, avoiding
  repeated front insertion in the first-party history union.
  Database::reorder_groups validates exact sibling permutations without altering
  group metadata or previous-parent provenance. Its direct test covers valid
  order and atomic rejection of incomplete/duplicate/foreign IDs, with full
  database equality before/after a round trip.
  Database::export_plain_xml exposes an explicitly plaintext, credential-free
  in-memory export; caller consent, destination writes and zeroizing ownership
  stay with the native application.
  Database::from_unencrypted_inner_xml parses only caller-validated/normalized
  PlainXml with the prepared dense binary pool. Database::import_group_tree
  copies complete current/history trees, binaries and icons with fresh object
  IDs and remapped ownership/internal UUID field/auto-type/OverrideURL references.
  It rejects missing binary payloads and live history leases before mutation;
  source settings/deletion intent are not adopted. First-party candidates own
  target/lifetime/schema/resource validation, protected source archival and
  publication rollback. Owned reference-rewrite buffers use Zeroizing.
- `src/format/xml_db/mod.rs`: reconstruct attachment ownership on load and map
  every current/history `Binary Ref` to the exact dense inner-header position on
  save. Missing references fail before destination publication. Serialize raw CR
  as XML character references so text values/keys retain their line endings.
  The model emits text rather than CDATA; literal entity strings and protected
  base64 remain intact, and the reader still applies standard raw-CR normalization.
  Pure-whitespace entry text is handled separately by the entry reader below.
  Validate actual serialized XML against XML 1.0 Char ranges before publication;
  forbidden ordinary text/keys/group/meta/history returns a content-free error.
  Protected values are already Base64 and may retain arbitrary source scalars.
  No silent filtering, reader change or all-Unicode-noncharacter rejection.
  Complete PlainXml export reuses this model and dense map with PlainCipher,
  then converts only controlled protected-string/date events and embeds the
  original binary pool in Meta. Generator identifies SecPivot; protected empty
  values stay marked. Dates are ISO UTC within the official1–9999 range;
  every modeled meta/group/entry/history/custom-data/icon/deletion date is covered.
  Output validates plaintext XML scalars and preserves CR references. No input
  mutation, file I/O or key is involved. Four direct tests cover empty/literal
  fields, illegal historical secrets, missing current/history binaries and dates.
  Independent official-library import/resave/native reopen proves selected
  synthetic object fidelity; binary protection, standard-field client resets,
  unprotected newline normalization and foreign metadata semantics remain limits.
  parse_plain_inner_xml scopes the PlainXml CustomData text conversion to this
  prepared import; ordinary KDBX typed decoding is unchanged. Native tests cover
  complete tree copying, sparse/inline/current/history binaries, fresh references,
  protected empty values, icons and failed publication. Independent official
  library read/resave and native reopen use a separate synthetic import fixture.
- `src/format/xml_db/entry.rs`: deserialize entry values through dedicated
  `$text` and a direct String converter, preserving ASCII/Unicode whitespace.
  This works with the scoped QuickXML patch described in `QUICK_XML_PATCH.md`;
  direct conversion alone cannot bypass upstream blank-event skipping. Empty
  values, missing-Value rejection, protected flags/Base64 and generic optional
  metadata rules retain their contracts. Current/history and external-file
  native regressions accompany two direct tests.

- `src/format/xml_db/timestamp.rs`: require exactly eight decoded Base64 bytes,
  validate the signed-second duration and checked addition to the year-one epoch.
  Short/long values and out-of-range dates return fixed errors instead of panic
  or silently truncating trailing bytes. Existing ISO parsing, signed seconds,
  serialization and optional/missing-date rules are preserved. Four specialized
  tests cover small invalid inputs, valid chrono bounds/negative seconds/ISO,
  the complete inner XML reader and a normal AES KDBX save/reopen control.

- `src/db/types/meta.rs` and `src/format/xml_db/meta.rs`: retain optional
  MasterKeyChangeForceOnce in the model and both XML conversions. The existing
  boolean converter preserves explicit True/False; absent remains absent.
  A literal-XML regression exercises all three states through both conversions;
  native temporary KDBX and offline remote tests cover ordinary save, Save As,
  KDF/storage updates and read-only refusal. Expiry enforcement and consumption
  of this flag remain first-party work, not a claim of this preservation patch.
  XML Meta/CustomData also expose an internal PlainXml-only text conversion;
  Base64-looking custom values remain text at this import boundary. Existing
  KDBX read/type interpretation and source archive preserve their separate limits.

The other 51 upstream files are byte-for-byte unchanged. The provenance JSON
records each original hash and the nine reviewed patched hashes. Run
`npm run verify:keepass-patch` after any patch edit; it verifies source hashes,
Cargo wiring and formatting of the patched Rust files. Update the nine patched
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
The thirty-six dependency-specific tests can be run separately:

```powershell
npm run test:keepass-patch
```

The helper tests a disposable source copy with the QuickXML path override so
Cargo can adjust its standalone lock without rewriting the published package.
The ordinary full verification runs this check and the QuickXML library tests.

An upgrade alone is not a verified fix: the inspected 0.13.25 and 0.17.0 packages
retain the relevant old attachment-removal and tracking logic. Remove this local
patch only after an upstream version passes the same integrity regressions.
History restoration of the whole attachment set is a separate application
contract and must not be inferred from this ownership fix.

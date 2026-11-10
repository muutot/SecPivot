# QuickXML dependency patch

SecPivot pins `quick-xml = 0.41.0` and uses the complete published MIT package
under `quick-xml-0.41.0/`. Its crates.io archive is
<https://static.crates.io/crates/quick-xml/quick-xml-0.41.0.crate>, SHA-256
`e660451e55124f798a69a5af3f49ccfbefbd41910eefd25caf2393e1f3473ec1`, matching
the previous registry lock. The separate registry 0.42 dependency is unchanged.

Only `src/de/map.rs` is changed. `ElementMapAccess` preserves blank text events
when the destination struct explicitly declares `$text`. Structural maps and
`$value` mappings keep their existing indentation skipping. KeePass's
`StringValue` opts into `$text` with direct String conversion, preserving both
ASCII and Unicode whitespace without globally changing optional metadata.
Empty-value defaults, protected attributes/Base64, raw-CR normalization and XML
entity/CDATA/comment handling remain under the existing reader contract.
Two new unit tests cover the explicit-text boundary and structural controls.

The other 42 files and MIT license are byte-identical. Published sources use
CRLF, including the reviewed file. `.gitattributes` exempts this package and
`quick-xml-local.patch` from text/EOL normalization so checkout preserves exact
bytes and the diff retains its CRLF payload. Do not blanket-format the package.
The owned source format gate compares normalized EOL while its hash gate checks
the original EOL. Check all staged package hashes before committing changes.

`quick-xml-patch-provenance.json` records all 43 upstream/current hashes.
`quick-xml-local.patch` is a byte-preserving zero-context diff. In a fresh
original package, `git -c core.autocrlf=false apply --unidiff-zero` reconstructs
the verified bytes. Updating a hash is appropriate only after reviewing that
specific file against the original archive and rerunning reconstruction.

Run `npm run verify:quickxml-patch` for version/path/hash/format checks and
`npm run test:quickxml-patch` for serialization-enabled library unit tests.
The test helper uses a disposable source copy, keeping the published Cargo.lock
unchanged; it validates the temporary root before cleanup. Both commands and
the KeePass/native regressions are included in `npm run verify`.
The packaged upstream suite has four ignored low-level bang-element cases;
they do not count as executed checks. Native cross-client/Android/provider,
all features and complete XML/KDBX fidelity remain separate verification areas.

# Project Structure and Runtime Surfaces

`vault/master_key_expiry.rs` owns nonsecret read-only key-age offers, with
`master_key_expiry/rules.rs` implementing KeePass database/global comparisons
and XML duration/cutoff parsing. Its tests execute temporary KDBX/MemoryStorage
and the independent .NET Framework vectors in
`tests/fixtures/master-key-expiry-xml.json`; the captured local date and boundary
offset keep partial-date/timezone reference values deterministic across hosts.
The `get_master_key_expiry_offer` command reads native ConfigStore rules and
resolves its explicit session. Vault service reads guard their renderer epochs.
Config strings default empty on both sides; tests cover old configs, restart
and invalid values. validate_master_key_expiry_rule shares the actual parser for
explicit Apply in settings/MasterKeyExpiryRuleCard. MasterKeyChangeForm owns the
shared settings/renewal credential form and current-view success/busy callbacks.
MasterKeyExpiryDialog owns compact required/recommended/error review and Back.
services/master-key-open-prompt.ts owns nonsecret inspection/acknowledgement;
KdfOpenPrompt serializes master renewal before KDF and is excluded from TCATO.
Optional expiryToken binds native change preparation to the history lifetime.
Real WebView/provider/official-client behavior remains unverified.

The ninth reviewed KeePass patch preserves Meta.master_key_change_force_once
in the core model and XML conversion. Native master_key_force_once_tests.rs
covers absent/false/true across local/remote save/reopen, Save As, KDF/storage
changes and read-only refusal. A dependency regression starts from literal XML
and executes both model conversions. First-party persist_change/complete_change
consume true only after an authoritative new-key commit. Additional native tests
cover three-state local reopen, failed writes/read-only refusal, concurrent edits
and offline remote keyfile-only change. Opening renewal uses the same change path;
the dependency patch itself is metadata fidelity only.

vault/master_key_time_tests.rs exercises real temporary KDBX dates for local/
remote creation, password/keyfile-only changes, concurrent completion, failed
writes and ordinary save/Save As/KDF preservation. persist_change owns its
snapshot and returns the committed timestamp with PersistOutcome; both native
command and synchronous session completion pass this date without extra cloning.
The credential fixture follows this internal signature, using pinned chrono for
its synthetic metadata boundaries; its ownership observations remain unchanged.

`utils/master-key-attempts.ts` owns per-credential-prompt retry admission and
positive-int32 normalization. VaultWelcome and LockScreen own prompt reset,
exhaustion/credential clearing and explicit restart UI. SecuritySettingsPanel
exposes masterKeyTries through the shared compact numeric TextField, whose
optional min/max/step attributes now pass through to its input. Config settings
and normalization persist only the preference; no vault/IPC type changes.
`tests/master-key-attempts.test.mjs` executes actual host handlers, cleanup and
settings persistence with synthetic vault/credential APIs; ConfigStore tests
exercise legacy defaults and real temporary config reloads.

`scripts/test-android-tls.mjs` executes `tests/fixtures/android-tls.rs` over the
actual net.rs Android branches in a disposable host Cargo package. Only cfg is
substituted; all network/crypto APIs are real and versions follow Cargo.lock.
Public DER provenance/licenses and a synthetic localhost TLS pair live under
tests/fixtures; the script validates public sample hashes before execution.
The four native fixture tests run through test:android-tls in the full verify
gate. Android dependencies impose the reviewed root/verifier security floors.

`vault/kdf_upgrade.rs` owns nonsecret lifetime-bound offers and an in-memory
upgrade transaction serialized by the existing persistence permit; its nested
tests cover real temporary KDBX/MemoryStorage and parked-session/older-save
ordering. `helpers::default_kdf_config` supplies the same costs to upgrades and
apply_kdf/create. `commands/vault.rs` exposes get_kdf_upgrade_offer and
upgrade_weak_kdf, registered in lib.rs. `services/vault.ts` keeps offer/mutation
replies in the original epoch and exposes a read-only getSessionEpoch lookup.
`services/kdf-open-prompt.ts` owns once-per-lifetime acknowledgement, UI ownership
and invalidation; `components/KdfOpenPrompt.svelte` wires it only to the main
route, outside TCATO. `KdfUpgradeDialog.svelte` owns compact current/recommended
costs and Later/Use recommended actions. Tests execute actual host/controller/
service bodies without changing a user vault or live clipboard.

`components/KdfWarningDialog.svelte` owns the shared localized weak-cost
confirmation over ModalShell/Buttons. DatabaseSettingsDialog uses it before a
changed weak KDF patch; MasterKeyChangeForm uses it after native-cost preflight
for master-key changes. `utils/kdf-parameters.ts` owns exact official thresholds,
and ConfigStore/TS settings persist the default-on keyTransformWeakWarning.
Master-key confirmations are bound to SessionViewGuard and dismissed on tab/view
changes. Existing IPC commands are unchanged; getDatabaseSettings adds an epoch
check to reject disposed-session replies.

`vault/close.rs` owns the shared save/discard/cancel transaction and close-attempt
permit; its tests run disposable local and offline remote KDBX persistence plus
ownership races. `commands/vault_close.rs` owns localized native prompts, their
focus lease event and workspace cleanup adapter. `exit.rs` owns desktop exit
admission/retry, retaining the main window during save or cancellation. lib.rs
registers ExitGate and routes tray quit/main-window close through ExitRequested.
Async close_vault and close_all_vaults use the same engine; successful closure
precedes capture/TCATO/temp cleanup. `services/vault-close.ts` classifies native
markers and owns error feedback; vault service failure refresh retains epochs.
Security automatic locks, VaultTabs and page manual lock handle cancelled/failed
close explicitly. Config and SecuritySettingsPanel expose autoSaveOnClose.

`utils/entry-row-layout.ts` builds prefix offsets for full-height entries and
half-height directory separators, plus binary-search row/range lookup.
EntryTable uses that index for virtualization, spacer totals, scroll clamps,
focus and pixel-based page navigation; recomputation follows rows/density,
not every scroll. Shared fixed-height virtual-list helpers remain available
to other lists.

`utils/group-drag.ts` owns validate-first group relocation plans and local
session/root drag ownership. GroupTree owns the controller/target indicator;
recursive GroupNode renders drag sources and before/inside/after targets.
The page binds drop to vault.moveGroup and guarded selection/reveal;
commands/groups.rs requires the explicit origin and vault/entries.rs owns
candidate mutation/rollback. Native sibling permutation is in the existing
patched keepass db/types/mod.rs, without changing sibling provenance.

`vault/kdf.rs` owns exact tagged KDF parameter DTOs, cryptographic validation and
credential-free timed calibration. The existing vendored KeePass `KdfConfig::benchmark`
exposes the same transform used by KDBX, avoiding duplicate crypto implementation.
`commands/vault.rs::benchmark_database_kdf` owns worker scheduling, the process-wide
benchmark gate and originating-session checks. `vault/kdf_storage_tests.rs` exercises
real timing, JSON parameter patches, KDBX save/reopen and concurrent completion.
`utils/kdf-parameters.ts` owns BigInt cost validation and exact MiB/byte conversion;
DatabaseSettingsDialog composes its draft/apply/save UI, and `services/vault.ts`
binds benchmark replies to the originating session epoch. `tests/kdf-*.test.mjs`
cover conversions, extracted production handlers and actual service IPC/lifetimes.

`EntryTable.focusRowAt` subtracts the measured sticky header from the available
row viewport; filtering's scroll clamp includes the same header height. Narrow
summary mode contributes zero, while desktop and narrow full-column mode retain
the header. `tests/entry-table-focus.test.mjs` executes the complete focus/clamp
bodies against independent DOM-flow bounds. Native browser End/Home checks cover
all three modes and toggling full columns; no CSS or virtualization change is made.

`EntryTable.handleRowKeydown` keeps the existing Ctrl/Cmd+A entry shortcut but
otherwise handles only keys targeted at the row itself. Inline favorite/copy
buttons retain native activation without an additional row selection or arrow
navigation. Their click handlers retain propagation isolation. The production
row/list handler regressions in `tests/entry-table-keyboard.test.mjs` exercise
descendant Enter/Space/arrows and row activation/navigation/select-all with a
synthetic event seam; they do not emulate native browser button click dispatch.
No markup, CSS, theme tokens or clipboard behavior is changed.

`usePanelLayout` derives selected UUID separately from its auto-open effect.
Equal-UUID snapshot replacement therefore preserves manually closed detail and
does not consume pending right-click suppression. Explicit selectionVersion
changes still re-open the same entry; showDetailOnSelect and selection clearing
retain their existing behavior. `tests/panel-layout.test.mjs` compiles the full
layout and selection modules and exercises actual Svelte effects/settings
subscriptions with synthetic entries; it does not inspect native window layout.

`columns.svelte.ts::applyColumnReorder` receives an insertion index in the
visible column order and uses that same order for self/next-neighbor no-ops.
The full persisted array supplies removal and anchor placement; hidden columns
retain relative order and custom columns retain widths. `tests/entry-columns.test.mjs`
compiles the complete columns/layout rune modules and runs actual Svelte effects,
settings subscriptions and saveLayout with synthetic config IPC. It covers
forward/backward/end/self drops and normalized persistence/reload, not native
pointer gestures or platform runtime behavior. No markup or theme rule changes.

`vault/group_search_tests.rs` covers the group-search raw/effective contract,
default/root inheritance, explicit overrides, save/reopen, notes preservation,
native matching/candidates/REF and recycle-bin exclusion. Nine tests pass.
`tests/fixtures/group-search-snapshots.json` is checked against the actual native
serialized search fields after temporary KDBX reopen (random IDs normalized).
`tests/group-search.test.mjs` compiles the complete production filter through the
Svelte client compiler/runtime and uses this shared projection plus the real
advanced matcher/tree collector. `group-search-demo.test.mjs` executes the actual
demo service graph with synthetic storage; `group-search-dialog.test.mjs` extracts
complete production initialization/submit with a plain state seam. Together they
add twelve controls, without actual native IPC or rendered component evidence.
`src/lib/utils/group-search.ts` resolves only browser demo search metadata before/
after edits and on loading; native snapshots are resolved in vault/serialize.rs
using vault/helpers.rs. GroupMetaDialog preserves raw inheritance when the toggle
is unchanged. No markup, CSS, theme-token or component layout change is involved.

`tests/totp-copy.test.mjs` compiles the complete production `useTotpCode` rune
module and runs the actual Svelte client effects. Eight controls cover entry and
session changes (including revisits/shared cache), disposal, copy refusal and the
1200ms feedback timer. Vault/OTP, clipboard and timer/document APIs are synthetic.
Copy feedback belongs to one subscription generation; cleanup invalidates pending
success callbacks before another entry/session or a disposed instance can flash.
The configured full verification passes with474 frontend tests and855 root Rust tests.

`vault/save_status_snapshot_tests.rs` covers the new statusRevision publication,
actual temporary KDBX save/change/reopen, deliberate concurrent edits, failures,
history selection and delta/status serialization. Eight native tests pass under
the explicit root dev codegen-units256 configuration and4/2GiB resource guard;
the configured full gate passes after the test-only Clippy correction and
earlier runtime resource interruption. `tests/vault-save-status.test.mjs` executes the
real service graph with synthetic invoke/storage; eight controls include delayed
reads, status-bearing deltas and stale icon-cache refusal. statusRevision belongs
to persistence status; the existing edit revision remains SaveJob/history identity.

`vault/save_as_snapshot_tests.rs` exercises the actual native session and
temporary KDBX persistence with prepare/edit/cache/persist/complete ordering.
Save-as target adoption now advances revision and clears the full snapshot cache
while retaining concurrent edits. Counter/warning injection and MemoryStorage
are synthetic boundaries; actual Tauri/device/provider behavior is separate.

`tests/vault-save-as-reopen.test.mjs` runs the actual vault/settings service graph
with synthetic invoke/storage and cloned replies. The save-as quick-reopen side
effect accepts only the gate-adopted reply from the active session; six controls
cover stale revision/epoch, selection labels, parked/closed sessions and rejection.
No native save-as, IPC or Android provider is exercised by these frontend checks.

`platform/clipboard.rs` shares a same-thread message-only writer owner and guarded
GlobalAlloc buffer across text/binary publication. Failed writes wipe/free owned
bytes; successful publication transfers memory to OS. No IPC signature changes.
`scripts/test-clipboard-write.mjs` extracts the complete module with only imports/
cfg redirected to `tests/fixtures/clipboard-native-write.rs`'s synthetic API.
Fourteen independent paths run in an offline disposable Cargo fixture; the native
owner-window check in the platform module uses no clipboard API. Actual clipboard
interop and external identical-copy/cross-renderer scheduling remain separate.

`persist.rs` owns guarded credential fields on SaveJob/RemoteRefreshJob/RemoteMergeJob.
Five session factories wrap existing clones immediately; consumers rely on field
destruction while preserving database/result moves and error classification.
`tests/fixtures/persistence-job-credentials.rs` extracts three job declarations,
five factories and four consumers. Only credential payload/parameter types are
observed; database/KDF/parser/storage/settings boundaries remain stubs. Five added
groups cover80 paths/120 owned values, including real Tokio queued versus running
abort behavior. The shared credential suite now executes25 groups.

`commands/credentials.rs` owns the String-shaped `VaultPassword` argument guard
for local/remote open/create and master-key replacement. It deserializes directly
into Zeroizing ownership
before future polling, has no Debug/Serialize/extraction surface and borrows
unchanged values for native preparation/adoption. It is distinct from desktop
`commands/credential.rs`, which handles the OS saved-credential store.
`scripts/test-vault-credentials.mjs` compiles ten checks from exact handler
blocks and the actual guard module; only the password payload type is observed.
`tests/fixtures/vault-command-credentials.rs` covers42 owned paths/nine JSON
controls, using stub Tauri/config/persistence/adoption APIs and real Tokio worker
abort. Its disposable manifest seeds the repository lock, is offline and shares
the target cache; full verify runs it before root Rust tests. No native IPC/
KDBX/provider or full process-memory claim follows from these fixtures.

`tests/fixtures/master-change-credentials.rs` adds14 command paths/26 observed
new password/keyfile values. Completion is a stub matching the current session
API; adopted credentials remain valid until simulated close. The production
command and synchronous session method guard successfully read new keyfiles;
`complete_change` borrows and copies only inside session adoption. Previous
keyfile replacement is covered below; helper/framework/renderer buffers remain separate.

`session.rs::wipe_replaced_keyfile` owns stored-keyfile cleanup at local replace,
remote adoption and master completion, including removal to None.
`tests/fixtures/keyfile-replacement.rs` extracts five actual session methods
and wipe helpers with Vec bound to a live-owned buffer observer. Four checks
cover25 paths/25 old-and-new buffers while state/storage/snapshot/UUID/outcome
boundaries are stubs. These four groups run in the shared credential suite;
native temporary KDBX controls separately exercise three replacement boundaries.

`persist.rs::PreparedKeyfile` guards each successfully read open/create keyfile
through all four preparation functions and returned tuples. Direct session methods
transfer immediately at adoption; commands take only inside the registry callback.
`sessions.rs::open` owns an Unregistered guard until registration; failure or unwind
closes the fresh session, while success moves it out. No global VaultSession Drop,
IPC shape or session storage type changes. `tests/fixtures/keyfile-preparation.rs`
extracts four preparation bodies with observed return payloads; boundary read/KDF/
parse/storage/mirror functions are stubs. The command fixture adds20 keyfile paths,
including post-adoption registry refusal, active-slot poison and discarded worker
output. It extracts actual registry open and does not supply implicit session Drop.
These six added checks cover62 paths/52 buffers within the shared credential suite.

Native KDBX `format/xml_db/timestamp.rs` now validates exactly eight Base64
bytes and checked signed-second/year-one date arithmetic before publishing
a Timestamp. Four dependency regressions cover invalid lengths/date bounds,
valid signed/ISO controls, inner XML refusal and normal AES KDBX reopen.
The timestamp file is among nine reviewed patches; 51 upstream files remain
unchanged. Other native metadata/container/client semantics are separate.

`exchange_icon_metadata_tests.rs` is a test-only child of `vault/exchange.rs`.
Four red regressions establish dropped names and replaced/created icon times;
five final tests exercise complete metadata, shared source definitions, native
source/target reopen and combined UTF-8-name/image copy budgets. `XmlIconName`
preserves optional text; `PastedCustomIcon` resolves data/name/time for insertion.
Existing icon-identity/expansion tests read its data member and retain their
original identity, byte ownership and budget assertions. No IPC/UI is added.

`exchange_custom_time_tests.rs` is a test-only child of `vault/exchange.rs`.
It exercises native CustomData value/time preservation through in-memory framing
and source/target KDBX reopen, fresh pasted UUIDs, missing legacy timestamps and
out-of-range rejection. The internal XML/resolved models carry optional item
timestamps; native commands, renderer DTOs and component ownership are unchanged.

`templates/form/RadioGroup.svelte` owns compact string-valued radio choices,
selection state, roving Tab and arrow/Home/End focus. DatabaseSettingsDialog
composes it for KDF/cipher/compression while retaining its save/view ownership.
`tests/radio-group.test.mjs` runs the real select/navigation handlers with
deterministic focus peers; browser fixtures separately verify native keys,
accessibility state, layouts and synthetic save patches. No IPC/backend is added.

`entry_autotype_retention_tests.rs` is a test-only child of `vault/mod.rs`.
Four regressions exercise both native entry-save APIs from reopened None and
UseClipboard sources, same/changed visible settings, current/history/time,
omission/null, creation/no-config defaults and failed save rollback.
`entries.rs::build_entry_autotype` takes an optional prior native configuration;
standalone updates and `entry_save.rs` supply the same edited entry.
No public IPC, dependency or UI surface is added.

`exchange_autotype_tests.rs` is a test-only child of `exchange.rs` for the stored
AutoType obfuscation option. Four regressions cover four native configurations
in memory and reopened-source/target KDBX comparisons, six absent/zero controls,
five known-one integer spellings and six invalid codes. `XmlAutoType` carries
the optional wire code; `parse_auto_type` resolves native `PastedEntry.auto_type`
before insertion. The existing in-file sample now actually initializes AutoType
and compares it, matching its stated scope. No IPC, vendor, dependency or UI
change; runtime efficacy remains separate audit work.

`exchange_icon_identity_tests.rs` is a test-only child of `exchange.rs` for
custom icon definition identity. Three regressions cover ten duplicate refusals,
unique/empty/unused and shared-reference controls, native shared-definition
generation and temporary KDBX reopen. The parser checks exact decoded XML UUID
text globally before data filtering; entry reference sharing stays independent.
No IPC, vendor, dependency or UI change; UUID canonicalization and invalid/missing
unique-icon/reference semantics remain separate audit work.

`exchange_custom_value_tests.rs` is a test-only child of `exchange.rs` for
complete custom metadata value transfer. Four regressions exercise native
String/Binary/None memory transfer, reopened-source/target KDBX comparisons,
legacy plain text and strict internal type/shared-byte-budget controls.
`XmlCustomDataItem.value_type` owns the optional SecPivotType wire extension;
`PastedCustomDataItem` holds resolved native values. `parse_custom_data` and
`decode_custom_binary` validate before insertion. No Tauri/TS IPC, dependency
or vendor change; official formats, native metadata typing and subsecond/foreign
timestamp formats remain separate audit areas.

`exchange_custom_identity_tests.rs` is a test-only child of `exchange.rs` for
per-entry custom metadata identity. Three regressions cover nine duplicate-key
refusals, distinct names/two-entry scope and an independent ordinary-field
namespace through native KDBX reopen. Production `parse_entry` uses a separate
borrowed-key set before publishing any list; no IPC/dependency/vendor changes.
Native KDBX value typing and custom-icon UUID canonicalization remain audit areas.

`exchange_string_identity_tests.rs` is a test-only child of `exchange.rs` for
ordinary per-entry field identity. Three regressions cover 18 duplicate-key
refusals, distinct decoded names/two-entry scope and legal repeated binary
references through native KDBX reopen. `parse_entry` holds borrowed exact keys
in a per-entry set after handling binary references, refusing a partial parsed
list without normalizing names. No IPC, dependency or vendor patch changes;
other metadata identity and foreign-container semantics remain audit areas.

`exchange_gzip_member_tests.rs` is a test-only child of `exchange.rs`. Its five
regressions cover five legal member partitions, seven suffix refusals, cumulative
small-budget overflow, later trailers at the exact total limit and synthetic
native KDBX reopen fidelity. The shared `gunzip_limited` uses `MultiGzDecoder`
from the existing pinned flate2 dependency, reaching underlying EOF for successful
bounded output before XML parsing. No IPC, dependency or vendor patch changes;
full header/container semantics and allocation/CPU costs remain audit areas.

`exchange_document_tests.rs` is a test-only child of `exchange.rs`. Four tests
cover complete envelope refusal, named-root controls, legal surrounding misc
and actual synthetic KDBX saved/reopened values. `validate_entries_document`
streams to EOF before `parse_entries_document_limited` deserializes or resolves
binaries; it publishes no partial list and retains pre-root header handling.
No IPC, dependency or vendor patch changes. Full XML schema/encoding and
gzip/container interoperability remain distinct audit areas.

`exchange_encode_tests.rs` covers `encode_payload_limited`, called by the public
encoder with 64 MiB for both document and full frame. Four Windows-host tests
exercise compressible input, gzip/flags overhead, legal byte round trips and
native DPAPI size overhead; the DPAPI test is Windows-only. No IPC is added.

`exchange_expansion_tests.rs` and `tests/fixtures/exchange-expanded-binaries.json`
cover the shared attachment/custom-icon copy budget in `exchange.rs`. The public
parser delegates to a limited helper with 64 MiB; each copy debits one cumulative
remaining budget before allocation. Five synthetic tests cover amplification,
overflow refusal, empty/exact/shared controls and native KDBX reopen fidelity.

`exchange_inflate_tests.rs` is a test-only child of `exchange.rs` for the gzip
output bound. Four regressions use `gunzip_limited` with small limits to cover
overflow refusal, exact-limit CRC/ISIZE/truncation, legal boundaries and a
parseable truncated XML prefix. `gunzip` keeps the64 MiB production constant;
the helper reads limit+1 with saturating arithmetic and rejects before returning
output. Attachment/icon copies have the separate cumulative budget above;
XML-generation costs and full gzip/XML-schema/container semantics remain audit areas.

`exchange_binary_tests.rs` also owns duplicate binary-ID regressions, using
`tests/fixtures/exchange-binary-identity.json`. Six duplicate-definition cases
reject during `parse_entries_document`; three unique sparse/reordered/shared
controls exercise native persistence and re-export. Definitions require unique
numeric IDs across the whole document, while references may share one ID.
Existing empty-binary tests and actual clipboard/foreign-format limits remain.

`exchange_field_identity_tests.rs` is a test-only child of `exchange.rs` for
ordinary field collisions with the legacy attachment-reference namespace.
Two regressions cover 16 outgoing-refusal combinations and legitimate near-prefix
fields with empty/nonempty attachments through native save/reopen. `serialize_entry`
rejects exact `BinDes-` prefixes before document publication; full field identity
support and foreign schema/container compatibility remain separate work.

`exchange_binary_tests.rs` is the test-only child module of `exchange.rs` for
empty binary payloads. Four regressions cover actual document/framing/parse,
empty XML forms, mixed-byte/native persistence/re-export and malformed binary
envelopes. `XmlBinary` requires an ID, defaults dedicated text to empty Base64
and rejects unknown structure. Real clipboard and foreign compressed/container
formats remain separate compatibility work.

`exchange.rs` owns exact entry-transfer text (`XmlStringValue.$text`, referenced
CR), with `exchange_text_tests.rs` as its test-only child module. The five native
regressions cover field/protection values, reference controls, saved/reopened
insertions and attachment names without system clipboard access. Its current
payload uses a legacy plaintext `Protected` marker; the omitted KeePass container
is one of the remaining interoperability differences, not a completeness claim.

`kdbx-xml.ts` owns the pre-IPC XML field identity gate (empty, edge-whitespace or
duplicate names reject). `tests/kdbx-xml.test.mjs` covers the production guard
with DOM-shaped trees; actual DOMParser/page/IO evidence is recorded separately.
`tests/fixtures/xml-migration-wire.json` holds synthetic browser-captured bulk
inputs and independent native-export expectations, replayed by IO tests and
`vault/website_rules_storage_tests.rs` through native persistence/re-export.

`website_rules.rs` owns HistoryRuleSource and get_history_website_rules, sharing
the validated parser with the current reader and reusing entries.rs selection
validation. commands/website_rules.rs exposes a strict required-session read
envelope, registered for both desktop/mobile in lib.rs. services/vault.ts validates
selection/source and response lifetime. utils/history-rule-view.ts cancels stale
reads and owns ephemeral rule content; HistoryVersionDialog composes the compact
read-only rows with Button/Feedback. EntryDetail passes the originating session.
Native history_rule_tests.rs and command tests cover exact sides, absence/empty/
invalid states, shifted/reopened versions, parked/closed origins and KDBX reopen.
Frontend utility/service tests and isolated DOM fixtures cover cancellation,
focus, language/theme/narrow/long-URL rendering; real WebView/provider is separate.

`entries.rs` owns history selection tokens and validates them before restore or
delete. `session.rs` regenerates `VaultSession.history_lifetime` for each local/
remote adoption; revision changes reject old selections. `dto.rs::HistoryVersion`
serializes the metadata-only token. `commands/vault.rs` requires String session
IDs and mutation tokens; `services/vault.ts` accepts `HistorySelection` and guards
completion epochs. `EntryDetail` passes the original selected version and clears
an obsolete preview/cache on refusal. `vault/history_selection_tests.rs` covers
native insertion/deletion/reopen/remote-adoption/registry boundaries; its legacy
fixture convenience methods exist only under cfg(test). Frontend service and
extracted production-handler tests cover payload/lifetime/notification behavior.
Historical protected-content reading uses the separate selection-bound command described below.

Shared `vault/website_rules.rs` imports `parse_entry_id` from `vault/helpers.rs`
directly. The parent vault aliases for browser matching remain desktop-only;
shared modules must not use those aliases for cross-platform helpers. A host
compiler fixture covers this complete module with desktop enabled/absent and
real pinned host dependencies; it is supplemental cfg-name evidence, not an
Android crate/ABI/NDK/JNI/APK or device runtime gate. See `docs/android.md` for
the current toolchain limit and separate real-platform requirements.

`vault/entry_save.rs` owns the explicit create/edit request, nullable optional
editor extras, target/attachment/flag validation, candidate database mutation,
one complete tracked edit and final publication rollback. `commands/entry_save.rs`
exposes required-session `save_entry` on desktop/mobile; native tests cover
origin isolation and synthetic KDBX data/history/rollback. `services/vault.ts`
publishes the returned UUID/state under the captured Symbol lifetime and applies
one candidate browser draft. `useEntryEditor` uses that single transaction for
create/edit and the existing batch mutation for edit-multi; page selection remains
injected, without newest-created timestamp lookup. `entries.rs` owns the shared
Auto-Type normalization used by standalone and editor writes.

`EntryEditorDialog.svelte` owns `utils/website-rule-draft.ts` for editor-local
policy reads and draft changes across tabs; `WebsiteRulesPanel.svelte` composes
shared Toggle/TextField/Select/Button/Feedback controls. The dialog sends the
optional set/clear website patch as the fifth `onsaved` argument through page
and `useEntryEditor` to the same native editor transaction. `templates.ts` resolves
template policy with its hidden values atomically. `tests/website-rule-draft.test.mjs`
exercises real draft operations/lifetimes; editor/service tests cover forwarding
and browser refusal. Native editor tests cover policy history/KDBX and rollback.

`entries.rs::history_diff` and `dto.rs::HistoryDiff.website_rules` provide one
managed-policy change flag to entry history and `security.rs::change_timeline`.
`HistoryVersionDialog` includes it in counts/Metadata presence summaries;
`ChangeTimelineDialog` supplies its localized chip. The history dialog separately offers explicit rule reading.
`website_rules_storage_tests.rs` covers native policy-only changes, raw/protection/
presence controls and KDBX reopen while keeping DTO content masked.

`services/vault.ts` separates publication-epoch admission from read-only ownership
checks on completion, uses fresh Symbol lifetime identities to prevent reused-ID
re-admission collisions, and versions tab-list reads across successful native
registry transitions. Backend lock acknowledgement clears renderer state before
temp cleanup. `tests/vault-session-publication.test.mjs` runs the real service
graph with synthetic invoke/storage seams for late mutation/refresh/tab-list
replies, lock cleanup, failed closes, explicit/uncached IDs and retained-session
ordering. The following standalone policy IPC remains available to non-editor
callers; the compact editor uses the atomic save transaction.

`commands/website_rules.rs` owns registered explicit-session policy get/set IPC
and a required nullable write envelope. `website_rules_tests.rs` covers native
orchestration/serialization/session isolation and current/history presence hints.
`tests/website-rules-service.test.mjs` executes actual vault service methods with
synthetic storage/invoke seams for envelopes, lifecycle guards and browser refusal.
`types/vault.ts` mirrors version-1 modes/rules and `hasWebsiteRules`; rules remain
outside ordinary/history snapshots. No platform/dependency changes are required.

Main-page feedback preserves `TipKind` across `flash` and the IO/group/favicon/
entry-editor notify seams. Known failures pass error explicitly; the global
`TipsHost` keeps its existing semantic danger styles and success default.

This reference is a source map, not a substitute for reading the current files. Update this map when ownership or entry points change.

`vault/entries.rs` uses tracked Entry/Group removal for all user-facing permanent
deletes and recycle-bin emptying, persisting DeletedObjects times recursively.
`vault/tests.rs` covers actual delete APIs, KDBX reopen, stale merge in both
directions, rejected batches and newer-edit policy. Temporary merge detachment
still uses untracked removal; it must not manufacture permanent-delete intent.

Frontend `utils/csv.ts::buildCsv` and native `vault/serialize.rs::escape_csv`
guard formula-like values before RFC quoting. `tests/csv.test.mjs` and the
native `export_csv_keeps_guarded_cells_inside_rfc_quotes` exercise their wire
format with separators, quotes, multiline fields and literal apostrophes.
`parseCsv` throws content-free `CsvParseError` on malformed quoting;
`io.csvToImportEntries` localizes it and the main CSV handler stops before writes.
`io-import.test.mjs` exercises that actual extracted handler, valid multiline
bulk fields and obsolete picker completions, alongside the parser wire tests.
The same tests extract the XML handler to check its error/view boundary and
XML-only filter; actual DOMParser/menu/filechooser behavior is verified with
fresh-browser synthetic XML, not inferred from the handler fixture.

`GroupTree` initializes known UUIDs from every initial group independently of
persisted expansion flags; existing collapsed groups must never auto-expand as
if they were newly inserted groups.

Both root and recursive tree rendering use `orderedChildren` to place the
recycle bin last without changing the database's stored child order.

`Select` consumes its own Escape, restores trigger focus and uses an instance
listbox ID. `ModalShell` handles keyboard events only for the top modal and when
the event has not already been consumed by a child control.
Its Tab boundaries exclude negative tabindex, effective disabled controls,
inert ancestry and hidden layout/visibility on every key. Production handler
regressions live in `tests/modal-focus.test.mjs`; real attachment-editor Tab
traversal is verified separately in a fresh browser with synthetic data.

Import favorite metadata flows from Bitwarden/explicit CSV Favorite columns
through IO/DTO into native and browser bulk creation. `bitwarden_tests.rs` covers
command/wire/KDBX and batch rejection, while CSV/IO tests cover conversion and
IPC/default/editor continuity. It uses the existing favorite field, not a new
setting or ordinary-edit property.

`vault/helpers.rs::regex_matches` centrally ignores empty KPRPC expressions
for Bridge/RPC and both auto-type scans. Host/KDBX regression tests distinguish
empty placeholders from meaningful whitespace and explicit wildcards; the
existing pattern/text/compiled-size bounds remain unchanged.

`vault/entries.rs::update_entry_flags` validates color before writing any flags;
its regressions compare raw entry, cached DTO, dirty/revision, browser matching
and KDBX persistence for both rejected set and clear requests.

`vault/import.rs` retains Bitwarden source password history as a protected,
collision-safe JSON custom field. Actual command/wire/bulk/KDBX tests cover
fidelity, masking and batch rejection; no new DTO or fabricated native history
snapshot is introduced.

`src-tauri/vendor/keepass-0.13.22/` is the complete upstream package plus nine
reviewed attachment/history/location/deletion/serialization/KDF/timestamp/metadata patches. Cargo's local patch keeps
the version pinned; `vendor/KEEPASS_PATCH.md` and its provenance JSON describe
the changes. `scripts/verify-keepass-patch.mjs` verifies all 60 file hashes,
Cargo wiring and the nine patched files' formatting as part of `npm run verify`.
The untouched upstream tree stays outside Prettier; owned patch files remain
in the audit scope. Native vault tests cover the application boundary and thirty-two
dependency-specific tests cover shared references, guard unwind, missing-payload
rejection, exact-byte/protection deduplication and prepared-owner lifetime.
EntryTrack drop explicitly collects its released unowned snapshot IDs after
tracked removal erased reverse references. Recursive owners, remaining leases
and unrelated orphan headers are preserved; native tests cover six real
permanent-delete paths with current/history payloads and KDBX save/reopen.
Canonical binary merge and
complete local recycle-bin version transplantation live in patched types/mod.rs.
prepare_attachment_merge returns the opaque AttachmentMergeState; first-party
merge_databases consumes it with finalize_attachment_merge after restoring the
local bin and unioning all retained history. Final collection validates leases
and recursive owners, removes unowned prepared IDs, and preserves original local
orphan headers and later-added binaries. Native bidirectional/repeated/reopened
deletion and orphan-header tests accompany dependency lifetime/alias/error tests.
The first-party vault/merge.rs adds history-only union at equal current times
and content-aware deduplication of captured older current versions in either direction;
current same-time content conflicts still fail before database publication.
It completes current expiration from actual entry winners and rejects surviving
equal-time expiration divergence after native deletion decisions; expiration is
included in losing-version identity and its native save/reopen regressions.
Patched types/mod.rs provides History::from_entries for linear list construction
after the wrapper sorts versions. Patched db/merge.rs merges same-parent location
times independently of content, carries the winning move's previous parent
through staged group relocation and retains missing-time destination provenance.
Entry::content_eq excludes current/previous parent bookkeeping; first-party
history identity adds expiration. Group deletion restores required ancestry and
removes only empty candidates in child-first order after entry/group moves;
retained ancestors lose obsolete markers, Deleted events reflect actual removal.
Native tests cover deep entry/group edits and moved-out descendants; dependency
tests cover independent ancestor/child markers, unmarked descendants and existing
equal/missing timestamp policy. The nine-file provenance gate applies.

Patched crypt/kdf.rs validates the Argon2 u64 iteration count and byte-to-KiB
memory value with checked u32 conversions before derivation. Unrepresentable
parameters return the existing Argon2 errors instead of wrapping to low costs.
Shared library read/write paths all use this transform; native fixtures cover
local/remote open and failed-save publication. Standard KDF defaults are unchanged.

Patched format/xml_db/mod.rs encodes raw CR from the trusted text-only XML
serializer as character references before KDBX encryption. It preserves current/
history field values and keys, group/meta text and literal entity strings while
leaving normal parser normalization and protected base64 unchanged.
It validates actual XML 1.0 Char ranges before publication, rejecting forbidden
ordinary current/history/key/group/meta text with a content-free error while
retaining live edits and old file bytes. Protected Base64 scalars and allowed
supplementary noncharacters remain supported; no reader/sanitization change. Native
entry/history/group/meta and Bitwarden wire regressions accompany two direct
serializer/parser tests. Patched format/xml_db/entry.rs opts StringValue into
dedicated `$text` mapping with direct String conversion, preserving ASCII and
Unicode whitespace without changing generic optional metadata. Native current/
history and original-registry open/rewrite regressions use a hashed synthetic
fixture under tests/fixtures/kdbx-whitespace-*.

`vendor/quick-xml-0.41.0/` preserves the complete 43-file MIT package, with only
src/de/map.rs patched to retain blank events for dedicated `$text` structs.
Other maps keep indentation skipping. Its provenance verifier checks all hashes,
the exact version/path override and owned formatting. Original CRLF bytes and
the diff payload are preserved by explicit Git attribute exceptions; the other
42 files remain unchanged. README/schema/ordinary parsing are unchanged and
the separate registry QuickXML 0.42 dependency is unaffected.
`scripts/test-dependency-patch.mjs` tests source copies in a validated temporary
folder so standalone Cargo locks remain original. KeePass's 29 specialized
and QuickXML's serialize-enabled library tests run through npm and full verify;
upstream ignored tests remain limits. Patch descriptions/reconstruction are in
KEEPASS_PATCH.md and QUICK_XML_PATCH.md.

First-party vault/merge.rs::merge_absent_deletion_info runs after restore_bin.
It unions both original DeletedObjects maps plus existing merged markers for
UUIDs absent from final entries/groups/custom icons, retaining newer concrete
times and carrying unseen-object intent. Final survivors are excluded from new
marker imports; existing survivor/bin collision policy remains. Native tests
cover three-copy deletion/edits, unseen subtrees, unknown/disjoint timestamps,
final bin/icon objects and failed-merge input preservation. Vendor bytes are
unchanged by this first-party deletion-registry supplement.

## Runtime surfaces

Release signing runs in `sign-android`, a clean Linux job with no checkout or
build code. `android` uploads only unsigned APKs; the signer validates both
64-bit ABIs and signatures before `publish-android` uploads them. Windows
release notes travel with the build artifact, so `publish` has no checkout.

| Surface             | Entry point                                                                                         | Responsibility                                                                                                                                                                                                                                                                          |
| ------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Main desktop window | `src/routes/+page.svelte`                                                                           | Welcome/unlock, group tree, entry list, detail, search, editor dialogs, status                                                                                                                                                                                                          |
| Settings window     | `src/routes/settings/+page.svelte` → `SettingsDialog.svelte`                                        | Standalone settings page, theme/font application, settings navigation/panels                                                                                                                                                                                                            |
| Frontend services   | `src/lib/services/*`                                                                                | Settings store + bootstrap, vault IPC wrapper + browser fallback                                                                                                                                                                                                                        |
| GUI backend         | `src-tauri/src/lib.rs`                                                                              | Tauri setup, managed config + vault session, commands                                                                                                                                                                                                                                   |
| Backend modules     | `src-tauri/src/crypto/`, `src-tauri/src/platform/`, `src-tauri/src/config/`, `src-tauri/src/vault/` | Pure security primitives (AES/HMAC/hash/base64/OTP) and OS-integration services (auto-type, focus, shield, clipboard, credential, DPAPI); config persistence; KeePass open/create/edit/save session                                                                                     |
| Release automation  | `scripts/version.mjs`, `changelog.mjs`, `release.mjs`, `regression.mjs`                             | Atomic version bump, gitmoji changelog, two-pass release orchestration; `regression.mjs` (`npm run regression`) runs `verify` then prints the manual PITFALLS checklist                                                                                                                 |
| CI / CD             | `.github/workflows/ci.yml`, `release.yml`                                                           | `npm run verify` gates push/PR and release; tagged build creates a Windows NSIS + portable ZIP draft release, while a parallel Linux job builds, verifies, and uploads the signed 64-bit split-per-ABI (`arm64-v8a`/`x86_64`) Android APKs; missing artifacts/signing fail the workflow |
| Windows installer   | `src-tauri/windows/installer.nsi`                                                                   | Custom NSIS template wired via `bundle.windows.nsis.template`                                                                                                                                                                                                                           |

SvelteKit runs as a static SPA: `src/routes/+layout.ts` disables SSR and awaits `appSettings.initialize()` before route load. `+layout.svelte` imports global CSS and applies settings to the document.

## Frontend ownership

| Path                                                | Ownership                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/app.css`                                       | Global reset, theme defaults, font variables, shared modal backdrop/layers, accessibility media rules; imports shared settings CSS                                                                                                                                                                                                                                                                                              |
| `src/lib/styles/settings-shared.css`                | Reusable settings-panel primitives (header/cards/toggles/slider/feedback)                                                                                                                                                                                                                                                                                                                                                       |
| `src/lib/components/templates/`                     | Shared Button, TextField, Toggle, Feedback and menu primitives; ModalShell owns modal framing and actions.                                                                                                                                                                                                                                                                                                                      |
| `src/lib/types/theme.ts`                            | `ThemeColors`, `DARK_THEME_COLORS`, `LIGHT_THEME_COLORS`                                                                                                                                                                                                                                                                                                                                                                        |
| `src/lib/types/settings.ts`                         | `AppSettings`, `GeneralSettings`, `SecuritySettings`, `DatabaseDefaults`                                                                                                                                                                                                                                                                                                                                                        |
| `src/lib/types/vault.ts`                            | `VaultState`, `VaultGroup`, `VaultEntry`, request/input shapes                                                                                                                                                                                                                                                                                                                                                                  |
| `src/lib/utils/theme.ts`                            | `ThemeColors` → CSS-variable mapping                                                                                                                                                                                                                                                                                                                                                                                            |
| `src/lib/utils/password.ts`                         | Password generator, entropy estimate, strength label                                                                                                                                                                                                                                                                                                                                                                            |
| `tests/*.test.mjs`                                  | Node built-in behavior tests for frontend pure utilities; included in `npm run verify` via `test:frontend`                                                                                                                                                                                                                                                                                                                      |
| `src/lib/utils/csv.ts`                              | RFC 4180 CSV build/parse (import + export); the importer recognizes LastPass export headers via aliases (`name`→title, `extra`→notes, `grouping`→group)                                                                                                                                                                                                                                                                         |
| `src/lib/utils/tree.ts`                             | Vault-group primitives: cached `buildVaultTreeIndex`, `buildGroupPathIndex`, `buildEntryCounts`, `totalExcludingBin` (root minus bin badge total), and atomic browser helper `setGroupsExpandedInTree`; covered by `tests/tree-counts.test.mjs`                                                                                                                                                                                 |
| `src/lib/utils/entry-search.ts`                     | Advanced search predicate: field scope, regex, exclude/invert, expired/favorite/tags/quality filters; `prepareAdvancedSearch` compiles once for many entries                                                                                                                                                                                                                                                                    |
| `src/lib/i18n/`                                     | UI dictionaries (`zh-CN.ts` source of truth, `en.ts` type-checked parity) + `t(locale, key, vars)` with `{var}` interpolation, `<key>_one` singulars, and locale → zh-CN → key fallback; covered by `tests/i18n.test.mjs`; language is the existing `general.language` setting (see `docs/i18n.md`)                                                                                                                             |
| `src/lib/utils/display-rows.ts`                     | Entry-list row builder (`buildDisplayRows` + `DisplayRow`): flat entry rows or per-block group-separator headers from page-sorted entries, entry-only total counted in the same pass; covered by `tests/display-rows.test.mjs`                                                                                                                                                                                                  |
| `src/lib/utils/entry-nav.ts`                        | Separator-aware entry-row navigation (`findNext/NextEntryIndex`, clamped `pageUp/PageDownTargetIndex`, entry-only `entryPositions` for `aria-posinset`): keyboard focus and announcements always land on entry rows; covered by `tests/entry-nav.test.mjs`                                                                                                                                                                      |
| `src/lib/utils/virtual-list.ts`                     | Shared fixed-height range helper; EntryTable uses entry-row-layout.ts                                                                                                                                                                                                                                                                                                                                                           |
| `src/lib/utils/long-press.ts`                       | Touch long-press tracker for the entry list (`createLongPressTracker`): at most one armed press per gesture, the first (innermost) registration wins because pointer events bubble, and the trailing click after a fired long press is suppressed once; covered by `tests/long-press.test.mjs`                                                                                                                                  |
| `src/lib/utils/templates.ts`                        | Entry-templates helper (`collectTemplateEntries`): direct entries of the database's `entryTemplatesGroup` (nested groups not expanded); empty when unset or dangling so the editor hides its template selector; covered by `tests/templates.test.mjs`                                                                                                                                                                           |
| `src/lib/utils/format.ts`                           | Byte-size display formatting (`formatBytes`) and the compact entry description (`formatEntryDescription`: URL with scheme stripped, empty when no URL)                                                                                                                                                                                                                                                                          |
| `src-tauri/src/vault/exchange.rs`                   | KeePass binary entry exchange: backend-owned plaintext framing, optional DPAPI wrapper and dedicated clipboard wipe; no renderer whole-entry plaintext utility.                                                                                                                                                                                                                                                                 |
| `src/lib/utils/hibp.ts`                             | HIBP dialog display-state resolution (`hibpResultState`): a cancelled run never maps to the clean state, even with zero findings; covered by `tests/hibp.test.mjs`                                                                                                                                                                                                                                                              |
| `src/lib/utils/contact.ts`                          | Contact detection and inline link tokens: `classifyContact` (whole-value URL/email/phone), `detectContacts` (dedupe extraction), `linkifyContacts` (position-merged token stream for notes)                                                                                                                                                                                                                                     |
| `src/lib/utils/disposable.ts`                       | Owned-reference lifecycle helpers: dispose replaced/cleared resources once and reject late consumers that no longer own the current reference                                                                                                                                                                                                                                                                                   |
| `src/lib/utils/import-groups.ts`                    | Import group-path resolver that reuses indexed groups and keeps every async subgroup creation bound to the captured session                                                                                                                                                                                                                                                                                                     |
| `src/lib/utils/date.ts`                             | Date formatting (column `YYYY-MM-DD`, `datetime-local` input value, localized long date)                                                                                                                                                                                                                                                                                                                                        |
| `src/lib/utils/kdbx-xml.ts`                         | KeePass 2.x XML import parser (DOMParser → `XmlImportEntry[]`, groups as `A / B` paths, PlainXml values; legacy SecPivot Base64 compatibility; rejects encrypted fields, custom fields)                                                                                                                                                                                                                                         |
| `src/lib/utils/totp.ts`                             | Browser-fallback TOTP (RFC 6238, WebCrypto); desktop uses backend `totp_code`                                                                                                                                                                                                                                                                                                                                                   |
| `src-tauri/src/crypto/otp.rs`                       | Pure one-time-password primitives: TOTP (RFC 6238), HOTP (RFC 4226), Steam Guard; seed parsing + field contract                                                                                                                                                                                                                                                                                                                 |
| `src/lib/utils/clipboard.ts`                        | Clipboard copy + scheduled clear (`clipboardClearSeconds`) + `scheduleExchangeWipe` (backend wipe for the binary KDBX entry-exchange payload, which the text wipe cannot see)                                                                                                                                                                                                                                                   |
| `src/lib/services/settings.ts`                      | `appSettings` store, defaults, normalization, debounced persistence                                                                                                                                                                                                                                                                                                                                                             |
| `src/lib/services/settings-bootstrap.ts`            | Apply settings to document (theme colors, font vars, window effect)                                                                                                                                                                                                                                                                                                                                                             |
| `src/lib/services/security.ts`                      | `lockVault`/`copySensitive`/`armIdleLock`/`installAutoLock`/`installFocusLock`/`beginTcatoOverlayOpen`/`setTcatoOverlayOpen` (lock lifecycle, clipboard-clear gating, TCATO pending-lease + confirmed-event focus-lock guard)                                                                                                                                                                                                   |
| `src/lib/composables/useTotpCode.svelte.ts`         | Shared reactive OTP poller (fetch loop, countdown, HOTP static code, copy+flash); the OTP skins (`TotpWidget`, `EntryTotpBadge`) are pure renderers over it                                                                                                                                                                                                                                                                     |
| `src/lib/composables/usePanelLayout.svelte.ts`      | Main-window panel layout state machine: group/detail widths, detail auto-open with right-click suppression, mobile drawer, pointer-drag resize, persistence ordering (entryColumns written before panelWidths)                                                                                                                                                                                                                  |
| `src/lib/composables/useVaultSelection.svelte.ts`   | Entry-selection model: single select, shift-range over visible rows, ctrl/cmd toggle, anchor bookkeeping; the page drives flows that react to selection (detail auto-open, post-save rebind) through its accessors                                                                                                                                                                                                              |
| `src/lib/composables/useEntryFilter.svelte.ts`      | Search/filter pipeline (`useEntryFilter`): lazy search-text memoization per snapshot entry, subtree filtering honoring per-group `enableSearching`, advanced-query integration; sort keys/direction stay in the page + columns service                                                                                                                                                                                          |
| `src/lib/composables/useEntryEditor.svelte.ts`      | Entry-editor dialog flow (`useEntryEditor`): open modes (create/edit/edit-multi with multi-row fallback to single) and the session-guarded save pipeline (create→returned-UUID rebind, multi→selectedEntry-only rebind); the page keeps selection ownership and lookup helpers                                                                                                                                                  |
| `src/lib/utils/menu-items.ts`                       | Pure builders for the main window's context menus (`buildEntryMenuItems`/`buildBlankMenuItems`/`buildToolbarMenuItems`): state flags in, `ContextMenuItem[]` out; the page keeps only the action dispatchers. Covered by `tests/menu-items.test.mjs`                                                                                                                                                                            |
| `src/lib/components/AppToolbar.svelte`              | Main-window toolbar (new/save/save-as/lock, search box + advanced-filter toggle, overflow menu or detail/report/export/settings actions, window controls); owns all toolbar-scoped styles incl. 720px/420px responsive rules; page passes state flags + callbacks, binds `search` two-way, and calls the exposed `focusSearch()` for Ctrl+K                                                                                     |
| `src/lib/components/AutotypePickDialog.svelte`      | TCATO/global auto-type candidate picker over `ModalShell`: performs the `autotype_pick` invoke itself (failure surfaced via `onerror` → page toast) and closes immediately for responsive UX                                                                                                                                                                                                                                    |
| `src/lib/services/io.ts`                            | Import/export orchestration and native pickSaveFile for CSV/XML/HTML and both attachment saves. Android uses SAF MIME/document URIs; desktop uses dialog. Caller view/session/busy/notify ownership remains with the page or keyed child; import conversion/group resolution, browser downloads and export-row mapping live here.                                                                                               |
| `src/lib/services/columns.svelte.ts`                | Entry-table column service (`useEntryColumns`): persisted visibility/width/order, custom-field columns, KeePass-style extra columns (`historyCount`/`attachmentCount`/`autoTypeEnabled`/`overrideUrl`/`uuid`, all hidden by default), grid template, memoized sort keys and cell display text; the page keeps selection and sort direction                                                                                      |
| `src/lib/services/keyboard.ts`                      | Shortcut action catalog (`KEYBOARD_ACTIONS`), stored-binding merge (`effectiveShortcuts`), pure combo matcher (`matchesShortcut`) and first-match dispatcher (`dispatchShortcut`, consumes the event); the page only supplies the action-id handler map                                                                                                                                                                         |
| `src/lib/services/favicon-flow.ts`                  | Favicon download flow (`runFaviconDownload` over a `FaviconFlowHost` hook object): progress-dialog state machine around `vault.downloadFavicons`, live `favicon-progress` events gated by session staleness; the page owns the dialog `$state` and busy flag                                                                                                                                                                    |
| `src/lib/services/group-flows.ts`                   | Group/recycle-bin mutation flows over a `GroupFlowHost` (create/rename/meta/icon/delete/empty/restore/move/expand): staleness gating, busy tokens and confirm wording live here; the page owns dialog `$state` and passes narrow callbacks (`closeModal`, `resetSelectedGroup`, `stillTarget`) so branch behavior stays byte-identical. Gated by `component-contracts.test.mjs` view-epoch assertions                           |
| `src/lib/services/vault.ts`                         | `vault` store: open/create/close/save + entry/group CRUD + `remembered` path, plus multi-session tabs (`tabs`/`activeId` stores, `setActiveSession`, `closeTab`, `closeAll` lock path, `list_sessions` sync) (Tauri + browser)                                                                                                                                                                                                  |
| `src/lib/services/tips.ts`                          | Global transient tip/toast queue (`showTip`/`dismissTip`, success/error kinds), rendered by `TipsHost.svelte` in the layout                                                                                                                                                                                                                                                                                                     |
| `src/lib/data/demo-vault.ts`                        | Browser-preview fallback data; not proof of desktop KDBX behavior                                                                                                                                                                                                                                                                                                                                                               |
| `src/lib/components/AppIcon.svelte`                 | Hand-written inline SVG icon set (stroke `currentColor`)                                                                                                                                                                                                                                                                                                                                                                        |
| `src/lib/components/ContextMenu.svelte`             | Right-click menu items over `ViewportMenuShell` (icons, destructive/disabled states, fixed scrollable cascades following parent scroll/resize)                                                                                                                                                                                                                                                                                  |
| `src/lib/components/ColumnConfigMenu.svelte`        | Column visibility picker over `ViewportMenuShell` (sections, checkbox rows, footer hint)                                                                                                                                                                                                                                                                                                                                        |
| `src/lib/components/ViewportMenuShell.svelte`       | Shared viewport-clamped, size-bounded scrollable menu surface (Escape/click-outside close, role/aria label, resize-aware position measurement)                                                                                                                                                                                                                                                                                  |
| `src/lib/components/SettingsDialog.svelte`          | Settings shell: sidebar nav + content pane + panels                                                                                                                                                                                                                                                                                                                                                                             |
| `src/lib/components/settings/*`                     | General / Appearance / Security / Database / Remote / Integrations / About panels; canonical `SettingToggleCard` and `SettingRangeCard` templates                                                                                                                                                                                                                                                                               |
| `src/lib/components/BridgeApprovalPrompt.svelte`    | Global browser-association approval modal; listens for `bridge-associate-request` and answers via `bridge_approve` (mounted in `+layout.svelte`, skipped on the TCATO overlay)                                                                                                                                                                                                                                                  |
| `src/lib/components/RpcSideChannelPrompt.svelte`    | Global KeePassRPC side-channel modal; listens for `rpc-side-channel-request`, shows the one-time SRP password with a countdown/copy (mounted in `+layout.svelte`, skipped on the TCATO overlay)                                                                                                                                                                                                                                 |
| `src/lib/components/TipsHost.svelte`                | Global bottom-center transient tip host over the tips queue (mounted in `+layout.svelte`, skipped on the TCATO overlay)                                                                                                                                                                                                                                                                                                         |
| `src/lib/components/StandaloneVaultShell.svelte`    | Standalone shell for welcome/lock (logo/title/subtitle + slot); shared outer for VaultWelcome/LockScreen, theme-token driven                                                                                                                                                                                                                                                                                                    |
| `src/lib/components/VaultCredentialFields.svelte`   | Shared credential form (password/confirm/keyfile/showPassword/error, `bind:`) used inside ModalShell for VaultWelcome open/create/remote and inside LockScreen                                                                                                                                                                                                                                                                  |
| `src/lib/components/VaultWelcome.svelte`            | Welcome/unlock via StandaloneVaultShell + VaultCredentialFields; open/create/remote flows are now ModalShell dialogs (`small/medium` scrollable) with Button/TextField/Toggle                                                                                                                                                                                                                                                   |
| `src/lib/components/LockScreen.svelte`              | Lock screen via StandaloneVaultShell + VaultCredentialFields (password/keyfile) + Hello/switch actions; reopen remembered path                                                                                                                                                                                                                                                                                                  |
| `src/lib/components/ModalShell.svelte`              | Canonical modal backdrop/surface/header/icon/close/size/scroll shell; body and actions are supplied as snippets; traps Tab focus inside the dialog, focuses the surface on open and restores the invoker focus on close                                                                                                                                                                                                         |
| `src/lib/components/EntryDetail.svelte`             | Detail pane (fields/meta/attachments/history tabs): secret reveal/copy with session+UUID epoch guard, inline field edit after protected reveal, dual-mode notes editor (read view with inline clickable contacts via `linkifyContacts` + textarea edit) with debounced auto-save via `update_entries`; the attachments tab is a drag-and-drop dropzone that adds files in memory via `add_attachments` (fields never rewritten) |
| `src/lib/components/EntryTable.svelte`              | Windowed entry-table UI, indexed mixed-height spacers, keyboard focus, and transient column drag/resize state; `+page.svelte` owns sorted data, persisted columns, and vault mutations                                                                                                                                                                                                                                          |
| `src/lib/components/TotpWidget.svelte`              | OTP code readout with countdown bar; TOTP/Steam refetch per period, HOTP shows a static code + counter                                                                                                                                                                                                                                                                                                                          |
| `src/lib/components/EntryTotpBadge.svelte`          | List-row OTP badge (TOTP/Steam countdown bar; HOTP static, no bar); copy on click                                                                                                                                                                                                                                                                                                                                               |
| `src/lib/components/TotpQrButtons.svelte`           | TOTP seed picker icon buttons embedded inside the entry editor's TOTP seed input (right-aligned in `.totp-input-wrap`; "截图选取" region-select overlay over a GDI screenshot + "屏幕识别" full-screen scan); decode errors float under the input; decodes via `capture_screen_png`/`decode_barcode_png`, multiple hits show a picker, result flows to `onpick`                                                                 |
| `src/lib/components/GroupAutoTypeDialog.svelte`     | Group Auto-Type settings dialog (inherit/enable/disable + default sequence) over `ModalShell`                                                                                                                                                                                                                                                                                                                                   |
| `src/lib/components/GroupMetaDialog.svelte`         | Group properties dialog (notes, comma-separated tags, search participation toggle) over `ModalShell`; saves via `update_group_meta`                                                                                                                                                                                                                                                                                             |
| `src/lib/components/AttachmentPreviewDialog.svelte` | In-memory attachment preview dialog (text/image data URL/binary) over `ModalShell`; loads via `preview_attachment`, saves to disk via the existing `save_attachment`                                                                                                                                                                                                                                                            |
| `src/lib/components/SimilarPasswordsDialog.svelte`  | Similar-password report dialog (edit-distance clusters, passwords never leave the backend) over `ModalShell`; loads via `similar_passwords`, click a member to locate the entry                                                                                                                                                                                                                                                 |
| `src/lib/components/HibpCheckDialog.svelte`         | HIBP breach-check dialog over `ModalShell`: privacy screen (k-anonymity explanation) with an explicit start, results list with occurrence counts, click to locate an entry                                                                                                                                                                                                                                                      |
| `src/lib/components/ExpiredEntriesDialog.svelte`    | Expired-entry maintenance dialog over `ModalShell`: lists `expired_entries`, extends 30 days via `update_entries` or moves to recycle bin via `delete_entries`, locates entries                                                                                                                                                                                                                                                 |
| `src/lib/components/LargeEntriesDialog.svelte`      | KeePass "Find → Large Entries" dialog over `ModalShell`: view-only list of all non-recycle-bin entries sorted by size descending (snapshot `VaultEntry.size`, no IPC), optional minimum-KB filter, click locates the entry; opened from the toolbar ⋯ menu (`large-entries` id, `+page.svelte` passes `allEntries`/`pathOf`)                                                                                                    |
| `src/lib/components/ChangeTimelineDialog.svelte`    | Vault-wide change-timeline dialog over `ModalShell`: loads `change_timeline`, groups events by day (time, entry title/username, changed-field chips from the backend `HistoryDiff`), click locates the entry                                                                                                                                                                                                                    |
| `src/lib/components/DatabaseSettingsDialog.svelte`  | Database storage-settings dialog (KDF/cipher/compression/history cap/recycle flag) over `ModalShell`; reads via `get_database_settings`, writes via `update_database_settings`                                                                                                                                                                                                                                                  |
| `src/lib/components/AdvancedSearchDialog.svelte`    | Advanced search filters over `ModalShell` (field scope, regex/exclude, expired/favorite/tags/quality) plus named saved searches (load/delete/save current); applies to the current view                                                                                                                                                                                                                                         |
| `src/lib/components/VaultTabs.svelte`               | Tab bar over the open backend sessions: switch (`set_active_session`), close per tab (`close_vault` by id), live dirty marker for the active tab and parked dirty flags from `list_sessions`; rendered only when >1 tab                                                                                                                                                                                                         |

## Backend ownership

`src-tauri/src/loopback.rs` owns the desktop browser servers' shared accept
policy and RAII connection permits. `BridgeState` and `RpcState` each retain
one budget across listener stop/start cycles: at most 32 connection workers per
service, including incomplete HTTP/WS requests. Excess connections are dropped
before thread creation; completed, failed or unwound handlers release permits.
Each start uses a fresh `ListenerLifetime`, registering sockets and approval
cancellation senders with RAII cleanup. Stop rejects old-generation dispatch
and approvals before joining the listener. `loopback/stream.rs` supplies the
`CancelableStream` used by HTTP and tungstenite: 100 ms native polling checks
cancellation without shortening logical protocol read deadlines. This avoids
Windows clone shutdown leaving an existing recv blocked. Already admitted
storage jobs may finish; their permits are retained until worker return.
The listener join alone still does not mean every admitted job has exited.
RPC stop clears its live-session list before releasing the server mutex, so
it cannot erase a restarted listener's records. Association prompts do not yet
consume a cancellation event. RPC SRP prompts consume correlated
`rpc-side-channel-ended` events from `rpc/server/prompt.rs`: each connection owns
a secret-free attempt lease, ended at the next setup message or connection drop.
`vault/session_end.rs` supplies weak one-shot active-lifetime observers registered
by RPC leases. Successful open/create adoption, active close, lock and switch
end them centrally in `VaultSessions`, outside registry/observer guards and
while the caller still holds the active vault mutex. No per-command hook can be
missed by local or remote adoption; failed/idempotent changes and parked close
do not end the current lifetime. Callback unwind is contained.

`rpc/server/handshake.rs` validates cached connection keys against live host
authorization before read dispatch and stored-key proof completion. Its
`dispatch_setup_for_session` binds pending SRP proofs to stable vault-session
ids. `rpc/server/mod.rs::prepare_authorized_rpc_write` performs the same key
validation after writer admission and before vault write preparation; blocking
persistence and durable completion retain their existing lock-free handoff.

`vault/import.rs` defines shared import DTOs and the Bitwarden parser;
`vault/import/onepassword.rs` parses real standard/Windows JSON-line 1PIF.
`onepassword_tests.rs` covers true schema, strict failures, concealment and
serde → batch EntryInput → KDBX save/reopen. The frontend `io-import.test.mjs`
exercises the real bulk IPC mapping with a fake backend boundary. 1PIF folders
and binary attachments are refused rather than silently dropped; unmapped secure
contents/history are preserved as protected JSON fields. Native client export
verification remains missing. `vault/import/onepux_archive.rs` provides a
native-only bounded version-3 ZIP reader with an opaque `OnePuxArchive`;
`onepux_archive_tests.rs` uses tiny independent Python stored/deflate/ZIP64
fixtures. `vault/import/onepux.rs` prepares a native protected plan and owns
`VaultSession::import_1pux` candidate-database publication/rollback;
`onepux_tests.rs` verifies account/vault identity, source archives, exact binary
binding, masking, typed-string secret getter usability and synthetic KDBX reopen.
Compound/multi-key source values remain protected JSON. `commands/import.rs` exposes the
bounded spawn_blocking read/prepare and stable-origin completion, covered by
`commands/import_tests.rs`. `io.ts::importOnePux`, `vault.ts::importOnePux` and the
page's `handleImportOnePux` own the new existing-menu/picker route; Node tests in
`tests/io-onepux.test.mjs` use production page/service code with synthetic API
seams. Real client export/provider/dialog execution remains separate pending
evidence; the earlier native-only stages alone did not complete this route.

`vault/website_rules.rs` owns the exported native `WebsiteMode`, `WebsiteRule`,
`WebsiteRuleSet` and immutable `ValidatedWebsiteRules` API. It shares the pinned
PSL helper and canonical authority utility, while retaining per-rule modes and
raw exact URL bytes. `website_rules_tests.rs` covers strict schema/destination/
resource gates, mixed rules and a synthetic protected KDBX round trip.
The same module owns native `VaultSession` policy getter/setter, protected managed
field publication and creation/import validation. `website_rules_storage_tests.rs`
exercises actual Bridge/RPC URL routing, candidate rollback/no-op/read-only behavior,
generic edit preservation, bounded history/restore, native XML and in-memory entry
exchange. `helpers.rs` gives native policy positive-rule precedence with existing
KPRPC vetoes and excludes policy entries from title-only heuristic auto-type;
`hosts.rs` rejects RPC URL-policy changes/custom overwrites before persistence.
Session IPC, frontend editor/history policy display and source-format mapping are
separate pending work; native tests do not establish real client/provider execution.

`commands/clipboard/scheduler.rs` provides the shared latest-deadline worker
for `commands/clipboard.rs`. Text and entry-exchange wipes share one pending
fingerprint slot; replacement/cancel drops it immediately. Clipboard I/O runs
outside the scheduler mutex and rechecks generation under the OS clipboard lock.

| Path                                      | Ownership                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`                    | Tauri builder, managed state (`AppState`), command registration, setup, system tray (Show/Lock/Quit + minimize-to-tray close handling), global hotkeys (classic auto-type + TCATO summon, register/re-register + handlers), window close handling, `RunEvent::Exit` clipboard clear, TCATO window-close event                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/commands/mod.rs`           | All Tauri IPC command handlers, grouped by domain; module root re-exports every command + `sync_bridge`/`sync_rpc`/`TcatoTarget` for `lib.rs` (extracted from lib.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/commands/config.rs`        | Config IPC: `get_config`/`set_config`/`app_info` (portable flag + config/data paths for the About panel), bridge/RPC server sync, backup-template normalization                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src-tauri/src/commands/bridge.rs`        | KeePassHttp/RPC status + browser-association approval commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `src-tauri/src/commands/credential.rs`    | Credential-store (Windows Hello quick unlock) commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/commands/vault.rs`         | Vault lifecycle (open/create/close/state/save/save-as/change-key) + entry CRUD + TOTP/favorite/auto-type + screen-capture guard                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src-tauri/src/commands/groups.rs`        | Group CRUD + recycle-bin empty commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `src-tauri/src/commands/entries.rs`       | On-demand secrets and maintenance reports; CSV/XML/HTML export with temporary zeroizing payloads; bounded UTF-8 CSV/XML/JSON/1PIF reader accepts local paths or Android document URI plus display name; structured import parsers.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/commands/favicon.rs`       | Favicon download command + WinINET proxy parse + HTTP fan-out helpers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/commands/qrcode.rs`        | Screen capture (`capture_screen_png`, GDI virtual-desktop BitBlt) + multi-format barcode decode (`decode_barcode_png`, rxing: QR/DataMatrix/Aztec/PDF417/1D); NotFoundException maps to an empty hit list; unit tests generate a QR and round-trip it through the decoder                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `src-tauri/src/commands/tcato.rs`         | TCATO overlay commands + managed target state; emits `tcato-overlay-open`/`tcato-overlay-close` events (backing the frontend focus-lock guard)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `src-tauri/src/commands/clipboard.rs`     | Clipboard read/clear commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/commands/remote.rs`        | S3/WebDAV remote commands: resolve canonical profile path, list objects, open remote, create + upload                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/net.rs`                    | Shared `reqwest` client builders: Android pins `webpki-root-certs` via `tls_certs_only` (avoids the uninitialized `rustls-platform-verifier` panic); desktop/iOS keep platform verification; S3/WebDAV/favicon/HIBP all route through it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src-tauri/src/commands/tests.rs`         | Command-layer unit tests                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src-tauri/src/config/mod.rs`             | Config module root: `pub mod settings`/`store`, re-exports (`AppConfig`, `normalize_config`, `ConfigStore`, `RECENT_FILES_MAX`, `DEFAULT_BACKUP_TEMPLATE`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `src-tauri/src/config/settings.rs`        | `config.json` serde shapes (mirror `AppSettings`), defaults, normalization incl. remote backup template + file extension (extracted from config.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/config/store.rs`           | Data-root resolution (`resolve_data_dir`: portable when a `portable.flag` marker or legacy `conf/config.json` sits beside the executable, else app-data dir) + atomic persistence to `<data_dir>/conf/config.json` (`read_config`/`write_config`) + managed `ConfigStore` with `data_dir()`/`is_portable()`/`config_path()` accessors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/config/tests.rs`           | Config schema/defaults/normalization/persistence test suite (extracted from config.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/vault/mod.rs`              | Vault module root: `VaultSession` struct + `RemoteMode`/`RemoteTarget`, shared field/event constants, `vault::*` re-exports (`dto` types, `helpers`/`persist` free functions)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/vault/session.rs`          | `VaultSession` lifecycle impl: open/create/close/state/save/change_master_key/save-as, adopt/replace, snapshot + prepare_save/change/complete_save/change internals (extracted from mod.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `src-tauri/src/vault/file_io.rs`          | Bounded vault/keyfile/text-import reads and write_named for vault/export/attachment writes. Local paths use atomic replacement; mobile ContentResolver writes read old bytes first, verify writes and rollback, and zeroize temporary readback/rollback buffers. Native provider behavior still requires device tests.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/vault/sessions.rs`         | Multi-database session registry, addressed-session helpers, and process-local persistence gate; tests cover session switching, gate serialization, and completion after a tab switch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `src-tauri/src/vault/temp_attachments.rs` | Controlled temp extraction for external attachment viewing (`AttachmentTempStore` managed state): random token dirs under `<system temp>/secpivot-attachments/`, sanitized names, session-bound capped reads outside the vault lock, per-session/all-session cleanup outside the vault lock; failed removals remain registered for retry                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `src-tauri/src/vault/breach.rs`           | HIBP k-anonymity range check: SHA-1 prefix (5 hex chars) grouping, range-response parsing, full-hash matching strictly local; `hibp_entries` collects uuid/title/username/password rows (recycle bin excluded, optional uuid filter); mock-server tests assert only the prefix appears on the wire                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/vault/import.rs`           | Shared structured-import DTOs and plaintext-only Bitwarden JSON parser; vault/import/onepassword.rs parses actual standard/Windows JSON-line 1PIF, preserving hidden fields/OTP/URLs and refusing unsupported folder/attachment migration.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `src-tauri/src/vault/entries.rs`          | `VaultSession` entry & group CRUD impl: add/update/move/delete, matching/quality/foreground flags (`update_entry_flags`), history + restore + clear-all-history, group add/rename/set-icon/meta (notes/tags/search)/delete/restore, recycle-bin empty (extracted from mod.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/vault/security.rs`         | `VaultSession` OTP/favorites/favicons/auto-type impl: totp_code, toggle_favorite, favicon jobs + apply, autotype context/expand/match, get_entry_password/totp, attachment preview (text/image data URL/binary, 2 MiB cap), security_report, similar-password clustering (edit distance ≤ 2, capped), expired-entry listing (recycle bin excluded), change timeline (`change_timeline`: consecutive history-snapshot transitions newest first, recycle bin excluded, capped at 500, no secrets), export_csv, KeePass 2.x XML export (`export_xml_content`, PlainXml secrets with ProtectInMemory), HTML emergency-sheet export (passwords opt-in + warning banner, HTML-escaped) (extracted from mod.rs)                                                                                                                                                  |
| `src-tauri/src/vault/persist.rs`          | Lock-free persistence primitives: `SaveTarget`/`SaveJob`, remote refresh/merge jobs + results, `prepare_local/remote_open/create`, `persist_snapshot/save/save_with_db/change/remote_refresh/remote_merge` (remote writes re-read + hash-compare first, `REMOTE_CHANGED` conflict errors; `save_with_db` returns the persisted clone for settings adoption), `read_keyfile`, attachment/CSV writers                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/vault/exchange.rs`         | KeePass "Data Exchange" payload core: `build_entries_document` (entries → KDBX XML document, attachments in `<Binaries>`, custom icons in `<Meta><CustomIcons>`), `parse_entries_document` (complete internal payload preparation, including typed custom metadata), `encode_payload`/`decode_payload` (KeePass framing `[flags u32 LE][gzip(...)]`, optional DPAPI wrap with the `ClipDomainSep` entropy), `insert_entry` (paste under a **fresh UUID**, restoring fields/attachments/tags/custom data/auto-type/colours/expiry/icon). Mirrors KeePass' `EntryUtil` byte layout; the KDBX 4.1 `PlainXml` container KeePass wraps the document in is **not** emitted, so cross-client paste is not yet interoperable, and entry history is not exchanged (no public API to append a historical entry). Covered by in-file `#[cfg(test)]` round-trip tests |
| `src-tauri/src/vault/helpers.rs`          | Free functions shared by the session: `parse_entry_id`/`parse_group_id`/`recycle_bin_id`, auto-type match walkers, `entry_match_urls` (native non-Never policy URLs, or legacy primary + KPRPC altURLs), full `KPRPC JSON` parse (`kprpc_config`) + `kprpc_matches_url` (KPRPC blocked vetoes → native policy when present, else legacy regex/host tiers; invalid/conflicting native rules fail closed), OTP field resolution, `build_database_key`, vault header probe (`probe_vault`: KDBX/KDB/unknown + size, no decryption), `save_database`/`write_database_bytes`, KDF/cipher/compression application, secret wipe helpers (extracted from mod.rs)                                                                                                                                                                                                  |
| `src-tauri/src/vault/tests.rs`            | Session/entry/group/OTP/favicon/security/remote/autotype/RPC/bridge test suite (extracted from mod.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/vault/hosts.rs`            | Bridge/RPC host adapters, bridge update history, RPC DTO mapping, and transactional RPC write prepare/persist/complete with originating-session replay                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/bridge/mod.rs`             | KeePassHttp protocol module root: `BRIDGE_PORT` + shared constants (`KEY_LEN`/`NONCE_LEN`/`PROTOCOL_VERSION`), re-exports `crypto`/`types`/`dispatch`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/bridge/crypto.rs`          | Field crypto: `encrypt_field`/`decrypt_field` (AES-256-CBC + PKCS7), `make_verifier`/`check_verifier`, response `response_hmac`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src-tauri/src/bridge/types.rs`           | Wire shapes (exact camelCase): `BridgeRequest`/`BridgeEntry`/`BridgeResponse` (+ success/failure envelope builders), `BridgeLogin` + `BridgeHost` trait, `decrypt_request_field`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `src-tauri/src/bridge/dispatch.rs`        | Request dispatch: verifier gate + `handle_request`, two-phase `associate` validation/completion with a zeroizing pending key, `new_client_id`, `dispatch`, entries build/decrypt helpers, `generate_password`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/bridge/tests.rs`           | Protocol-core tests: crypto vectors, verifier/HMAC, dispatch + associate/reject flows (extracted from mod.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/bridge/server.rs`          | Loopback HTTP server (127.0.0.1:19455), `BridgeState` lifecycle, `ApprovalBoard`, lock-free associate approval wait bound to the stable backend-active session, HTTP framing, and browser-write UI notification                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src-tauri/src/crypto/mod.rs`             | Crypto module root: re-exports shared loopback primitives (AES-256-CBC + PKCS7 raw/b64, HMAC-SHA256, SHA-1/SHA-256, hex/base64, CSPRNG, constant-time MAC compare)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/crypto/primitives.rs`      | Shared loopback crypto: AES-256-CBC + PKCS7 (raw/b64), HMAC-SHA256, SHA-1/SHA-256, hex/base64, CSPRNG, constant-time MAC compare (single source for bridge/rpc/vault)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/rpc/mod.rs`                | KeePassRPC protocol module root: constants (`RPC_PORT`/`PROTOCOL_VERSION`/`SECURITY_LEVEL`/`FEATURES`), SRP group + mod-pow math, `hex`/`random_hex` re-exports, re-exports `dto`/`srp`/`keys`/`dispatch`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `src-tauri/src/rpc/dto.rs`                | Wire shapes (KeePassRPC v1 camelCase): `Envelope`/`SrpMessage`/`KeyMessage`/`ErrorMessage`/`JsonRpcFrame`, `RpcError`, host DTOs + `RpcHost` trait, write-path `RpcFieldWrite`/`RpcLoginWrite`, `write_username/password/custom_fields`, `merge_urls` (`MergeInNewURLs` semantics)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/rpc/srp.rs`                | SRP-6a server handshake: `SrpServer::begin`/`verify_proof` (incl. Kee SRPc negative-S quirk)/`secret_key` (pure, no sockets)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `src-tauri/src/rpc/keys.rs`               | Session-key handling: key-auth challenge/response (`key_auth_cr`/`key_auth_sr`), `secret_bytes`/`hex_decode`, AES-256-CBC + keyed SHA-1 MAC frame `encrypt_frame`/`decrypt_frame`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `src-tauri/src/rpc/dispatch.rs`           | v1 JSON-RPC dispatch, DTO conversion, and shared validation/parsing for read and write methods                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `src-tauri/src/rpc/tests.rs`              | Protocol-core tests: SRP math/proofs, key-auth, frame encrypt/decrypt/tamper, URL merging, JSON-RPC dispatch (extracted from mod.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `src-tauri/src/rpc/server/`               | Loopback WebSocket lifecycle/transport, handshake/frame helpers, connection state, and tests; RPC writes persist outside the session lock                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `src-tauri/src/platform/mod.rs`           | Platform module root: OS-integration services (auto-type, focus, shield, clipboard, credential, DPAPI)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/platform/autotype.rs`      | KeePass-style auto-type sequence parser + `enigo` keystroke replay; `{REF:...}` field-reference expansion                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `src-tauri/src/platform/focus.rs`         | Windows-only foreground-window title reader (Win32) for global auto-type matching; TCATO `WM_CHAR` channel injection                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `src-tauri/src/platform/shield.rs`        | Screen-capture guard (`SetWindowDisplayAffinity`) for the main window                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `src-tauri/src/platform/clipboard.rs`     | Clipboard read/clear/write helpers (commands surface clipboard text + scheduled clear)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `src-tauri/src/platform/credential.rs`    | Keyring-backed saved-credential store (Windows Hello quick unlock)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/platform/dpapi.rs`         | Windows DPAPI secret protection for config persistence (`dpapi:`-prefixed values)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `src-tauri/src/remote/mod.rs`             | Remote module root: `RemoteStorage` trait, `RemoteObject`, shared tokio runtime, `make_storage` factory, `list_objects_async`, `local_storage_dir`/`MemoryStorage` re-exports                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/remote/s3.rs`              | S3 transport: path-style requests signed with SigV4, blocking reqwest client, conditional ETag upload and capped downloads.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `src-tauri/src/remote/webdav.rs`          | WebDAV transport: blocking reqwest + PROPFIND multistatus parsing (quick-xml), Basic auth, `url_for`/`encode_path_segment`/`href_to_key` helpers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `src-tauri/src/remote/memory.rs`          | In-memory fake storage (`MemoryStorage`, `Arc<RwLock<HashMap>>`) for offline tests                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/remote/local.rs`           | Local mirror folder resolution: `local_storage_dir(app, profile_path)` → `<data_dir>/Storage/remote/<kind>/<sanitized name>` (data root from `ConfigStore`: portable = beside the executable, installed = app-data dir), plus canonical path validation / `sanitize_dir_name`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/remote/tests.rs`           | Transport tests: in-memory fake, S3 + WebDAV local TCP mocks, multistatus parser, URL encoding                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `src-tauri/src/remote/backup.rs`          | Local mirror write + backup rotation for remote vaults: key validation/basename, backup-template expand/match, write + prune retention (pure std + chrono; extracted from vault.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/util.rs`                   | Small cross-module helpers: URL host extraction (single source for bridge/rpc/vault URL matching), atomic tmp+rename file write                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `src-tauri/src/vault/dto.rs`              | IPC serde DTOs (camelCase) shared with the frontend: entries/groups/state/inputs/patch/OTP/security/favicon shapes + tri-state icon deserializer (re-exported via `vault::*`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/vault/serialize.rs`        | Snapshot building + entry mutation for the vault session: favicon host collection, `VaultState` tree/entry build, CSV/entropy/history helpers, `EntryInput`/`EntryPatch` field writes, custom-fields/attachment sync (extracted from vault.rs)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

`VaultSessions` also owns the runtime `rpc.matchByRegistrableDomain` mode and the `rpc.sessionTimeoutSecs` SRP key lifetime. Startup and settings updates synchronize both to the active slot, every parked session, and sessions opened later; the registry test suite covers propagation across open and switch operations.

`utils/menu-keyboard.ts` owns shared viewport-menu focus, level-scoped arrow/
Home/End navigation, Escape/Tab dismissal and capture-listener cleanup.
`ViewportMenuShell` attaches it on mount; `ContextMenu` handles cascade entry/
return and guarded post-tick child focus. MenuItem exposes submenu ARIA with
instance-scoped IDs. `tests/menu-keyboard.test.mjs` verifies ownership/lifetime
with a deterministic DOM seam; native browser button/Tab behavior requires
rendered verification. Appearance reuses the existing global focus outline.
The page's earlier window-capture shortcut handler checks the shared
`activeContextMenu` owner, including menus opened by recursive GroupNode;
document capture alone cannot block an earlier window listener.
`security.ts::installAutoLock` observes keys in window capture so consumed
menu navigation still refreshes idle time; cleanup uses the same capture flag.

## Mobile (Android) surface

Android support is scoped and wired into the release workflow: platform-specific
backend code is desktop-gated (`#[cfg(desktop)]`), mobile-only config lives in
`src-tauri/tauri.android.conf.json` and `src-tauri/capabilities/android.json`, and
the frontend hides desktop chrome through `isMobile()`/`isAndroid()`. GitHub run
`39409873118` built and signed both 64-bit split APKs (`arm64-v8a`, `x86_64`) for
v1.6.3. Mobile file picking uses the local `tauri-plugin-saf`
(`src-tauri/plugins/saf`): `ACTION_OPEN_DOCUMENT`/`ACTION_CREATE_DOCUMENT` with
persistable grants, display names in `general.recentFileNames`, and
`vault/file_io.rs` for ContentResolver I/O. Android TLS uses the bundled
`webpki-root-certs` via `src-tauri/src/net.rs`. Lifecycle locking, touch UX, and
real-device behavior remain incomplete — see `docs/android.md`.

The Android capability binds `saf:default` to `windows: ["main"]`; Tauri has no
`screens` capability field. `src-tauri/tests/android_acl.rs` resolves the generated
plugin manifests for Android on any host and verifies picker access from the local
main window, isolation from other windows/remote pages, and exclusion on desktop/iOS.

## Persistent layout

```text
<data_dir>/                # resolved by config::resolve_data_dir
├─ portable.flag           # optional marker forcing portable mode (shipped in the portable ZIP)
├─ conf/
│  └─ config.json          # AppConfig
└─ Storage/remote/         # remote-vault local mirrors (<kind>/<profile>)
```

Portable mode (marker or legacy `conf/config.json` beside the executable) keeps `<data_dir>` next to the executable so the whole folder travels on a USB stick; installed mode uses the per-user app-data dir. User databases live where the user chooses (`.kdbx`). The vault session is in-memory only; nothing is stored outside the chosen `.kdbx` file except the optional last-path setting.

## High-coupling files

Treat these as integration points and avoid concurrent edits:

- `src/routes/+page.svelte`
- `src/lib/components/SettingsDialog.svelte`
- `src/lib/types/settings.ts` + `src/lib/types/vault.ts`
- `src/lib/services/settings.ts` + `src/lib/services/vault.ts`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/` (mod + config + bridge + credential + vault + groups + entries + favicon + tcato + clipboard + remote + tests)
- `src-tauri/src/remote/` (mod + s3 + webdav + memory + local + backup + tests)
- `src-tauri/src/config/` (mod + settings + store + tests)
- `src-tauri/src/vault/mod.rs`
- `src-tauri/src/bridge/` (mod + crypto + types + dispatch + server + tests) + `src-tauri/src/rpc/` (mod + dto + srp + keys + dispatch + server + tests) + `src-tauri/src/vault/hosts.rs` (protocol ↔ server ↔ session boundaries)
- `TODO.md`
- `SKILL.md` and shared references

## Structure update rule

When adding, removing, renaming, or moving a route/module/component/service, update this map and the focused reference that describes its contract. Describe stable ownership; do not add transient implementation notes or raw file listings that will immediately go stale.

`src/lib/utils/clipboard.ts` owns a per-renderer promise queue for text copy,
conditional native wipe and explicit clear. In-flight writes precede lock clear;
old queued copies are epoch-cancelled. `security.ts::lockVault` starts session
closure independently of that queue. Synthetic `clipboard-service.test.mjs`
regressions cover write/lock order, rejection recovery, same-content conditional
clear order and synchronous idle start. Cross-window/native binary order and
external clipboard sequence ownership remain outside this queue.

`security.ts::copySensitive` owns temporary active-id/state observers and a
`KeyedViewGuard` for its nonsecret copy-follow-up lifetime. `clipboard.ts` admits
its optional afterCopy hook only for completed current copies. `vault.ts` checks
the optional closeAll predicate inside the existing topology queue; no backend
command or new global secret state is added. `tests/security-copy.test.mjs` uses
the real service graph with synthetic Clipboard API/IPC seams to cover stale,
queued, cancelled, browser-path, current and failure follow-ups.

`src/lib/services/saf.ts` wraps the existing Android open/create-document plugin;
optional MIME arrays preserve default KDBX behavior. `io.pickSaveFile` calls it
for exports and both attachment-save components, retaining caller view guards.
`vault/persist.rs` text and attachment writers delegate to `file_io::write_named`;
its tests cover exact local bytes and failed destination preservation, while
`tests/io-export.test.mjs` runs production page/component handlers with synthetic
platform invokes. This is not Android provider/runtime verification.

Bitwarden JSON import rejects true or invalid encryption markers before item
mapping; account-encrypted files share the plaintext envelope and require this
gate. Its command zeroizes incoming text. Dynamic linked-field/multi-URI
fidelity and actual client exports remain separate pending work.
The same parser's optional_string/optional_array helpers reject malformed common
values instead of dropping them. Login/folder/field members and numeric 0–3
custom types are validated before rows return; missing legacy type stays Text.
Four shape regressions cover whole-document rejection and valid optional/string
values through the actual command/wire/bulk/KDBX path, including hidden masking.

`vault/import/fields.rs` owns source-prefixed unique custom-label allocation for
Bitwarden and 1PIF. `import/bitwarden_tests.rs` exercises the real command, shared
ImportRow wire fields, bulk mutation and KDBX save/reopen with synthetic data,
checking reserved/blank/duplicate retention, masking and OTP/KPRPC semantics.

`vault/import.rs` retains Bitwarden types 1–8 with protected typed fields and
an optional quality flag. `vault/dto.rs::ImportEntryInput` adds import-only
metadata to flattened EntryInput. The `commands/vault.rs::import_entries` wrapper
calls `VaultSession::import_entries`, sharing validate-first insertion with
`add_entries`; non-password records disable quality evaluation at creation.
`io.ts` and the browser bulk fallback preserve this flag. Mixed metadata and
failed-batch tests verify KDBX reopen/default behavior without real user data.

`vault/import/bitwarden_links.rs` maps official login/card/identity linked IDs
into protected snapshots for the existing custom-field allocator. It derives
Identity FullName from source name parts and rejects unknown/malformed targets.
`bitwarden_tests.rs` checks all target IDs, optional empty values, stale literals,
invalid mixed documents and KDBX protection; dynamic linkage is not implemented.

`vault/helpers.rs::registrable_domain` uses the pinned `psl`/`psl-types` static
Mozilla list, including private suffixes, wildcards and exceptions, for opt-in
Domain accuracy in the shared Bridge/RPC matcher. It borrows the domain slice
and retains full IP/nonregistrable host identity. No runtime fetch or config/IPC
schema change is involved. Credential-boundary tests use synthetic reopened KDBX.

`vault/import.rs::default_login_uri` validates Bitwarden URI collections and
rejects multiple destinations or explicit source match modes until native
per-URI policy support exists. Errors contain only record numbers and reason;
the command returns no partial rows. `import/bitwarden_tests.rs` covers the
negative command boundary plus supported default/no-URI KDBX round trips.

`util.rs::url_host` owns canonical destination-host extraction through the
existing url crate (userinfo exclusion, IDNA, IPv4/IPv6 and authority validation),
shared by URL matching/title helpers. Its unit boundary and reopened synthetic
KDBX Bridge/RPC tests cover spoofed userinfo, invalid authority and canonical
host equivalence; no frontend or IPC surface is added.

`vault/helpers.rs::url_matches_accuracy` implements Exact as identical complete
URL strings after host validation. `vault/hosts.rs` passes original request/submit
URLs without lowercasing. `utils/match-url.ts` retains the full trimmed input when
suggesting an Exact rule, and `tests/match-url.test.mjs` covers that
value boundary. The backend tests exercise the shared Bridge/RPC policy after
synthetic KDBX reopen; no markup or theme changes are involved.

`util.rs::url_authority` owns canonical host plus explicit numeric-port identity
for Hostname accuracy; `url_host` delegates without exposing ports to existing
host-only consumers. `match-url.ts` validates WHATWG authority and preserves
ports/IPv6 brackets in Hostname suggestions. The strict-JSON shared corpus in
`tests/fixtures/url-authorities.json` runs in Rust and Node tests, with suggested
alt rules verified through reopened synthetic KDBX Bridge/RPC adapters. Domain
keeps its broader port scope; no new frontend component or IPC surface exists.

# tldts 7.4.2

The unchanged MIT-licensed published `tldts` 7.4.2 CommonJS bundle provides
Bitwarden-compatible hostname extraction and public/private suffix handling.
The bundle embeds its parser and PSL data; it has no external `require` calls.
It is used only by the native Bitwarden source profile, leaving standard native
Site/Host semantics unchanged.

Upstream: https://github.com/remusao/tldts

Published package: https://registry.npmjs.org/tldts/-/tldts-7.4.2.tgz

Package integrity selected by Bitwarden clients commit
`4035e2332dab338d9046a1d7319c644b4368682c`:

`sha512-kCwffuaH8ntKtygnWe1b4BJKWiCUH30n5KfoTr6IchcXOwR7chAOFJxFrH3vjANafUYrIA4a7SDL+nn7SiR4Sw==`

| Repository path                | Published source                | SHA-256                                                            |
| ------------------------------ | ------------------------------- | ------------------------------------------------------------------ |
| `tldts-7.4.2/index.cjs.min.js` | `package/dist/index.cjs.min.js` | `fd2168b91e566b90616774678e73e7a91010ce20da3d35817b5ed8a2abc93856` |
| `tldts-7.4.2/LICENSE`          | `package/LICENSE`               | `c64182d48160db948b6aa97a984f1a32974ce4d2053900361264cd33853796cb` |

No source changes, format changes or EOL normalization are allowed for these
two files. `scripts/verify-tldts.mjs` checks both working-tree bytes and, with
`--staged`, the exact staged bytes. The build embeds the bundle using
`include_str!`; it fetches nothing at build or runtime.

The worker initializes this static module once, without user inputs. URI text
is passed only to its fixed exported function. `parse` creates fresh result
objects; the bundle's retained state is static PSL data, regexes, boolean and
numeric lookup cursors. No URI cache is used. Pending exceptions are cleared
and unreachable query objects collected after each request; native owned input
strings are zeroized. Regex matching uses separate fresh bounded contexts.
This does not claim JavaScript or allocator-wide memory erasure.

The independent semantic fixture `tests/fixtures/bitwarden-uri-official.json`
records hashes of the original Bitwarden TS source methods and the exact bundle.
Its 61 synthetic cases execute the original source under Node, with original
punycode and the actual equivalent-domain selection method. The observable
adapter is synchronous; account/policy services and real exports are outside
that oracle. GPL Bitwarden source is used as read-only test-reference material
and is not shipped or copied into the application.

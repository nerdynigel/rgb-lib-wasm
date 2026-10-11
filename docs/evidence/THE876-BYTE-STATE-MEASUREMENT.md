# THE-876 test-only byte-state measurement — 2026-10-11 UTC

Scope: Chief comments e1d6834e / 7ff8af60, registered engine workspace
f85beea3-1dd0-403d-aa2d-9f495e2cfef0; nerdynigel/rgb-lib-wasm,
branch feat/the-876-transfer-export, clean starting revision
1028ba6353a27d4f35ade9b10768dd75528478bc. Delivered revision is the commit
containing this file (reported with independently verified origin/PR3 SHA).
Source/test-only delta; production function bodies, dependency/pin/loader and
served JS/WASM artifacts unchanged. Full acceptance OPEN.

## Measurement contract and real boundaries

`tests/support/state_bytes.rs` is included only by Rust unit tests or the
WASM integration test. It decodes the three actual strict stock serializations
from existing `wallet::idb_store::serialize_stock` (stash, state, index, in that
order). It compares decoded bytes directly and emits only length/SHA256 receipts.
`deserialize_stock` supplies the independently restored Stock for reserialization.

`wallet::offline::Wallet::rgb_runtime` moves stock into the real internal
`utils::RgbRuntime`; its Drop returns that stock to the wallet. The new internal
test measures both sides directly, tests snapshot refusal while runtime is held,
and checks sequence remains unchanged on that refusal. It does not call a
second rgb_runtime while one is active. This is empty-stock checkout/drop proof,
not interruption of a financial transition.

Private `Wallet::snapshot` is the exact existing flush construction boundary.
It INCREMENTS sequence; it is not a read-only getter. Tests distinguish:

- Raw serialized snapshot bytes, including sequence.
- Canonical JSON state bytes: recursively sorted object keys, only top-level
  sequence removed; database, BDK changeset, pending artifacts, received bytes,
  reuse indexes and all strict stock strings retained. Equality here is normalized
  snapshot-state equality, not exact persisted-byte equality.
- Strict stock component bytes, compared without normalization.

`tests/wallet_wasm.rs::durable_snapshot_bytes` reads the actual UTF-8 string
from IndexedDB `rgb_lib_wallet` version1, store `snapshots`, at fresh
`Wallet::idb_key()`, in a read-only transaction, awaits completion and closes its
connection. It does NOT reserialize `load_snapshot` or call snapshot/flush.
The fresh key is never printed. Browser assertions compare the exact stored
bytes across export/inspection refusals, drop and `Wallet::restore`. Baseline
explicit flush writes sequence1, next explicit flush sequence2, and explicit
flush after restoration sequence3; normalized state and strict stock bytes
remain equal across these explicit writes. No profile/financial setup is used.

The existing synthetic contract round-trip test now compares all three strict
components before/after JSON snapshot restore, and verifies each differs from
empty stock. Its dummy seal/DumbResolver/local contract is not real funded
issuance or authenticated chain state. An internal BDK address derivation is a
sensitivity control: snapshot state changes while RGB stock bytes remain equal.

## Executed validation and receipts

All commands used `/home/nigel/.cargo/bin/cargo` because bare cargo is absent
from this run PATH. Initial bare `cargo fmt` / `cargo test` exited127 and did not
run; explicit known user binary succeeded. No installation or host change.

- cargo test --locked --lib byte_state_runtime -- --nocapture: PASS1, FAIL0.
- cargo test --locked --lib stock_serialize_round_trip_with_contract -- --nocapture:
  PASS1, FAIL0; all three synthetic populated stock components survive byte-exact.
- cargo test --locked --lib color_export_unfunded_refusals_preserve_stock_and_records:
  PASS1, FAIL0; existing PSBT/database/stock refusal regression preserved.
- cargo test --locked --target wasm32-unknown-unknown --test wallet_wasm --no-run:
  exit0; test artifact wallet_wasm-7976edaa0d409ec1.wasm.
- GECKODRIVER=/usr/local/bin/geckodriver MOZ_HEADLESS=1
  WASM_BINDGEN_TEST_ONLY_WEB=1 NO_PROXY=localhost,127.0.0.1
  no_proxy=localhost,127.0.0.1
  /home/nigel/.cache/.wasm-pack/wasm-bindgen-8a74166b323833fc/wasm-bindgen-test-runner
  target/wasm32-unknown-unknown/debug/deps/wallet_wasm-7976edaa0d409ec1.wasm
  test_color_export_fresh_wallet_refusal --exact --nocapture: real headless
  Firefox PASS1, FAIL0. Initialization deprecation warning only.
- cargo fmt --all -- --check; git diff --check: exit0.

Observed receipt lengths (hashes are emitted by the executable tests):

| Boundary | Stash/state/index bytes | Other measured bytes |
| --- | --- | --- |
| Internal empty-stock runtime | 20 / 10 / 22 | normalized snapshot1399, serialized1412 |
| Synthetic contract stock | 4633 / 393 / 137 | normalized snapshot7736, serialized7749 |
| Fresh unfunded Firefox | 4432 / 10 / 22 | stored snapshots7296 each, normalized7283 |

Firefox stash SHA256:
75f73ba4bbae4b4c35941c19e7bce32390a2ca3066bc55df5faf4050f6e76d48.
Firefox stored baseline SHA256:
2a8bfffd708a882d80287ebad4a7a0bb9036bbc9d835dbcd5b2ae5333368c543.
Firefox stored sequence2 SHA256:
53ec9abfc97731b0130e3d604c6aca5538e93ccdd0d77eb8de8d6633f2648c26.
These are attributable one-run receipts, not fixed fixture expectations:
fresh random wallet keys, contract seals and map iteration can change hashes.
No keys, raw snapshot bytes, mnemonic, wallet identifier or credential is logged.

## Corrected acceptance / Reliability handoff

| Requirement | Delivered evidence | Remaining |
| --- | --- | --- |
| Actual strict stock/snapshot measurement | Internal runtime + synthetic stock + Firefox stored byte checks | Bind fresh four-role operation journal to its exact fresh record/source |
| Refusal preservation | Existing real engine refusal + exact persisted bytes in Firefox | Positive validated RGB and allocation negatives |
| Runtime checkout/drop | Actual internal empty-stock boundary, snapshot guard | Financial interruption and consumption accounting |
| Flush/restore | Actual unfunded IndexedDB commit/drop/restore/explicit flush | Forced commit failure and immediate process crash |
| Served all-recipient fee settlement | No new evidence in this slice | Full inherited live/independent gates remain |

Reliability on existing THE-872 can consume the test receipt protocol
`THE876_BYTE_STATE {boundary, stock:[{bytes,sha256}...], ...}` and the existing
read-only IndexedDB measurement implementation. A fresh fixture binding must
attribute its exact key/record to the operation locally and output only safe
length/hash/sequence receipts; it must not substitute another wallet, historical
profile, public projection or the synthetic test contract. This receipt does
not grant that fixture execution authority.

There is still no secure production JS stock/snapshot getter, and this patch
adds none. Internal helpers are not callable through the pinned secure facade.
For a measurement harness using its OWN fresh Rust Wallet, existing idb_key,
load_snapshot, serialize_stock and deserialize_stock suffice; no production
capability is necessary for that path. To bind measurements to the served
secure wallet, Chief/Reliability must reconcile a same-origin test-only
IndexedDB diagnostic at the attributable fresh record (or request a separately
bounded capability). The current backend cannot claim this binding merely
from these receipts. Any proposed new capability must return only three strict
component lengths/SHA256, exact durable record length/SHA256 and sequence;
require current secure owner/lifecycle guard and the exact fresh operation key;
refuse active runtime/missing record/decoding errors; perform read-only IDB with
no snapshot call, sequence increment, consume, flush, sign or broadcast. That
is a contract proposal for Chief, not implemented/authorized production API.

Chief next action: reconcile this usable measurement receipt with Reliability's
existing partial backend febb779604545044262e9494690fadeb71c834e7 and choose the
bounded test-only fresh-record binding. Software retains implementation/live
proof ownership after explicit preparation handoff. No new task or cycle.

## Compatibility and rollback

All consumer/pins/artifacts remain: protocol2a849f20/1.9.0;
btcx263f92fed35eeab235f7997ebb80e56e5a279ca4;
wallet57a226366ee093364fb12b09d474510e5d54a05a;
RLNc8bad5125d85f90c2d879d0615321d5ad3f8ab2e;
signer074de663746b654bbd1cb9756e34c531101edec8;
lightning0939f7751fb876e02caec9cd0a778794b81eee9e.
Served WASM025dfd7a483a723413eb173bb9e89ec5515a1941cdcb4b675e4876f4ae1fb7b7;
JSbf488bfefca54fa7192ed4bb3b8b614c1742da7a83e233a274591fa96be24c20
unchanged; test WASM compilation is not a runtime repin/rebuild/publication.
Breaking protocol change NO; peer source/artifact migration NO.
Rollback: revert this test/evidence commit; no state deletion/reset or operation
replay. No funded setup, issuance, financial receive, signing, broadcast, native
edit, loader extension, runtime promotion, merge or downstream hold removal.
Native403/accounting/crash/independent gates and THE-870/872/873/874 holds remain.
NOT SAFE TO ACCEPT/merge/release.

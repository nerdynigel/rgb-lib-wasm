# Browser prospective transfer export — unfunded source validation

Issue: THE-876; scope document revision cd57d63f-a6fb-4577-8a66-7b4686a020ce.
Repository: nerdynigel/rgb-lib-wasm. Registered workspace
f85beea3-1dd0-403d-aa2d-9f495e2cfef0; branch feat/the-876-transfer-export.
Clean starting HEAD: e9422e7faa939bdbc4528531c3f0901840eee164.
Recorded 2026-10-11 UTC. Source compilation and unfunded refusal checks now pass; full acceptance remains open.

## Implementation

`Wallet::color_psbt_and_export` reuses established `color_psbt` and the pinned
rgb-ops 0.11.1-rc.10 `Stock::transfer_from_fascia` via the existing RgbRuntime
adapter. Each contract's nonzero output beneficiaries derive from the same
committed fascia and witness. No alternate stock/store, custom consignment,
consume, flush, high-level transfer registration, signing or broadcast is added.
The caller PSBT is cloned and only replaced after all exports succeed.
Empty asset requests and unavailable/active stock are refused. The existing
coloring vout bounds check now rejects a vout equal to the output count.

The additive JS `colorPsbtAndExport` accepts a base64 PSBT and JSON ColoringInfo;
returns the colored PSBT plus strict serialized consignment byte arrays.
ColoringInfo uses existing snake_case field names and contract-ID keys, with
integer vout keys in output_map. It carries explicit allocations, not fee policy.

These exports are derivation primitives. Caller-local input ownership, foreign
prevout resolution, immutable approval binding and independent checks of every
principal/change/fee allocation remain required in the facade/wallet consumers.
`color_psbt` can insert an OP_RETURN and shift vouts; consumers must compare the
returned committed transaction to their approved transaction and refuse any
unapproved substitution. No acceptance claim about those consumers is made.

## State and persistence boundary

Static control-flow inspection: export does not call consumption or record writes.
Runtime acquisition moves the initialized wallet stock out of its shared slot;
Drop returns it. The export refuses an absent stock rather than initializing an
empty replacement. The synchronous call has no await/yield between acquisitions.
Wallet construction, schema import, random blinding generation and PSBT coloring
are separate effects; this API is not globally zero-write. This browser runtime
uses an in-memory shared slot, not a filesystem lockfile. Construction/load,
IndexedDB persistence and drop effects were measured on a fresh unfunded Firefox
wallet as described below. No successful-export effects have been measured.

Authored native refusal test compares strict stash/state/index bytes, high-level
database JSON, PSBT bytes and persistence sequence before/after an unowned
contract error, and verifies refusal while a runtime holds the shared stock.
Authored fresh unfunded browser test compares transfer records and PSBT bytes
after refusal. Both checks executed successfully after the toolchain repair. The browser test
additionally compares durable snapshots before/after refusal, explicit caller
flush, wallet drop and restore. No successful-export state proof is claimed.

## Historical checkpoint validation (superseded toolchain blocker)

- `/home/nigel/.cargo/bin/cargo fmt --all -- --check`: exit 0.
- `git diff --check`: exit 0.
- `/home/nigel/.cargo/bin/cargo check --locked --target wasm32-unknown-unknown`:
  exit 101 at secp256k1-sys; clang not found.
- `/home/nigel/.cargo/bin/cargo check --locked --workspace --target wasm32-unknown-unknown`:
  exit 101 at secp256k1-sys; clang not found, before checking product code.
- `/home/nigel/.cargo/bin/cargo test --locked --lib color_export --no-default-features`:
  exit 101 at openssl-sys; pkg-config and OpenSSL development headers unavailable.
- `/home/nigel/.cargo/bin/cargo test --locked --lib color_export`:
  exit 101 at the same dependency boundary. Zero executed tests.

No dependency versions, Cargo.lock, consumer pins, artifacts or held profiles
changed. No funded fixture, service setup or privileged installation performed.

## Repair follow-up validation — 2026-10-11 UTC

Caller availability: `/usr/bin/clang`, `pkg-config --modversion openssl` = 3.5.7.
Reliability repair receipt is published in BTCX at
2d1daee9bf4cd6c31f4a908704e427c176edbe9b; Chief released the build hold.
No dependency or lock changes. Test-only change qualifies the Bitcoin Transaction
name directly rather than relying on an esplora-gated alias.

Executed on the source/test tree delivered by this evidence commit:

- `/home/nigel/.cargo/bin/cargo check --locked --workspace --target wasm32-unknown-unknown`:
  exit 0, both library and JS binding crates checked.
- `/home/nigel/.cargo/bin/cargo test --locked --lib color_export`:
  exit 0, PASS 1 / FAIL 0 / 113 filtered. Compares strict stock stash/state/index,
  DB JSON, caller PSBT and persistence sequence; empty/unowned-contract refusal
  and active-runtime refusal preserve state; runtime drop restores shared stock.
- `cargo test --locked --lib color_export --no-default-features`:
  exit 101, zero tests; 33 product compile errors before the alias correction.
  Existing feature gates leave idb_store referencing disabled online types,
  VSS referencing optional reqwest, snapshot fields absent and existing tests
  referencing disabled methods. The additive test alias was fixed; broader
  feature-gate repair is outside this change. Default production features pass.
- `PATH=/home/nigel/.cargo/bin:$PATH /home/nigel/.cargo/bin/wasm-pack test --headless --chrome --chromedriver /usr/bin/chromedriver -- --locked --test wallet_wasm`:
  WASM tests compiled; Chrome driver HTTP 404 before any test, exit 1.
  Initial invocations without cargo PATH and with an unsupported positional test
  filter failed before tests; corrected command above reached the runner.
- Exact affected browser test (existing runner 0.2.117):

```sh
GECKODRIVER=/usr/local/bin/geckodriver MOZ_HEADLESS=1 \
WASM_BINDGEN_TEST_ONLY_WEB=1 NO_PROXY=localhost,127.0.0.1 \
no_proxy=localhost,127.0.0.1 \
/home/nigel/.cache/.wasm-pack/wasm-bindgen-8a74166b323833fc/wasm-bindgen-test-runner \
target/wasm32-unknown-unknown/debug/deps/wallet_wasm-7976edaa0d409ec1.wasm \
test_color_export_fresh_wallet_refusal --exact --nocapture
```

  Exit 0, Firefox PASS 1 / FAIL 0 / 15 filtered, 0.78s. Runner serves an ephemeral
  loopback origin; no existing profile, service or financial fixture accessed.
  Wallet construction leaves no snapshot for the fresh wallet key. Explicit
  caller flush creates the baseline. Refusal preserves PSBT, transfer records,
  contract IDs and the ENTIRE durable snapshot including sequence. A second
  explicit caller flush changes only sequence (+1), preserving strict stock,
  transfer/received consignment artifacts, DB, BDK and address state. Drop and
  restore do not change the durable snapshot; restored transfers/IDs stay empty.
  Initialization imports schemas into memory; browser synchronization uses the
  shared stock slot, no filesystem lockfile. Active slot refusal/drop is covered
  by the separate stock test. This is refusal-path proof, not successful export.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0.

## Acceptance map and remaining owner work

| Requirement | Evidence / disposition |
| --- | --- |
| Additive browser source and JS export compile | WASM workspace check PASS |
| Unfunded no-consumption/no-record error paths | Stock test and real Firefox refusal/snapshot test PASS |
| Initialization, locking, persistence effects | Measured refusal lifecycle above; slot acquisition/drop test PASS |
| Successful export, same-approved-transaction every-recipient validation | OPEN; requires consumer binding and fresh operation-specific fixture plan before funded validation |
| Both directions × zero/maker-only/simultaneous fee served paths | OPEN; existing consumer proof is not replaced by these refusal tests |
| Native source delivery | Chief-owned native workspace/origin provisioning blocker |
| Accounting interruption/flush and immediate crash | OPEN; these tests do not exercise financial accounting or crash |
| Independent acceptance / release | WITHHELD; Chief reconciles, Verification owns independent acceptance |

Software owns isolated engine→RLN→wallet adoption and validation under the
current execution scope, after publishing this source gate. Consumer pins and
artifacts have not changed in this source validation deliverable. Positive
export and allocation validation must remain explicitly open until executed.

Native fork access remains Chief-owned, unavailable at the stated HTTP403
boundary; no native edits or denial retries. Served bilateral fee acceptance,
engine interruption/flush accounting and crash durability are still unresolved.
THE-870/872/873/874 holds remain intact.

Rollback: leave previous engine/facade/wallet pins and artifacts unchanged and
keep export capability disabled. If this source is adopted later, revert this
additive change on its task branch; never reset wallet state or replay cases.

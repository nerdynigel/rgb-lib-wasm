# Browser prospective transfer export — source checkpoint

Issue: THE-876; scope document revision cd57d63f-a6fb-4577-8a66-7b4686a020ce.
Repository: nerdynigel/rgb-lib-wasm. Registered workspace
f85beea3-1dd0-403d-aa2d-9f495e2cfef0; branch feat/the-876-transfer-export.
Clean starting HEAD: e9422e7faa939bdbc4528531c3f0901840eee164.
Recorded 2026-10-11 UTC. This is an incomplete source checkpoint, not acceptance.

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
IndexedDB persistence and drop effects have NOT been measured in a browser here.

Authored native refusal test compares strict stash/state/index bytes, high-level
database JSON, PSBT bytes and persistence sequence before/after an unowned
contract error, and verifies refusal while a runtime holds the shared stock.
Authored fresh unfunded browser test compares transfer records and PSBT bytes
after refusal. Neither test executed because dependency builds are blocked.
No successful export or durable before/after state proof is claimed.

## Executed validation

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

## Residuals and owner

Chief must route build capability setup to Reliability: clang supporting the
installed wasm32 Rust target, pkg-config and OpenSSL development dependencies,
then return this exact workspace to Software. Source compilation, executable
unfunded tests, browser effect measurement, successful export and all-recipient
validation remain open. Do not repin consumers or promote this candidate before
the source validation gate passes. Funded candidate validation requires the fresh
operation-specific disposable fixture plan; this checkpoint does not grant it.

Native fork access remains Chief-owned, unavailable at the stated HTTP403
boundary; no native edits or denial retries. Served bilateral fee acceptance,
engine interruption/flush accounting and crash durability are still unresolved.
THE-870/872/873/874 holds remain intact.

Rollback: leave previous engine/facade/wallet pins and artifacts unchanged and
keep export capability disabled. If this source is adopted later, revert this
additive change on its task branch; never reset wallet state or replay cases.

# THE-944 existing TEST-helper Clippy correction

PARTIAL SOURCE CHECKPOINT. No financial or acceptance release.

Chief comment 3c3b1826-cc53-410f-8818-254483932dfd explicitly requests this existing
minimal correction without suppression, grouped with native root composition.
The only engine source delta is make_test_wallet under cfg(any(test,
feature="test-fixtures")). It now uses restore_existing_snapshot over complete
local empty stock/BDK/database state instead of duplicating Wallet's struct and
allocating its Arc-backed database in the fixture. It restores the original
fixture's detached-stock precondition immediately. No new allow attribute,
pointer-type redesign, product constructor change, schema import, storage/network
operation or changed assertion. Default library bytes and dependency locks unchanged.

Actual validation, toolchain1.97.1 on PATH, CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup, RUSTUP_TOOLCHAIN=1.97.1, CARGO_BUILD_JOBS=1:

- cargo clippy --offline --locked --lib --features test-fixtures -j 1 -- -D warnings:
  PASS (4.46s); prior d85bd355 feature Clippy failure is superseded only here.
- cargo test --offline --locked --lib --features test-fixtures -j 1 -- --quiet:
  PASS153/153, 31.53s. Includes all six existing input-availability regressions.
- Changed helper rustfmt, exact preservation of previous test suffix, git diff
  --check and delivered show/remote checks: recorded with publication.

Initial attempt moving the database allocation to a local variable still failed
arc_with_non_send_sync, so it was replaced by the existing state constructor.
One Clippy invocation mistakenly used the BTCX cwd and failed because that workspace
has no test-fixtures feature; no BTCX validation inferred. Native formatter stdin
--check printed a formatting diff despite zero exit; the diff was applied.

No current facade/WASM/browser/financial result follows from these native tests.
Publish together with the facade's root controller-correspondence correction;
retain all financial fences. Roll back TEST helper/pins as a group, preserving all
consumed cases, roots, debits and pending obligations; never replay funds.

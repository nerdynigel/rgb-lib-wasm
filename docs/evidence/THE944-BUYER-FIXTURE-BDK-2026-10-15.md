# Buyer fixture: BDK precondition

PARTIAL / NOT SAFE TO ACCEPT. Existing SAME-Lead source continuation; no operator
or financial release. Predecessor ad5b895466b33e78046979bb1de6dc459b42f730.

TEST only: `src/wallet/offline.rs` extends the existing off-by-default
`test-fixtures` funding helper to apply the exact funding transaction through
BDK's normal unconfirmed transaction indexer and persist its changeset before
seeding the existing RGB database. It grants no ownership to a foreign script.
The existing helper still resets TEST stock; call only during initial fixture
setup, before issuance, receiving or authority. No production code path changes.

`src/wallet/input_availability.rs` adds a native regression using the real wallet
and BDK graph: a local funded output passes the unchanged BTC availability query;
a foreign funded output refuses. This repairs a missing positive buyer fixture
precondition, not a product ownership check. No synthetic accepted root/debit,
receipt or signing authority is introduced.

Executed with existing 1.97.1 toolchain bins on PATH, CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup, RUSTUP_TOOLCHAIN=1.97.1:

- `cargo test --offline --locked --lib --features test-fixtures input_availability::tests -- -q`: PASS 11/11, 147 filtered.
- `cargo clippy --offline --locked --lib --features test-fixtures -j 1 -- -D warnings`: PASS.
- `git diff --check`: PASS. Cargo manifests/lock unchanged.

Full facade/default+TEST build, actual buyer browser/IDB/native receipt and root
consumer, served nonzero bilateral consent, financial accounting, signing,
immediate process-kill, R1/R4/live reverse and independent acceptance remain OPEN.
The test evidence does not close any of those gates. Protocol1.9.0, signer and
Lightning repair are unchanged. Roll back this TEST helper and consumer source
pin together; preserve every pending/consumed/ambiguous obligation.

# Read-only native RGB input observation

Adds bounded REGTEST-only comparison of actual native Stock assignments, database
allocations and BDK unspent inputs by canonical ContractId/outpoint. Integer
amounts are decimal strings; non-fungible/inflation rights are explicit. No
address generation, transaction preparation, network call, signature, persistence,
Stock change or reservation release. The output explicitly grants no spending
authority. It diagnoses the actual new post-recovery preparing intent0b4ceeee;
it does not classify an unavailable original as safely failed or permit replay.

165native tests PASS, including3new real BDK/Stock tests with an invented graph.
Clippy--Dwarnings and owned formatter/diff pass. Tests compare exact DB/Stock/
snapshot sequence before/after; missing display allocations do not hide Stock,
and inflation rights do not become fungible spendable units. Initial compile
failure from unchecked parse-error conversion retained and corrected. No funded
native diagnosis/deployment/financial acceptance is claimed in this commit.

No dependency, snapshot format, signing path, chain, asset identity or fee change.
SDK secure facade and exact-pin source build are required before actual read-only
observation. Preserve every pending original including0b4ceeee; main untouched.

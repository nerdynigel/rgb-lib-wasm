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


Split-carrier follow-up: real BDK/standard RGB local invented graph matches
the post-recovery economics:1+3RGB on two1000sat carriers, separate93769sat
fee coin,2RGB recipient/2RGB change/ordinary Bitcoin change. Insufficient
carrier-only funding refuses; adding the assignment-free fee coin constructs
the expected4outputs, standard offchain consignment validates, Stock unchanged.
166native tests/Clippy--Dwarnings PASS. Equal-value fixture coins previously
conflicted accidentally; helper corrected using distinct invented parent
outpoints. Initial compile/fixture failures retained. Test-only addition, no
production dependency/signing/state change. This rules out simple split-input
construction in a fresh graph; it does not reproduce/fix the held0b4preparation
or prove actual customer settlement. See SPLIT-CARRIER-TESTS.json/logs.


Failed-history correction: ordinary RGB selection previously required all
historical allocations to be Settled, incorrectly rejecting an input after a
proven conflict/unsigned cancellation retained Failed Input rows. Reuse the
existing bounded Bitcoin native-original fence before dropping Failed history
from only the local selection copy. Managed, atomic and retained legacy signed
inputs remain excluded; missing/malformed originals, invalid conflict metadata,
signed/restored cancellations refuse. Caller holds, pending witness/blinded
promises, pending RGB, foreign assets and absent settled allocations refuse.
No persisted history/original/Stock/signing/snapshot/asset identity change.

Regression on real BDK/DB invented graph fails before fix and passes after.
172native cases/Clippy--Dwarnings/owned rustfmt/diff PASS;6new tests include
negative variants and exact DB history retention. Initial private test-helper
access compile failure retained/corrected. Read-only getter additionally reports
failed-history entry count and qualification under the same native fence; this
is not authority and does not apply host holds. Source build and actual R
read-only check still required.0b4unconfirmed intent remains held; no replay,
new signing, broadcast or safe cancellation is inferred from unavailable data.
See FAILED-HISTORY-CORRECTION.json and complete raw logs.

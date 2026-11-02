# Scoped ordinary RGB fee-input self-recovery candidate

Backend implementation only; not SDK-exposed, integrated or deployed. No actual
new recovery transaction has been prepared, signed, broadcast or confirmed.

The actual installed ordinary R RGB original cdd77c78-b822-416f-971b-6e99b671a724 /
9fdf0212bfd7635ae1006eb780f8d9b3f084034e8be61002f733f9092295a5b9 remains held.
A test-harness JSON parser crashed on the standard multipart consignment upload.
Native durable state retains the original signature, Prepared/Initiated,
signing_attempted=true, broadcast_attempted=false, restored=false. Trusted
regtest Core175 has no original and all three inputs unspent. Those observations
do not cancel it or grant replay. Its old approval is expired and unchanged.

This narrowly scoped API prepares/signs/submits a NEW Bitcoin self-payment from
one native-proven assignment-free fee input. It accepts only an expired fresh
ordinary original with retained default-SIGHASH signatures, a known pre-dispatch
native mark and unconsumed preparation. Restored, dispatched, unsigned, missing,
finalized and ambiguous state refuse. Readiness alone never retires the original.

Current BDK ownership/confirmation/maturity, every Stock contract, pending witness/
blinded receives and other managed native holds are checked through the existing
fee-input proof. Caller-owned other holds remain mandatory. RGB asset seals
cannot be recovery inputs. Only one input and one wallet-owned VANILLA output,
full value less an exact1..1000sat fee, no arbitrary destination/amount/change,
no nonbinding sighash, regtest genesis, and a separate120second approval qualify.
BDK performs construction/signing. A native snapshot commit precedes signing and
submission so a stale other-tab instance cannot authorize using its old mark.
The host must retain the NEW original/attempt and inputs before these methods.

No old transaction is re-signed/resubmitted, expiry changed, asset reconstructed,
original hold released, balance credited or operation marked cancelled. Existing
native/browser confirmed-conflict reconciliation must subsequently verify the
exact new Bitcoin spend and preserve the old original evidence. Only that proof
can retire the impossible unconsumed original and release its old RGB holds.
No mainnet, nonzero execution fee, custody/dependency/asset-identity change.

All162native library tests PASS (154existing +eightnew safety cases), library
Clippy-Dwarnings, format and locked build check pass. Tests use real BDK
signatures/Stock on invented local graphs; the reservation fixture is not a
valid RGB original or financial network. The initial compile error referenced
a computed response field as if stored; corrected by checking the actual
retained signed PSBT and every final witness. Raw failures are retained.
A broader --all-targets Clippy run fails on existing Arc/memory-DB host test
fixtures and host compilation of wasm-only integration imports. Those failures
are retained; do not claim all-targets lint passed. No tests/guards were weakened.

Next: secure SDK facade/source pins, wallet host refusal journal and new backup
profile, design-first explicit approval/recovery UI, exact source-built artifact
verification, guarded Lab release and actual bounded confirmed self-recovery.
Then complete ordinary RGB sending/received-output re-spend, atomic customer
both directions/partials/adverse recovery and independent acceptance. This native
component proof does not close those gates or the full product goal.

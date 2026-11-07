# Read-only retained preparation intent/receipt

Complete the safe negative-outcome recovery API with
bitcoin_input_preparation_intent(operation_id). It validates exact REGTEST,
canonical identity, bounded retained map and original consistency, then clones
only that existing record. No insertion, flush, selection, cancellation, signing,
broadcast, expiry/fence change or release authority. Missing/invalid IDs refuse,
never produce a negative outcome. This lets secure wallet recovery observe an
already-aborted original after a lost ACK or file restore without executing a
fresh cancellation. Pending/restored records remain observations, not permission.

254full Native library cases PASS (215.47s), one new case covers pending, aborted
and restored variants, absent/invalid IDs and no map/artifact mutation. Strict
library Clippy/fmt/locked wasm32 compile PASS. Existing raw broad-test lint limits
remain documented in the parent evidence. No dependency, lockfile, persistence
format, signer, Stock or monetary rule change. Prior snapshot6/backup8 still
apply. Local component fixtures are NOT funded/atomic acceptance. SDK getter and
host integration follow on the exact new pin; Lab/main untouched.

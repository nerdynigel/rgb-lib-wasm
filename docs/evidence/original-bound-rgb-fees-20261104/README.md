# Original-bound RGB recovery fee selection

Candidate fix, REGTEST only; no financial acceptance or mainnet promotion.

An expired signed RGB self-return can reserve a plain Bitcoin fee input. Ordinary
Bitcoin spending must keep that input excluded. The exact source-bound RGB
recovery can now qualify it for its own standard Native transfer, with the source
signed-byte digest, authoritative asset/units, own beneficiary, DB/BDK/Stock,
confirmed inputs and max miner fee checked by the existing engine path.

Preparation uses the narrowly checked source's fee view. Once the child exists,
read-only fee evidence can recognize the exact source/child pair in either
direction, so both original and child reviews remain possible. A third operation
still retaining a shared coin excludes it. Duplicate/invalid/recursive recovery,
unknown request JSON, wrong digest/asset/network/state and ordinary Bitcoin
spending retain fail-closed behaviour. No reservation, database row, Stock state,
original, backup field or version is removed or changed by these views.

Public methods/wire formats remain unchanged; two helpers are limited to the
existing online module. No custom crypto or unrelated dependency changes.
Wallet/SDK need exact pins and a genuine rebuilt WASM package before deployment.

Two new named Native tests passed in the running full226-case suite: original
held-fee selection leaves ordinary spendability0 and all DB/artifact bytes intact;
source/child fee evidence survives preparation and an unrelated third hold blocks
both reviews. Fixtures use real BDK signatures and standard NIA Stock on invented
unfunded graphs; no live indexer, outgoing consignment, broadcast or atomic proof.
The existing reservation helper now carries typed fixture request JSON rather
than an unparsable label; no assertion was weakened. An initial filename filter
matched0cases and is explicitly not acceptance. Full suite, strict Clippy and
wasm build remain pending at this commit checkpoint. Initial privacy compile
error is fixed with online-module visibility, not a new public API.

Actual W2RGB e0953b7e/X1RGB efdb13b1 remain held on current Lab13a63f27. No new
signature, replay, timeout release, issuance, funding or main merge is authorized
by this source note. Full recovery/customer/atomic/adverse acceptance stays OPEN.

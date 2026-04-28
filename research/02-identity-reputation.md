# 02 — Agent Identity & Reputation

**Scope**: Solana equivalents of `IdentityRegistry.sol` (ERC-8004) and `ReputationRegistry.sol` (append-only score history).

**Method**: Research agent invoked Helius skills (`svm`, `build`) and Exa search for ERC-8004, Solana Agent Registry, SAS, and reputation patterns.

---

**Bottom line**: As of April 2026, this is a *solved problem* on Solana — don't build a new program. The Solana Foundation's **Agent Registry** (live at `solana.com/agent-registry`) is an explicit ERC-8004 port with a published Solana profile, and the **`8004-solana`** reference implementation already exists with mainnet program IDs. SAS handles attestations as a second layer. Build CredMesh on top.

## Recommended identity primitive

**Use the Agent Registry's Metaplex Core NFT model.**

- Each agent = a Metaplex Core asset (single account, no separate metadata account; mintable in one CPI). The asset pubkey *is* the agent ID — analogous to ERC-721 `tokenId`, but globally unique without a registry-scoped counter.
- A `["agent_metadata", asset.key()]` PDA stores the off-chain `agentURI`, content hash, and an optional `agent_wallet` link (signature-verified update path mirrors ERC-8004's reserved field).
- Cost is ~0.009 SOL (~$0.81 at current prices) per registration — within budget for permissionless onboarding.

**Why not the alternatives:**
- **Pure custom PDA** (`["agent", owner_pubkey]`) is cheaper (~0.002 SOL) and is what SAID Protocol uses, but you give up NFT-ecosystem composability (wallets render it, marketplaces index it, ownership transfer is built-in via Core's transfer hook). For a credit protocol where lenders want to "see" the agent in a wallet UI, NFT identity wins.
- **Token-2022 NFT with `metadata` + `metadata-pointer` extensions** is the SPL-native NFT pattern, but Metaplex Core is cheaper (one account vs. mint+metadata+ATA), uses transfer hooks for revocation, and is what the published Solana spec aligns with.
- **Compressed NFTs (cNFT)** are too cheap to be useful here — identity needs to be referenced by other on-chain programs (escrow, oracle), and cNFTs require Merkle proofs at every read, forcing a Bubblegum CPI for any account-level check. Wrong primitive for *identity*; right primitive for *reputation events* (see below).
- **SAS attestations alone** are the wrong layer — they're attestations *about* a holder, not the holder identifier itself. SAS is complementary (KYC/accreditation badges *on top of* the agent NFT).
- **SNS domains** are human-readable aliases, not registries; they belong in the agent card, not as identity primitive.

## Recommended reputation primitive

**Hybrid: per-feedback Solana program events + a small rolling-digest PDA per agent**, as `8004-solana` already implements.

Rationale:
1. Solana account rent makes per-feedback PDAs (~0.002 SOL each) prohibitive at scale — a busy agent with 10k jobs would burn $90 on storage rent alone.
2. Solana program-emitted events are persisted in confirmed blocks and indexable forever via Helius webhooks/Laserstream/`getSignaturesForAsset`. Cost is just transaction fee (~$0.001).
3. The integrity gap (events are off the account-state Merkle tree) is closed by storing a **rolling keccak256 digest** (`feedback_digest: [u8; 32]`) and a counter (`feedback_count: u64`) on the per-agent reputation PDA. Each new feedback updates the digest = `keccak(prev_digest || new_event_hash)` (this is `8004-solana`'s SEAL v1 pattern). Anyone reconstructing the log off-chain can verify the digest matches.
4. For reputation *aggregates* the credit oracle actually reads (e.g., recent EMA score, default count), the per-agent PDA is sized for fixed fields and updated via CPI from the reputation program — no realloc treadmill. `8004-solana`'s ATOM Engine already does this with EMA arithmetic and tier vesting.

**Reject:**
- **Realloc growing PDA** — 10 KiB max per-instruction growth, 10 MiB per-account cap, and you pay rent linearly with size. Unbounded history is impossible.
- **Per-feedback cNFT** — works for "give the user a badge" UX but query latency via DAS for "give me this agent's last 50 scores" is acceptable, not great. Events + indexer is faster to read and 10x cheaper to write.
- **Off-chain log + Merkle root** — fine for cold storage, but losing on-chain queryability breaks composability with the lending program.

## Pseudo-IDL

**`agent_registry_8004` program**
```
state: AgentMetadata { agent_uri: String<200>, agent_uri_hash: [u8;32],
                       agent_wallet: Option<Pubkey>, owner: Pubkey, bump: u8 }
  PDA: ["agent_metadata", asset_pubkey]

ix register(agent_uri, agent_uri_hash) -> mints Core NFT to signer + inits PDA
ix set_agent_uri(new_uri, new_hash)     -> only NFT owner
ix set_agent_wallet(new_wallet, sig)    -> wallet-key signature required
ix transfer_agent(new_owner)            -> Core transfer CPI
```

**`reputation_registry` program**
```
state: AgentReputation { asset: Pubkey, feedback_count: u64,
                        feedback_digest: [u8;32], score_ema: u64,
                        score_decimals: u8, default_count: u32, bump: u8 }
  PDA: ["agent_reputation", asset_pubkey]

ix init_reputation(asset)
ix give_feedback(asset, score: u8, value: u64, value_decimals: u8,
                 reason_code: u16, feedback_uri: String,
                 feedback_file_hash: [u8;32], job_id: [u8;32])
   -> emits NewFeedback event, updates digest + EMA on PDA
ix append_response(asset, feedback_index, response_uri, response_hash)
ix revoke_feedback(asset, feedback_index, seal_hash) -> only original feedback signer
```

(Mirror `8004-solana`'s shape so you inherit the existing TS SDK, indexers like `8004scan`, and the SATI dashboard for free.)

## Read patterns

**Lending program (CredMesh trustless escrow on Solana):**

Pass accounts in `remaining_accounts`, do **not** CPI. Reasons: CPI depth is 4 max, and a single read shouldn't burn one of those slots; PDA derivation is deterministic so the escrow can validate it cheaply.

```
escrow.issue_advance(amount, agent_asset) accounts:
  - agent_core_asset                 (agent NFT, owned by core program)
  - agent_metadata_pda               (derived from agent_asset)
  - agent_reputation_pda             (derived from agent_asset)
  - ... vault/token accounts
```

Inside the handler:
1. Re-derive `["agent_metadata", agent_asset.key()]` and `["agent_reputation", agent_asset.key()]` and assert addresses match — proves the caller passed authentic registry accounts.
2. Deserialize `AgentReputation`, read `score_ema`, `default_count`, `feedback_count`. Apply your credit policy.
3. Optional: re-derive `["atom_stats", agent_asset.key()]` if you want Sybil-resistant tier (ATOM Engine HyperLogLog).

This mirrors the V3.1 Reputation oracle pattern from EVM CredMesh — same shape, no CPI tax.

## Write authorization

EVM hardcoded the worker as the sole reputation writer. On Solana, **prefer permissionless writes with stake-weighted/identity-weighted reads** (the ERC-8004 model: anyone calls `give_feedback`, the consumer decides whose feedback counts).

Concretely for CredMesh:
- `give_feedback` is permissionless on the program. Any signer can attest to any agent — this matches ERC-8004 semantics and the Agent Registry's `Leave Feedback` (~0.00001 SOL).
- The credit oracle program filters: it trusts feedback only from `client` Pubkeys it knows are actual job clients (job-completion attestations recorded by the marketplace program). The "writer is the protocol oracle" property becomes a *read-side filter* keyed off the marketplace's job-completion event, not a write-side ACL.
- For higher trust, layer **SAS attestations**: the protocol issues an attestation under its credential schema certifying "this feedback was emitted post-settlement," which the oracle then privileges.

This is strictly more flexible than EVM's hardcoded writer and matches Solana ecosystem norms.

## Cross-chain identity story

The Solana Agent Registry is **explicitly designed as a cross-chain ERC-8004 implementation** (the spec uses the same `agentRegistry: {namespace}:{chainId}:{address}` format, with namespace `solana:` instead of `eip155:`).

For a CredMesh agent live on Base today:
- The agent's existing ERC-8004 registration file (the JSON the EVM `IdentityRegistry` URI points to) gets a `registrations` array that lists *both* the EVM and Solana entries. This is already in the ERC-8004 spec body.
- On Solana, store the EVM address as a key in the agent metadata PDA's `additional_metadata` (or in the off-chain card under `crossChainIdentities[].address`). The `8004scan` explorer and SAID Protocol both already resolve cross-chain via this convention.
- For reputation, do **not** try to bridge scores. Each chain's reputation accrues independently; the credit oracle on each side reads its own chain's registry. Cross-chain reputation aggregation belongs in an off-chain scorer that pulls from both, signs an attestation, and posts it via SAS — *if* CredMesh ever wants it. Don't build it in v1.

A wallet-binding signature (sign EVM address with Solana key + reverse) prevents impersonation in the off-chain card.

## Open questions

1. **Single feedback program or one per protocol?** The Agent Registry / `8004-solana` pattern is one shared registry with permissionless writes; CredMesh's credit oracle just filters. Confirm this is acceptable to your audit posture vs. running CredMesh's own dedicated reputation program (cheaper writes but loses ecosystem network effects).
2. **Off-chain card hosting** — HTTPS (cheap, mutable, what the EVM side does) vs. Arweave (permanent, slow to update, ~$0.30 per upload) vs. IPFS (free pinning is unreliable). Recommendation: **HTTPS + content hash on-chain** (matches current EVM design) for the operator-mode agent card; **Arweave** only if you want trustless-mode agents to be decommission-resistant. Hybrid is fine — the URI scheme tells consumers which to expect.
3. **SEAL v1 hash chain semantics**: `8004-solana`'s rolling keccak digest is a one-way commitment but doesn't bind the *order* you discover events from indexers. Make sure CredMesh's oracle uses on-chain `feedback_count` + `feedback_digest` to detect indexer-supplied gaps.
4. **Token-2022 vs Metaplex Core long-term**: Core is younger but cheaper and is what the official Solana Agent Registry endorses. If you're risk-averse, Token-2022 NFT with metadata extension is the more conservative pick at ~2x the cost.
5. **Worker key on Solana**: the protocol still needs a hot key to settle and post reputation. Use a multisig (Squads V4) for the *update authority* on registry entries owned by CredMesh, but the per-feedback signer can be a fee-payer-only key with rotation.

## Key references

1. https://solana.com/agent-registry/what-is-agent-registry — Solana Foundation's official Agent Registry overview (ERC-8004 port).
2. https://eips.ethereum.org/EIPS/eip-8004 — ERC-8004 spec (cross-chain `registrations` array, `agentRegistry` namespacing).
3. https://github.com/QuantuLabs/8004-solana — `8004-solana` reference implementation (Identity + Reputation modules, SEAL v1 hash-chain, ATOM engine, Metaplex Core asset model).
4. https://quantulabs.github.io/8004-solana/ — Technical docs for the above (PDA seeds, instruction signatures, event-only feedback rationale).
5. https://attest.solana.com/ and https://github.com/solana-foundation/solana-attestation-service — SAS for layered attestations (Credential → Schema → Attestation PDAs).
6. https://saidprotocol.com/ + https://github.com/elizaos/eliza/pull/6510 — SAID Protocol; community alternative, pure-PDA non-NFT identity, useful as a fallback design point.
7. https://solana.com/docs/tokens/extensions/metadata — Token-2022 metadata + metadata-pointer extensions (alternative identity primitive).
8. https://www.helius.dev/docs/nfts/cnft-event-listening — DAS-based event indexing patterns; relevant if you ever do cNFT-per-event reputation.

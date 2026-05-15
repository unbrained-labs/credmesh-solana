/**
 * CredMesh-Solana liquidation keeper.
 *
 * Scans `Advance` PDAs owned by credmesh-escrow, finds the ones whose
 * `expires_at + LIQUIDATION_GRACE_SECONDS` has passed and `state ==
 * Issued`, and submits permissionless `liquidate()` ixs.
 *
 * Implementation is intentionally hand-rolled (no IDL) until issue #15
 * unblocks the Codama-generated client. The keeper does not need typed
 * accounts to do its job — Advance discriminator + a few field offsets
 * is enough.
 */

import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";
import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createTransactionMessage,
  getProgramDerivedAddress,
  getAddressEncoder,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  getSignatureFromTransaction,
  sendAndConfirmTransactionFactory,
  type KeyPairSigner,
  type Address,
} from "@solana/kit";

// Anchor account discriminator helpers — one source of truth in
// `../shared/src/index.ts`. We inline the helpers locally to keep the
// keeper a self-contained npm package (no workspace setup required).
async function anchorAccountDiscriminator(name: string): Promise<Uint8Array> {
  const buf = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`account:${name}`),
  );
  return new Uint8Array(buf).slice(0, 8);
}
async function anchorIxDiscriminator(name: string): Promise<Uint8Array> {
  const buf = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`global:${name}`),
  );
  return new Uint8Array(buf).slice(0, 8);
}

// ── Config ──────────────────────────────────────────────────────────────────

const RPC_URL = requireEnv("RPC_URL");
const KEEPER_KEYPAIR_PATH = requireEnv("KEEPER_KEYPAIR_PATH");
const ESCROW_PROGRAM_ID_STR = requireEnv("ESCROW_PROGRAM_ID");
const POOL_ASSET_MINT_STR = requireEnv("POOL_ASSET_MINT");
const SCAN_INTERVAL_SECONDS = Number(process.env.SCAN_INTERVAL_SECONDS ?? 300);
const LIQUIDATION_GRACE_SECONDS = BigInt(
  process.env.LIQUIDATION_GRACE_SECONDS ?? 14 * 24 * 60 * 60,
);

const ESCROW_PROGRAM_ID = address(ESCROW_PROGRAM_ID_STR);
const POOL_ASSET_MINT = address(POOL_ASSET_MINT_STR);
const POOL_SEED = new TextEncoder().encode("pool");

// AdvanceState enum: 0=Issued, 1=Settled, 2=Liquidated. We only crank
// `state == Issued` (settled and liquidated are terminal).
const ADVANCE_STATE_ISSUED = 0;

// ── Advance account decoding ────────────────────────────────────────────────

interface DecodedAdvance {
  pubkey: Address;
  agent: Uint8Array; // 32 bytes
  receivableId: Uint8Array; // 32 bytes
  expiresAt: bigint;
  state: number;
  bump: number;
}

/**
 * Decode the fields we care about from an Advance account's data buffer.
 *
 * MIRROR programs/credmesh-escrow/src/state.rs::Advance — field order
 * is load-bearing. Drift here = silent miscount (worst case: keeper
 * liquidates the wrong advance, or skips one that should liquidate).
 * Replace with a Codama-generated client once issue #15 unblocks IDL
 * extraction; until then, keep this comment + the field offsets in
 * sync with the Rust struct by hand.
 *
 *   discriminator (8) + bump (1) + agent (32) + receivable_id (32)
 *   + principal (8) + fee_owed (8) + late_penalty_per_day (8)
 *   + issued_at (8) + expires_at (8) + attestor (32) + state (1)
 *   = 152 bytes total (fixed-size; no Option after the EVM-bridge pivot)
 */
const ADVANCE_BYTES = 8 + 1 + 32 + 32 + 8 * 5 + 32 + 1;

function decodeAdvance(pubkey: Address, data: Uint8Array): DecodedAdvance | null {
  if (data.length < ADVANCE_BYTES) return null;
  const buf = Buffer.from(data);
  let off = 8;
  const bump = buf.readUInt8(off); off += 1;
  const agent = data.slice(off, off + 32); off += 32;
  const receivableId = data.slice(off, off + 32); off += 32;
  off += 8; // principal
  off += 8; // fee_owed
  off += 8; // late_penalty_per_day
  off += 8; // issued_at
  const expiresAt = buf.readBigInt64LE(off); off += 8;
  off += 32; // attestor
  const state = buf.readUInt8(off);

  return { pubkey, agent, receivableId, expiresAt, state, bump };
}

// ── Liquidate ix builder ────────────────────────────────────────────────────

interface LiquidateAccounts {
  cranker: Address;
  advance: Address;
  consumed: Address;
  issuanceLedger: Address;
  tombstone: Address;
  pool: Address;
  systemProgram: Address;
}

async function buildLiquidateIx(
  programId: Address,
  accounts: LiquidateAccounts,
  liquidateDiscriminator: Uint8Array,
) {
  // Anchor liquidate ix has no args beyond the discriminator.
  // Account order MUST match the Liquidate<'info> struct in
  // programs/credmesh-escrow/src/instructions/liquidate.rs.
  return {
    programAddress: programId,
    accounts: [
      { address: accounts.cranker, role: AccountRole.WRITABLE_SIGNER },
      { address: accounts.advance, role: AccountRole.WRITABLE },
      { address: accounts.consumed, role: AccountRole.READONLY },
      { address: accounts.issuanceLedger, role: AccountRole.WRITABLE },
      { address: accounts.tombstone, role: AccountRole.WRITABLE },
      { address: accounts.pool, role: AccountRole.WRITABLE },
      { address: accounts.systemProgram, role: AccountRole.READONLY },
    ],
    data: liquidateDiscriminator,
  };
}

async function deriveAdvancePda(
  pool: Address,
  agent: Uint8Array,
  receivableId: Uint8Array,
): Promise<Address> {
  const ADVANCE_SEED = new TextEncoder().encode("advance");
  const [pda] = await getProgramDerivedAddress({
    programAddress: ESCROW_PROGRAM_ID,
    seeds: [
      ADVANCE_SEED,
      getAddressEncoder().encode(pool),
      agent,
      receivableId,
    ],
  });
  return pda;
}

async function deriveConsumedPda(
  pool: Address,
  agent: Uint8Array,
  receivableId: Uint8Array,
): Promise<Address> {
  const CONSUMED_SEED = new TextEncoder().encode("consumed");
  const [pda] = await getProgramDerivedAddress({
    programAddress: ESCROW_PROGRAM_ID,
    seeds: [
      CONSUMED_SEED,
      getAddressEncoder().encode(pool),
      agent,
      receivableId,
    ],
  });
  return pda;
}

async function deriveTombstonePda(
  pool: Address,
  agent: Uint8Array,
  receivableId: Uint8Array,
): Promise<Address> {
  const LIQUIDATION_TOMBSTONE_SEED = new TextEncoder().encode("liq_tombstone");
  const [pda] = await getProgramDerivedAddress({
    programAddress: ESCROW_PROGRAM_ID,
    seeds: [
      LIQUIDATION_TOMBSTONE_SEED,
      getAddressEncoder().encode(pool),
      agent,
      receivableId,
    ],
  });
  return pda;
}

async function deriveIssuanceLedgerPda(
  pool: Address,
  agent: Uint8Array,
): Promise<Address> {
  const ISSUANCE_LEDGER_SEED = new TextEncoder().encode("issuance_ledger");
  const [pda] = await getProgramDerivedAddress({
    programAddress: ESCROW_PROGRAM_ID,
    seeds: [
      ISSUANCE_LEDGER_SEED,
      getAddressEncoder().encode(pool),
      agent,
    ],
  });
  return pda;
}

async function derivePoolPda(): Promise<Address> {
  const [pda] = await getProgramDerivedAddress({
    programAddress: ESCROW_PROGRAM_ID,
    seeds: [POOL_SEED, getAddressEncoder().encode(POOL_ASSET_MINT)],
  });
  return pda;
}

// ── Keeper loop ─────────────────────────────────────────────────────────────

async function loadKeeperSigner(): Promise<KeyPairSigner> {
  const path = KEEPER_KEYPAIR_PATH.startsWith("~")
    ? resolve(homedir(), KEEPER_KEYPAIR_PATH.slice(2))
    : KEEPER_KEYPAIR_PATH;
  const raw = readFileSync(path, "utf-8");
  const bytes = Uint8Array.from(JSON.parse(raw));
  return await createKeyPairSignerFromBytes(bytes);
}

async function tick(deps: {
  rpc: ReturnType<typeof createSolanaRpc>;
  pool: Address;
  signer: KeyPairSigner;
  advanceDiscriminator: Uint8Array;
  liquidateDiscriminator: Uint8Array;
  sendAndConfirm: ReturnType<typeof sendAndConfirmTransactionFactory>;
}): Promise<void> {
  const { rpc, pool, signer, advanceDiscriminator, liquidateDiscriminator, sendAndConfirm } = deps;

  // Filter program accounts whose first 8 bytes match the Advance
  // discriminator. The struct is fixed-size post-pivot so a state-byte
  // memcmp filter would also work, but this keeps the keeper insulated
  // from layout shifts; in-process filter is cheap at expected scale.
  const result = await rpc
    .getProgramAccounts(ESCROW_PROGRAM_ID, {
      encoding: "base64",
      filters: [
        {
          memcmp: {
            offset: 0n,
            bytes: Buffer.from(advanceDiscriminator).toString("base64") as any,
            encoding: "base64" as any,
          },
        },
      ],
    })
    .send();

  const nowSec = BigInt(Math.floor(Date.now() / 1000));
  const liquidatable: DecodedAdvance[] = [];

  for (const acc of result) {
    const data = Buffer.from(acc.account.data[0], "base64");
    const decoded = decodeAdvance(acc.pubkey, data);
    if (!decoded) continue;
    if (decoded.state !== ADVANCE_STATE_ISSUED) continue;
    const liquidationStart = decoded.expiresAt + LIQUIDATION_GRACE_SECONDS;
    if (nowSec >= liquidationStart) {
      liquidatable.push(decoded);
    }
  }

  if (liquidatable.length === 0) {
    console.log(`[${new Date().toISOString()}] no liquidatable advances`);
    return;
  }

  console.log(
    `[${new Date().toISOString()}] found ${liquidatable.length} liquidatable advance(s)`,
  );

  // Fetch blockhash ONCE per tick — valid for ~150 slots (≈60s), more than
  // enough time to fan out across the batch. Saves N-1 RPC round-trips on
  // multi-liquidation ticks.
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  // Liquidate concurrently. RPCs tolerate 25-50 in-flight tx submissions;
  // we cap at the batch size since liquidation events are rare per-tick.
  const results = await Promise.allSettled(
    liquidatable.map(async (adv) => {
      const [consumedPda, issuanceLedgerPda, tombstonePda] = await Promise.all([
        deriveConsumedPda(pool, adv.agent, adv.receivableId),
        deriveIssuanceLedgerPda(pool, adv.agent),
        deriveTombstonePda(pool, adv.agent, adv.receivableId),
      ]);
      const ix = await buildLiquidateIx(
        ESCROW_PROGRAM_ID,
        {
          cranker: signer.address,
          advance: adv.pubkey,
          consumed: consumedPda,
          issuanceLedger: issuanceLedgerPda,
          tombstone: tombstonePda,
          pool,
          systemProgram: address("11111111111111111111111111111111"),
        },
        liquidateDiscriminator,
      );
      const tx = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(signer, m),
        (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
        (m) => appendTransactionMessageInstruction(ix as any, m),
      );
      const signed = await signTransactionMessageWithSigners(tx);
      await sendAndConfirm(signed, { commitment: "confirmed" });
      return { advance: adv.pubkey, sig: getSignatureFromTransaction(signed) };
    }),
  );

  for (let i = 0; i < results.length; i++) {
    const r = results[i];
    const adv = liquidatable[i];
    if (r.status === "fulfilled") {
      console.log(`  ↳ liquidated ${r.value.advance} (sig ${r.value.sig})`);
    } else {
      console.error(`  ↳ liquidate FAILED for ${adv.pubkey}:`, r.reason);
    }
  }
}

async function main() {
  const rpc = createSolanaRpc(RPC_URL);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions: undefined as any });
  const signer = await loadKeeperSigner();
  const pool = await derivePoolPda();
  const advanceDiscriminator = await anchorAccountDiscriminator("Advance");
  const liquidateDiscriminator = await anchorIxDiscriminator("liquidate");

  console.log(`credmesh-solana-keeper v0.0.1`);
  console.log(`  RPC:            ${RPC_URL}`);
  console.log(`  Escrow program: ${ESCROW_PROGRAM_ID_STR}`);
  console.log(`  Pool PDA:       ${pool}`);
  console.log(`  Keeper:         ${signer.address}`);
  console.log(`  Scan interval:  ${SCAN_INTERVAL_SECONDS}s`);
  console.log(`  Grace period:   ${LIQUIDATION_GRACE_SECONDS}s`);

  // Loop forever (or until SIGTERM).
  while (true) {
    try {
      await tick({ rpc, pool, signer, advanceDiscriminator, liquidateDiscriminator, sendAndConfirm });
    } catch (err) {
      console.error("tick failed:", err);
    }
    await sleep(SCAN_INTERVAL_SECONDS * 1000);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((res) => setTimeout(res, ms));
}

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) {
    console.error(`missing required env var: ${name}`);
    process.exit(1);
  }
  return v;
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

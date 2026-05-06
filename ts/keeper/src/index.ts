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

// Anchor account discriminator for `Advance` = first 8 bytes of
// sha256("account:Advance"). Computed at startup so we can filter
// getProgramAccounts efficiently.
async function computeAdvanceDiscriminator(): Promise<Uint8Array> {
  const data = new TextEncoder().encode("account:Advance");
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf).slice(0, 8);
}

// AdvanceState enum: 0=Issued, 1=Settled, 2=Liquidated. We only crank
// `state == Issued` (settled and liquidated are terminal).
const ADVANCE_STATE_ISSUED = 0;

// `instruction:liquidate` discriminator = first 8 bytes of
// sha256("global:liquidate"). The Anchor 0.30 dispatch namespace is
// "global" for #[program] ixs.
async function computeLiquidateIxDiscriminator(): Promise<Uint8Array> {
  const data = new TextEncoder().encode("global:liquidate");
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf).slice(0, 8);
}

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
 * Field order matches programs/credmesh-escrow/src/state.rs Advance:
 *   discriminator (8) + bump (1) + agent (32) + receivable_id (32)
 *   + principal (8) + fee_owed (8) + late_penalty_per_day (8)
 *   + issued_at (8) + expires_at (8) + source_kind (1)
 *   + source_signer Option<Pubkey> (1 + 32 if Some) + state (1)
 */
function decodeAdvance(pubkey: Address, data: Uint8Array): DecodedAdvance | null {
  if (data.length < 8 + 1 + 32 + 32 + 8 * 5 + 1 + 1 + 1) return null;
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
  off += 1; // source_kind
  const sourceSignerTag = buf.readUInt8(off); off += 1;
  if (sourceSignerTag === 1) off += 32; // source_signer present
  const state = buf.readUInt8(off);

  return { pubkey, agent, receivableId, expiresAt, state, bump };
}

// ── Liquidate ix builder ────────────────────────────────────────────────────

interface LiquidateAccounts {
  cranker: Address;
  advance: Address;
  consumed: Address;
  pool: Address;
}

async function buildLiquidateIx(
  programId: Address,
  accounts: LiquidateAccounts,
  liquidateDiscriminator: Uint8Array,
) {
  // Anchor liquidate ix has no args beyond the discriminator.
  return {
    programAddress: programId,
    accounts: [
      { address: accounts.cranker, role: 3 /* WritableSigner */ },
      { address: accounts.advance, role: 1 /* Writable */ },
      { address: accounts.consumed, role: 0 /* Readonly */ },
      { address: accounts.pool, role: 1 /* Writable */ },
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
  // discriminator. We don't filter on `state` here because the offset
  // is variable (source_signer Option). Filter in-process.
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

  for (const adv of liquidatable) {
    try {
      const consumedPda = await deriveConsumedPda(pool, adv.agent, adv.receivableId);
      const ix = await buildLiquidateIx(
        ESCROW_PROGRAM_ID,
        { cranker: signer.address, advance: adv.pubkey, consumed: consumedPda, pool },
        liquidateDiscriminator,
      );

      const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
      const tx = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(signer, m),
        (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
        (m) => appendTransactionMessageInstruction(ix as any, m),
      );

      const signed = await signTransactionMessageWithSigners(tx);
      await sendAndConfirm(signed, { commitment: "confirmed" });
      const sig = getSignatureFromTransaction(signed);
      console.log(`  ↳ liquidated ${adv.pubkey} (sig ${sig})`);
    } catch (err) {
      console.error(`  ↳ liquidate FAILED for ${adv.pubkey}:`, err);
    }
  }
}

async function main() {
  const rpc = createSolanaRpc(RPC_URL);
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions: undefined as any });
  const signer = await loadKeeperSigner();
  const pool = await derivePoolPda();
  const advanceDiscriminator = await computeAdvanceDiscriminator();
  const liquidateDiscriminator = await computeLiquidateIxDiscriminator();

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

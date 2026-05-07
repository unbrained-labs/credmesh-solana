// scripts/init_pool.ts — calls `init_pool` on credmesh-escrow to allocate
// the singleton Pool PDA + share-mint + USDC vault for a given asset mint
// (USDC by default).
//
// This script uses a HAND-ROLLED instruction encoder because the escrow IDL
// is currently blocked behind issue #15 (Anchor 0.30 IDL extraction trips
// on `AssociatedToken` resolution). Once #15 lands and `target/idl/
// credmesh_escrow.json` exists, this script can be replaced with the
// Anchor-typed Codama-generated equivalent.
//
// Layout reference: programs/credmesh-escrow/src/instructions/init_pool.rs
// InitPoolParams + InitPool accounts struct. Borsh field order MUST match
// the Rust struct.
//
// Example:
//   npx ts-node scripts/init_pool.ts \
//     --cluster devnet \
//     --asset-mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
//     --governance <GOVERNANCE_PUBKEY> \
//     --treasury-ata <TREASURY_USDC_ATA> \
//     --max-advance-pct-bps 3000 \
//     --max-advance-abs 100000000 \
//     --timelock-seconds 86400 \
//     --chain-id 2 \
//     --agent-window-cap 500000000        # $500/24h per agent (0 = disabled)

import { createHash } from "node:crypto";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { ESCROW_PROGRAM_ID } from "./lib/program-ids";
import { buildProvider, parseArgs } from "./lib/cluster";

const POOL_SEED = Buffer.from("pool");

// Anchor 0.30 instruction discriminator: first 8 bytes of sha256("global:<method>").
function anchorDiscriminator(method: string): Buffer {
  return createHash("sha256")
    .update(`global:${method}`)
    .digest()
    .subarray(0, 8);
}

// Default fee curve for v1 devnet bring-up. Governance can mutate via
// `propose_params` + `execute_params` afterwards. All values are basis
// points (10_000 = 100%).
interface FeeCurve {
  utilizationKinkBps: number; // u16
  baseRateBps: number;
  kinkRateBps: number;
  maxRateBps: number;
  durationPerDayBps: number;
  riskPremiumBps: number;
  poolLossSurchargeBps: number;
}

const DEFAULT_FEE_CURVE: FeeCurve = {
  utilizationKinkBps: 8_000, // 80% utilization knee
  baseRateBps: 200, // 2% APY at 0% util
  kinkRateBps: 1_000, // 10% APY at the knee
  maxRateBps: 5_000, // 50% APY at 100%
  durationPerDayBps: 5, // 0.05% per day duration premium
  riskPremiumBps: 100, // 1% baseline risk premium
  poolLossSurchargeBps: 0, // no historical losses yet
};

// Borsh encoding of FeeCurve: 7 × u16 (LE) = 14 bytes.
function encodeFeeCurve(fc: FeeCurve): Buffer {
  const buf = Buffer.alloc(14);
  buf.writeUInt16LE(fc.utilizationKinkBps, 0);
  buf.writeUInt16LE(fc.baseRateBps, 2);
  buf.writeUInt16LE(fc.kinkRateBps, 4);
  buf.writeUInt16LE(fc.maxRateBps, 6);
  buf.writeUInt16LE(fc.durationPerDayBps, 8);
  buf.writeUInt16LE(fc.riskPremiumBps, 10);
  buf.writeUInt16LE(fc.poolLossSurchargeBps, 12);
  return buf;
}

interface InitPoolParams {
  feeCurve: FeeCurve;
  maxAdvancePctBps: number; // u16
  maxAdvanceAbs: bigint; // u64
  timelockSeconds: bigint; // i64
  governance: PublicKey;
  treasuryAta: PublicKey;
  chainId: bigint; // u64 — must equal CHAIN_ID_MAINNET (1) or CHAIN_ID_DEVNET (2)
  agentWindowCap: bigint; // u64 — 0 disables the on-chain per-agent cap
}

// Borsh encoding of InitPoolParams. Field order MUST match the Rust struct
// (programs/credmesh-escrow/src/instructions/init_pool.rs).
function encodeInitPoolParams(p: InitPoolParams): Buffer {
  const fc = encodeFeeCurve(p.feeCurve);
  const buf = Buffer.alloc(2 + 8 + 8 + 32 + 32 + 8 + 8);
  buf.writeUInt16LE(p.maxAdvancePctBps, 0);
  buf.writeBigUInt64LE(p.maxAdvanceAbs, 2);
  buf.writeBigInt64LE(p.timelockSeconds, 10);
  p.governance.toBuffer().copy(buf, 18);
  p.treasuryAta.toBuffer().copy(buf, 50);
  buf.writeBigUInt64LE(p.chainId, 82);
  buf.writeBigUInt64LE(p.agentWindowCap, 90);
  return Buffer.concat([fc, buf]);
}

async function main(): Promise<void> {
  const args = parseArgs(
    process.argv.slice(2),
    [
      "cluster",
      "asset-mint",
      "governance",
      "treasury-ata",
      "max-advance-pct-bps",
      "max-advance-abs",
      "timelock-seconds",
      "chain-id",
      "agent-window-cap",
    ] as const,
    ["wallet"],
  );

  const provider = buildProvider({
    cluster: args.cluster,
    walletPath: args.wallet,
  });
  const deployer = provider.wallet.publicKey;
  const assetMint = new PublicKey(args["asset-mint"]);
  const governance = new PublicKey(args.governance);
  const treasuryAta = new PublicKey(args["treasury-ata"]);

  if (governance.equals(deployer)) {
    throw new Error(
      "governance and deployer MUST differ — pre-create a Squads vault PDA and pass it as --governance.",
    );
  }

  const [poolPda, poolBump] = PublicKey.findProgramAddressSync(
    [POOL_SEED, assetMint.toBuffer()],
    ESCROW_PROGRAM_ID,
  );
  console.log(`pool PDA:   ${poolPda.toBase58()} (bump ${poolBump})`);

  const existing = await provider.connection.getAccountInfo(poolPda);
  if (existing !== null) {
    console.error(
      `pool already initialised at ${poolPda.toBase58()} for asset_mint ${assetMint.toBase58()}. Refusing to re-init.`,
    );
    process.exit(2);
  }

  // share_mint and usdc_vault are init'd inline by the program via
  // `payer = deployer` — they must be fresh keypairs we sign with.
  const shareMint = Keypair.generate();
  const usdcVault = Keypair.generate();
  console.log(`share_mint: ${shareMint.publicKey.toBase58()} (fresh)`);
  console.log(`usdc_vault: ${usdcVault.publicKey.toBase58()} (fresh)`);

  const chainId = BigInt(args["chain-id"]);
  if (chainId !== 1n && chainId !== 2n) {
    throw new Error(
      "--chain-id must equal 1 (mainnet) or 2 (devnet) — see crates/credmesh-shared::ed25519_credit_message::CHAIN_ID_*",
    );
  }
  const params: InitPoolParams = {
    feeCurve: DEFAULT_FEE_CURVE,
    maxAdvancePctBps: parseInt(args["max-advance-pct-bps"], 10),
    maxAdvanceAbs: BigInt(args["max-advance-abs"]),
    timelockSeconds: BigInt(args["timelock-seconds"]),
    governance,
    treasuryAta,
    chainId,
    agentWindowCap: BigInt(args["agent-window-cap"]),
  };
  if (params.maxAdvancePctBps > 10_000) {
    throw new Error("--max-advance-pct-bps cannot exceed 10000 (100%)");
  }

  const data = Buffer.concat([
    anchorDiscriminator("init_pool"),
    encodeInitPoolParams(params),
  ]);

  // Account order MUST match the Rust `InitPool` Accounts struct.
  const ix = new TransactionInstruction({
    programId: ESCROW_PROGRAM_ID,
    keys: [
      { pubkey: deployer, isSigner: true, isWritable: true },
      { pubkey: poolPda, isSigner: false, isWritable: true },
      { pubkey: assetMint, isSigner: false, isWritable: false },
      { pubkey: shareMint.publicKey, isSigner: true, isWritable: true },
      { pubkey: usdcVault.publicKey, isSigner: true, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(ix);
  const sig = await provider.sendAndConfirm(tx, [shareMint, usdcVault], {
    commitment: "confirmed",
  });
  console.log(`\ntx: ${sig}`);
  console.log("init_pool: ok");
}

main().catch((err) => {
  console.error("init_pool: failed");
  console.error(err);
  process.exit(1);
});

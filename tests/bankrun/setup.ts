/**
 * Shared Bankrun test setup. Loads the three credmesh programs into a fresh
 * BankrunProvider, deploys a test USDC mint, and exposes helpers for funding
 * agents + LPs.
 *
 * This file is the test harness. Individual test files import from it.
 *
 * Status: scaffolded; will activate once `anchor build` produces the program
 * artifacts at `target/deploy/*.so` and IDL at `target/idl/*.json`.
 */

import * as anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { startAnchor, BankrunProvider } from "anchor-bankrun";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotent,
  mintTo,
} from "@solana/spl-token";

// Devnet program IDs (Track A — PR #16). Mirror of `crates/credmesh-shared::program_ids`.
// Source: target/deploy/credmesh_*-keypair.json. Once Track A's PR merges and the
// IDL is generated, prefer importing the address from `target/idl/<name>.json`.
export const ESCROW_PROGRAM_ID = new PublicKey("DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF");
export const REPUTATION_PROGRAM_ID = new PublicKey("JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G");
export const ORACLE_PROGRAM_ID = new PublicKey("ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk");

export interface TestContext {
  provider: BankrunProvider;
  payer: Keypair;
  usdcMint: PublicKey;
  programs: {
    escrow: PublicKey;
    reputation: PublicKey;
    oracle: PublicKey;
  };
}

export async function setupBankrun(): Promise<TestContext> {
  const context = await startAnchor(
    ".",
    [
      { name: "credmesh_escrow", programId: ESCROW_PROGRAM_ID },
      { name: "credmesh_reputation", programId: REPUTATION_PROGRAM_ID },
      { name: "credmesh_receivable_oracle", programId: ORACLE_PROGRAM_ID },
    ],
    [],
  );
  const provider = new BankrunProvider(context);
  anchor.setProvider(provider);

  const payer = (provider.wallet as anchor.Wallet).payer;

  // Deploy a test USDC mint with 6 decimals owned by the payer.
  const usdcMint = await createMint(
    provider.connection as any,
    payer,
    payer.publicKey,
    null,
    6,
  );

  return {
    provider,
    payer,
    usdcMint,
    programs: {
      escrow: ESCROW_PROGRAM_ID,
      reputation: REPUTATION_PROGRAM_ID,
      oracle: ORACLE_PROGRAM_ID,
    },
  };
}

export async function fundUsdc(
  ctx: TestContext,
  recipient: PublicKey,
  amountAtoms: bigint,
): Promise<PublicKey> {
  const ata = await createAssociatedTokenAccountIdempotent(
    ctx.provider.connection as any,
    ctx.payer,
    ctx.usdcMint,
    recipient,
  );
  await mintTo(
    ctx.provider.connection as any,
    ctx.payer,
    ctx.usdcMint,
    ata,
    ctx.payer.publicKey,
    amountAtoms,
  );
  return ata;
}

export function poolPda(usdcMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), usdcMint.toBuffer()],
    ESCROW_PROGRAM_ID,
  );
}

export function advancePda(
  pool: PublicKey,
  agent: PublicKey,
  receivableId: Buffer,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("advance"), pool.toBuffer(), agent.toBuffer(), receivableId],
    ESCROW_PROGRAM_ID,
  );
}

export function consumedPda(pool: PublicKey, receivableId: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("consumed"), pool.toBuffer(), receivableId],
    ESCROW_PROGRAM_ID,
  );
}

export function reputationPda(agentAsset: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("agent_reputation"), agentAsset.toBuffer()],
    REPUTATION_PROGRAM_ID,
  );
}

export function oracleConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("oracle_config")],
    ORACLE_PROGRAM_ID,
  );
}

export function receivablePda(
  agent: PublicKey,
  sourceId: Buffer,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("receivable"), agent.toBuffer(), sourceId],
    ORACLE_PROGRAM_ID,
  );
}

export function allowedSignerPda(signer: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("allowed_signer"), signer.toBuffer()],
    ORACLE_PROGRAM_ID,
  );
}

// programs/credmesh-escrow/tests/helpers/setup.ts
//
// Bankrun program-test boot for credmesh-escrow + credmesh-attestor-registry.
//
// Loads both `.so` artifacts from `target/deploy/`, mints a fresh "USDC" mint
// inside the bankrun ledger (we control the universe so we don't need to
// clone mainnet USDC), funds a deployer + LP + agent wallet, inits the
// attestor-registry, inits a Pool with sane fee-curve defaults, and mints
// LP USDC. Returns a typed `TestCtx` for use by individual test files.
//
// No IDL is consumed — Anchor 0.30 IDL extraction is blocked behind issue
// #15, so this file (like `scripts/init_pool.ts`) hand-rolls Borsh encoders
// against the Rust structs. Field order MUST match the Rust source.

import { createHash } from "node:crypto";
import { resolve } from "node:path";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createInitializeMint2Instruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  MINT_SIZE,
} from "@solana/spl-token";
import { startAnchor, BankrunProvider } from "anchor-bankrun";
import { ProgramTestContext, BanksClient } from "solana-bankrun";

export const ESCROW_PROGRAM_ID = new PublicKey(
  "DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF",
);
export const ATTESTOR_REGISTRY_PROGRAM_ID = new PublicKey(
  "ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk",
);

const POOL_SEED = Buffer.from("pool");
const ATTESTOR_CONFIG_SEED = Buffer.from("attestor_config");
const ALLOWED_SIGNER_SEED = Buffer.from("allowed_signer");

export const CHAIN_ID_DEVNET = 2n;

/// Discriminator: first 8 bytes of sha256("global:<method>"). Matches the
/// existing scripts/ encoders.
function anchorDiscriminator(method: string): Buffer {
  return createHash("sha256")
    .update(`global:${method}`)
    .digest()
    .subarray(0, 8);
}

interface FeeCurve {
  utilizationKinkBps: number;
  baseRateBps: number;
  kinkRateBps: number;
  maxRateBps: number;
  durationPerDayBps: number;
  riskPremiumBps: number;
  poolLossSurchargeBps: number;
}

const DEFAULT_FEE_CURVE: FeeCurve = {
  utilizationKinkBps: 8_000,
  baseRateBps: 200,
  kinkRateBps: 1_000,
  maxRateBps: 5_000,
  durationPerDayBps: 5,
  riskPremiumBps: 100,
  poolLossSurchargeBps: 0,
};

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
  maxAdvancePctBps: number;
  maxAdvanceAbs: bigint;
  timelockSeconds: bigint;
  governance: PublicKey;
  treasuryAta: PublicKey;
  chainId: bigint;
  agentWindowCap: bigint;
}

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

export interface TestCtx {
  context: ProgramTestContext;
  banksClient: BanksClient;
  provider: BankrunProvider;
  // Wallets
  payer: Keypair; // bankrun-provided lamport sink; pays for everything by default
  deployer: Keypair;
  lp: Keypair;
  agentA: Keypair;
  agentB: Keypair;
  governance: PublicKey; // unused-but-stored Squads vault placeholder
  // Token universe
  usdcMint: PublicKey;
  shareMint: PublicKey;
  treasuryAta: PublicKey;
  lpUsdcAta: PublicKey;
  agentAUsdcAta: PublicKey;
  agentBUsdcAta: PublicKey;
  poolUsdcVault: PublicKey;
  // PDAs
  poolPda: PublicKey;
  attestorConfigPda: PublicKey;
  // Helpers
  derivedAllowedSignerPda: (signer: PublicKey) => PublicKey;
}

/// Compute the workspace root from the worktree-relative tests dir. Bankrun's
/// `startAnchor` takes a workspace path; the .so files must live under
/// `<workspace>/target/deploy/` named after the program crate (with hyphens
/// → underscores per cargo).
function workspaceRoot(): string {
  // __dirname at runtime is .../programs/credmesh-escrow/tests/helpers
  return resolve(__dirname, "..", "..", "..", "..");
}

/// Bootstrap the entire bankrun environment.
///
/// Returns a `TestCtx` with the pool initialized, vault holding LP USDC,
/// and the attestor-registry config PDA created. Individual tests then
/// derive AllowedSigner PDAs (either via the governance flow OR — for
/// adversarial replay tests — via bankrun's raw `setAccount` API; see
/// `prestampAllowedSigner` below).
export async function bootstrap(): Promise<TestCtx> {
  // `startAnchor` reads Anchor.toml for the workspace's declared programs
  // and loads the matching .so files from target/deploy/. We pass an empty
  // extra-programs array (Anchor.toml already declares both programs) and
  // no extra accounts.
  const context = await startAnchor(workspaceRoot(), [], []);
  const banksClient = context.banksClient;
  const provider = new BankrunProvider(context);
  const payer = context.payer;

  // Wallets
  const deployer = payer; // reuse the bankrun-funded payer as the init signer
  const lp = Keypair.generate();
  const agentA = Keypair.generate();
  const agentB = Keypair.generate();
  const governance = Keypair.generate().publicKey; // placeholder Squads vault

  await airdrop(context, lp.publicKey, 100n * 1_000_000_000n);
  await airdrop(context, agentA.publicKey, 100n * 1_000_000_000n);
  await airdrop(context, agentB.publicKey, 100n * 1_000_000_000n);

  // Mint a fresh "USDC" — we control the universe, so cloning mainnet USDC
  // isn't required for unit-level tests.
  const usdcMintKp = Keypair.generate();
  await createMint(context, deployer, usdcMintKp, 6);
  const usdcMint = usdcMintKp.publicKey;

  // Pre-create the treasury ATA owned by `governance` (a stand-in pubkey)
  // — `init_pool` just stores `treasury_ata` as a pubkey, doesn't validate
  // it, so any account suffices.
  const treasuryAta = getAssociatedTokenAddressSync(usdcMint, governance, true);
  await createAtaIfMissing(context, deployer, treasuryAta, governance, usdcMint);

  // Pool PDA + fresh share-mint + USDC vault keypairs (matching the
  // `InitPool` Accounts struct in init_pool.rs).
  const [poolPda] = PublicKey.findProgramAddressSync(
    [POOL_SEED, usdcMint.toBuffer()],
    ESCROW_PROGRAM_ID,
  );
  const shareMintKp = Keypair.generate();
  const usdcVaultKp = Keypair.generate();

  await initPool({
    banksClient,
    context,
    deployer,
    poolPda,
    usdcMint,
    shareMintKp,
    usdcVaultKp,
    governance,
    treasuryAta,
  });

  // Init the attestor-registry config (governance == placeholder vault).
  const [attestorConfigPda] = PublicKey.findProgramAddressSync(
    [ATTESTOR_CONFIG_SEED],
    ATTESTOR_REGISTRY_PROGRAM_ID,
  );
  await initRegistry({
    banksClient,
    context,
    deployer,
    attestorConfigPda,
    governance,
  });

  // Create LP + both agents' USDC ATAs and mint LP some USDC, then have LP
  // call `deposit` so the vault has liquidity for `request_advance` tests.
  const lpUsdcAta = getAssociatedTokenAddressSync(usdcMint, lp.publicKey);
  const agentAUsdcAta = getAssociatedTokenAddressSync(usdcMint, agentA.publicKey);
  const agentBUsdcAta = getAssociatedTokenAddressSync(usdcMint, agentB.publicKey);

  await createAtaIfMissing(context, deployer, lpUsdcAta, lp.publicKey, usdcMint);
  await createAtaIfMissing(context, deployer, agentAUsdcAta, agentA.publicKey, usdcMint);
  await createAtaIfMissing(context, deployer, agentBUsdcAta, agentB.publicKey, usdcMint);

  await mintTo(context, deployer, usdcMint, lpUsdcAta, 1_000_000_000n); // 1000 USDC

  return {
    context,
    banksClient,
    provider,
    payer,
    deployer,
    lp,
    agentA,
    agentB,
    governance,
    usdcMint,
    shareMint: shareMintKp.publicKey,
    treasuryAta,
    lpUsdcAta,
    agentAUsdcAta,
    agentBUsdcAta,
    poolUsdcVault: usdcVaultKp.publicKey,
    poolPda,
    attestorConfigPda,
    derivedAllowedSignerPda: (signer: PublicKey) =>
      PublicKey.findProgramAddressSync(
        [ALLOWED_SIGNER_SEED, signer.toBuffer()],
        ATTESTOR_REGISTRY_PROGRAM_ID,
      )[0],
  };
}

// ─────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────

async function airdrop(
  context: ProgramTestContext,
  to: PublicKey,
  lamports: bigint,
): Promise<void> {
  const account = await context.banksClient.getAccount(to);
  const current = account ? account.lamports : 0;
  context.setAccount(to, {
    lamports: Number(BigInt(current) + lamports),
    data: Buffer.alloc(0),
    owner: SystemProgram.programId,
    executable: false,
  });
}

async function createMint(
  context: ProgramTestContext,
  payer: Keypair,
  mintKp: Keypair,
  decimals: number,
): Promise<void> {
  const rent = await rentExempt(context, MINT_SIZE);
  const create = SystemProgram.createAccount({
    fromPubkey: payer.publicKey,
    newAccountPubkey: mintKp.publicKey,
    lamports: Number(rent),
    space: MINT_SIZE,
    programId: TOKEN_PROGRAM_ID,
  });
  const init = createInitializeMint2Instruction(
    mintKp.publicKey,
    decimals,
    payer.publicKey, // mint authority
    payer.publicKey, // freeze authority
  );
  await sendTx(context, payer, [create, init], [mintKp]);
}

async function createAtaIfMissing(
  context: ProgramTestContext,
  payer: Keypair,
  ata: PublicKey,
  owner: PublicKey,
  mint: PublicKey,
): Promise<void> {
  const existing = await context.banksClient.getAccount(ata);
  if (existing) return;
  const ix = createAssociatedTokenAccountInstruction(
    payer.publicKey,
    ata,
    owner,
    mint,
  );
  await sendTx(context, payer, [ix], []);
}

async function mintTo(
  context: ProgramTestContext,
  authority: Keypair,
  mint: PublicKey,
  to: PublicKey,
  amount: bigint,
): Promise<void> {
  const ix = createMintToInstruction(mint, to, authority.publicKey, amount);
  await sendTx(context, authority, [ix], []);
}

async function rentExempt(context: ProgramTestContext, space: number): Promise<bigint> {
  const rent = await context.banksClient.getRent();
  return rent.minimumBalance(BigInt(space));
}

async function sendTx(
  context: ProgramTestContext,
  payer: Keypair,
  ixs: TransactionInstruction[],
  extraSigners: Keypair[],
): Promise<void> {
  const tx = new Transaction();
  tx.add(...ixs);
  tx.recentBlockhash = context.lastBlockhash;
  tx.feePayer = payer.publicKey;
  tx.sign(payer, ...extraSigners);
  await context.banksClient.processTransaction(tx);
}

/**
 * Compute the two trailing accounts that Anchor 0.30's `emit_cpi!`
 * needs on every ix that calls `emit!`. The workspace has
 * `event-cpi` enabled (Cargo.toml line 19) which expands `emit!` into
 * a self-CPI; without these accounts the CPI access-violates reading
 * past the ix's account list.
 *   - event_authority: PDA at seeds=[b"__event_authority"], program=programId
 *   - program: the program itself
 * Order matters; both are read-only, non-signer.
 */
export function eventCpiAccounts(programId: PublicKey): {
  eventAuthority: PublicKey;
  programKeys: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[];
} {
  const [eventAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("__event_authority")],
    programId,
  );
  return {
    eventAuthority,
    programKeys: [
      { pubkey: eventAuthority, isSigner: false, isWritable: false },
      { pubkey: programId, isSigner: false, isWritable: false },
    ],
  };
}

async function initPool(opts: {
  banksClient: BanksClient;
  context: ProgramTestContext;
  deployer: Keypair;
  poolPda: PublicKey;
  usdcMint: PublicKey;
  shareMintKp: Keypair;
  usdcVaultKp: Keypair;
  governance: PublicKey;
  treasuryAta: PublicKey;
}): Promise<void> {
  const params: InitPoolParams = {
    feeCurve: DEFAULT_FEE_CURVE,
    maxAdvancePctBps: 3_000,
    maxAdvanceAbs: 100_000_000n, // 100 USDC
    timelockSeconds: 86_400n,
    governance: opts.governance,
    treasuryAta: opts.treasuryAta,
    chainId: CHAIN_ID_DEVNET,
    agentWindowCap: 0n, // disabled for the smoke test pool
  };
  const data = Buffer.concat([
    anchorDiscriminator("init_pool"),
    encodeInitPoolParams(params),
  ]);
  const ix = new TransactionInstruction({
    programId: ESCROW_PROGRAM_ID,
    keys: [
      { pubkey: opts.deployer.publicKey, isSigner: true, isWritable: true },
      { pubkey: opts.poolPda, isSigner: false, isWritable: true },
      { pubkey: opts.usdcMint, isSigner: false, isWritable: false },
      { pubkey: opts.shareMintKp.publicKey, isSigner: true, isWritable: true },
      { pubkey: opts.usdcVaultKp.publicKey, isSigner: true, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      ...eventCpiAccounts(ESCROW_PROGRAM_ID).programKeys,
    ],
    data,
  });
  await sendTx(opts.context, opts.deployer, [ix], [
    opts.shareMintKp,
    opts.usdcVaultKp,
  ]);
}

async function initRegistry(opts: {
  banksClient: BanksClient;
  context: ProgramTestContext;
  deployer: Keypair;
  attestorConfigPda: PublicKey;
  governance: PublicKey;
}): Promise<void> {
  const data = Buffer.concat([
    anchorDiscriminator("init_registry"),
    opts.governance.toBuffer(),
  ]);
  const ix = new TransactionInstruction({
    programId: ATTESTOR_REGISTRY_PROGRAM_ID,
    keys: [
      { pubkey: opts.deployer.publicKey, isSigner: true, isWritable: true },
      { pubkey: opts.attestorConfigPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ...eventCpiAccounts(ATTESTOR_REGISTRY_PROGRAM_ID).programKeys,
    ],
    data,
  });
  await sendTx(opts.context, opts.deployer, [ix], []);
}

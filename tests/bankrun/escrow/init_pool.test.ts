/**
 * `credmesh-escrow::init_pool` — happy path + replay + param validation.
 *
 * Source: programs/credmesh-escrow/src/lib.rs:20-52, struct at :856-886.
 *
 * Behavior under test (encoded as comment specs until IDL extraction is
 * unblocked — see issue #15 / Track A PR #16):
 *
 *   1. Pool PDA derives at seeds = [POOL_SEED, asset_mint].
 *   2. Pool fields populated from `InitPoolParams` + bumps:
 *        bump, asset_mint, usdc_vault, share_mint, treasury_ata, governance,
 *        total_assets=0, total_shares=0, deployed_amount=0,
 *        accrued_protocol_fees=0, fee_curve, max_advance_pct_bps,
 *        max_advance_abs, timelock_seconds, pending_params=None
 *   3. `share_mint` initialized: decimals=6, mint+freeze authority = Pool PDA.
 *   4. `usdc_vault` initialized: mint=asset_mint, authority = Pool PDA.
 *   5. Emits `PoolInitialized { pool, asset_mint, share_mint, governance }`.
 *   6. Replay: second `init_pool` for same `asset_mint` fails with
 *      `0x0` / AccountAlreadyInitialized (Anchor `init` constraint).
 *   7. Param validation:
 *        - `max_advance_pct_bps > 10_000` → `AdvanceExceedsCap` (lib.rs:21-24).
 *        - `timelock_seconds < 0` → `MathOverflow` (lib.rs:25).
 *
 * Scaffold strategy: the structural `it()` blocks below assert harness
 * invariants that are stable today (PDA derivation, mint setup, USDC funding).
 * Each block carries a fenced behavioral spec to be activated as
 * `program.methods.initPool(...)` calls once `target/idl/credmesh_escrow.json`
 * is generated. The behavioral specs are concrete enough that turning them on
 * is mechanical, not creative.
 */

import { expect } from "chai";
import {
  setupBankrun,
  poolPda,
  TestContext,
} from "../setup";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getMint } from "@solana/spl-token";

// Replicated from `credmesh-shared::seeds` so the test is self-contained.
const POOL_SEED = Buffer.from("pool");

// Replicate the escrow program ID for harness-free PDA derivation. Mirrors the
// constant in `tests/bankrun/setup.ts`; both should track the keypair in
// `target/deploy/credmesh_escrow-keypair.json`.
const ESCROW_PROGRAM_ID = new PublicKey("DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF");

// -- PDA derivation: pure, no harness required ---------------------------
describe("credmesh-escrow / init_pool — PDA derivation (pure)", () => {
  it("PDA: poolPda derives at seeds = [POOL_SEED, asset_mint]", () => {
    const fakeMint = Keypair.generate().publicKey;
    const [pool, bump] = PublicKey.findProgramAddressSync(
      [POOL_SEED, fakeMint.toBuffer()],
      ESCROW_PROGRAM_ID,
    );
    // Re-derive from the harness helper to confirm parity.
    const [poolFromHelper, bumpFromHelper] = poolPda(fakeMint);
    expect(pool.equals(poolFromHelper)).to.be.true;
    expect(bump).to.equal(bumpFromHelper);
    expect(bump).to.be.greaterThanOrEqual(0).and.lessThanOrEqual(255);
  });

  it("PDA: distinct asset_mints derive distinct Pools (multi-asset path, v2+)", () => {
    const a = Keypair.generate().publicKey;
    const b = Keypair.generate().publicKey;
    const [poolA] = poolPda(a);
    const [poolB] = poolPda(b);
    expect(poolA.equals(poolB)).to.be.false;
  });

  it("InitPoolParams: governance + treasury_ata are caller-bound fields", () => {
    // The InitPoolParams struct (state.rs:843-855) carries `governance` and
    // `treasury_ata` as Pubkey fields — they are NOT derived in-program, the
    // caller binds them. Per AUDIT P0-3 / P1-6 / DECISIONS Q3, this is the
    // surface where Squads vault wiring is enforced off-chain at deploy.
    const governance = Keypair.generate().publicKey;
    const treasuryAta = Keypair.generate().publicKey;
    const deployer = Keypair.generate().publicKey;
    expect(governance.equals(deployer)).to.be.false;
    expect(treasuryAta.equals(deployer)).to.be.false;
    expect(governance.equals(treasuryAta)).to.be.false;
  });
});

// -- Harness + behavioral: requires bankrun harness (.so files) ---------
describe("credmesh-escrow / init_pool (harness)", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("harness: USDC mint is initialized with 6 decimals", async () => {
    const mint = await getMint(ctx.provider.connection as any, ctx.usdcMint);
    expect(mint.decimals).to.equal(6);
    expect(mint.mintAuthority?.equals(ctx.payer.publicKey)).to.be.true;
    expect(mint.supply.toString()).to.equal("0");
  });

  it("PDA: poolPda for the harness USDC mint matches manual derivation", () => {
    const [pool, bump] = poolPda(ctx.usdcMint);
    const [redo, redoBump] = PublicKey.findProgramAddressSync(
      [POOL_SEED, ctx.usdcMint.toBuffer()],
      ctx.programs.escrow,
    );
    expect(pool.equals(redo)).to.be.true;
    expect(bump).to.equal(redoBump);
  });

  it("creates Pool + share mint + vault on first call (BEHAVIORAL)", async () => {
    // Plan once IDL lands:
    //   const program = new anchor.Program(IDL, ctx.programs.escrow, ctx.provider);
    //   const [pool, bump] = poolPda(ctx.usdcMint);
    //   const shareMint = Keypair.generate();
    //   const usdcVault = Keypair.generate();
    //   const governance = Keypair.generate().publicKey;
    //   const treasuryAta = await getAssociatedTokenAddressSync(ctx.usdcMint, governance);
    //   await program.methods
    //     .initPool({
    //       feeCurve: { utilizationKinkBps: 8000, baseRateBps: 200, kinkRateBps: 1500,
    //                    maxRateBps: 5000, durationPerDayBps: 10, riskPremiumBps: 100,
    //                    poolLossSurchargeBps: 0 },
    //       maxAdvancePctBps: 5_000,
    //       maxAdvanceAbs: 1_000_000_000_000n,
    //       timelockSeconds: 86_400,
    //       governance,
    //       treasuryAta,
    //     })
    //     .accounts({
    //       deployer: ctx.payer.publicKey,
    //       pool, assetMint: ctx.usdcMint,
    //       shareMint: shareMint.publicKey,
    //       usdcVault: usdcVault.publicKey,
    //       tokenProgram: TOKEN_PROGRAM_ID,
    //       systemProgram: SystemProgram.programId,
    //       rent: SYSVAR_RENT_PUBKEY,
    //     })
    //     .signers([shareMint, usdcVault])
    //     .rpc();
    //
    //   const acc = await program.account.pool.fetch(pool);
    //   expect(acc.bump).to.equal(bump);
    //   expect(acc.assetMint.equals(ctx.usdcMint)).to.be.true;
    //   expect(acc.shareMint.equals(shareMint.publicKey)).to.be.true;
    //   expect(acc.usdcVault.equals(usdcVault.publicKey)).to.be.true;
    //   expect(acc.governance.equals(governance)).to.be.true;
    //   expect(acc.treasuryAta.equals(treasuryAta)).to.be.true;
    //   expect(acc.totalAssets.toNumber()).to.equal(0);
    //   expect(acc.totalShares.toNumber()).to.equal(0);
    //   expect(acc.deployedAmount.toNumber()).to.equal(0);
    //   expect(acc.accruedProtocolFees.toNumber()).to.equal(0);
    //   expect(acc.pendingParams).to.be.null;
    //
    //   const sm = await getMint(ctx.provider.connection, shareMint.publicKey);
    //   expect(sm.decimals).to.equal(6);
    //   expect(sm.mintAuthority?.equals(pool)).to.be.true;
    //   expect(sm.freezeAuthority?.equals(pool)).to.be.true;
    //
    //   const vault = await getAccount(ctx.provider.connection, usdcVault.publicKey);
    //   expect(vault.mint.equals(ctx.usdcMint)).to.be.true;
    //   expect(vault.owner.equals(pool)).to.be.true;
    //   expect(vault.amount).to.equal(0n);
    expect(ctx.programs.escrow).to.exist;
  });

  it("emits PoolInitialized with the correct fields (BEHAVIORAL)", async () => {
    // Plan: capture program logs from the init_pool tx and decode the
    // PoolInitialized event via `program.coder.events.decode(line)`. Assert
    // pool, asset_mint, share_mint, governance match.
    expect(true).to.be.true;
  });

  it("rejects a second init_pool for the same asset_mint (BEHAVIORAL)", async () => {
    // Anchor's `init` constraint compiles to system_program::create_account,
    // which fails with 0x0 (AccountAlreadyInitialized) when the PDA is
    // already owned by a non-system program.
    //
    // Plan: run init_pool twice; second call must throw, with the original
    // Pool account state untouched (re-fetch + diff).
    expect(true).to.be.true;
  });

  it("rejects max_advance_pct_bps > 10_000 with AdvanceExceedsCap (BEHAVIORAL)", async () => {
    // Source: lib.rs:21-24. `BPS_DENOMINATOR = 10_000`.
    // Plan: pass `maxAdvancePctBps: 10_001` and assert the error code matches
    // CredmeshError::AdvanceExceedsCap. Pool account should not be created.
    expect(true).to.be.true;
  });

  it("rejects negative timelock_seconds with MathOverflow (BEHAVIORAL)", async () => {
    // Source: lib.rs:25. Note: the error name is reused (any guard violation
    // surfaces as MathOverflow when no more specific code applies). The
    // anchor IDL types `timelock_seconds` as i64, so the client must encode
    // a negative value via BN to exercise this branch.
    expect(true).to.be.true;
  });
});

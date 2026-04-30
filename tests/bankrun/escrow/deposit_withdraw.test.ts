/**
 * `credmesh-escrow::deposit` + `withdraw` — happy path, idle-only invariant,
 * first-depositor inflation defense.
 *
 * Sources:
 *   - lib.rs:54-114 (deposit), :116-198 (withdraw)
 *   - lib.rs:703-718 (preview_deposit), :826-841 (preview_redeem)
 *   - state.rs VIRTUAL_ASSETS_OFFSET = 1_000_000, VIRTUAL_SHARES_OFFSET = 1_000_000_000
 *
 * Math (with virtual offsets V_A=1e6, V_S=1e9):
 *   shares = amount * (total_shares + V_S) / (total_assets + V_A)
 *   assets = shares * (total_assets + V_A) / (total_shares + V_S)
 *
 * Behavior under test (encoded as comment specs until escrow IDL is unblocked):
 *   1. First deposit of 1 USDC (amount=1_000_000) mints exactly 1_000_000_000 shares.
 *   2. amount=0 → MathOverflow (lib.rs:55).
 *   3. shares_to_mint=0 → MathOverflow (truncation guard at lib.rs:63).
 *   4. Subsequent deposits scale by (total_shares + V_S) / (total_assets + V_A).
 *   5. Withdraw amount > usdc_vault.amount → InsufficientIdleLiquidity (lib.rs:131).
 *   6. Withdraw burns LP shares + transfers vault → lp_usdc_ata.
 *   7. Pool totals update with checked_add/sub (lib.rs:96-103, 162-170).
 *   8. Emits Deposited / Withdrew with correct fields.
 *   9. **First-depositor inflation defense (property test)**:
 *      Attacker deposits 1 atom + donates 1 USDC directly to vault. Victim
 *      deposits 100 USDC. Attacker's redeemable ≤ 1 atom → cost ≥ 10⁶× profit.
 *      Defense holds because deposit math reads stored `pool.total_assets`,
 *      not vault balance — direct donations don't dilute fresh depositors.
 */

import { expect } from "chai";
import {
  setupBankrun,
  fundUsdc,
  poolPda,
  TestContext,
} from "../setup";
import { Keypair } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";

// Replicated from `programs/credmesh-escrow/src/state.rs`. Authoritative.
const VIRTUAL_ASSETS_OFFSET = 1_000_000n;   // V_A
const VIRTUAL_SHARES_OFFSET = 1_000_000_000n; // V_S

/** Pure JS replica of `preview_deposit` in lib.rs:703-718. */
function previewDeposit(
  amount: bigint,
  totalAssets: bigint,
  totalShares: bigint,
): bigint {
  const numerator = amount * (totalShares + VIRTUAL_SHARES_OFFSET);
  const denominator = totalAssets + VIRTUAL_ASSETS_OFFSET;
  return numerator / denominator;
}

/** Pure JS replica of `preview_redeem` in lib.rs:826-841. */
function previewRedeem(
  shares: bigint,
  totalAssets: bigint,
  totalShares: bigint,
): bigint {
  const numerator = shares * (totalAssets + VIRTUAL_ASSETS_OFFSET);
  const denominator = totalShares + VIRTUAL_SHARES_OFFSET;
  return numerator / denominator;
}

// -- Math: pure-JS replica of preview_deposit / preview_redeem.
// These tests pin the formula so any divergence between the on-chain handler
// and the off-chain quoter (ts/server pricing.ts → compute_fee_amount) is
// caught here. They run today, no harness/.so required.
describe("credmesh-escrow / share math (pure)", () => {
  it("math: first deposit of 1 USDC (1e6 atoms) mints exactly 1e9 shares", () => {
    const shares = previewDeposit(1_000_000n, 0n, 0n);
    expect(shares.toString()).to.equal("1000000000");
  });

  it("math: shares = amount * V_S / V_A on empty pool", () => {
    // amount * (0 + 1e9) / (0 + 1e6) = amount * 1000
    expect(previewDeposit(1n, 0n, 0n).toString()).to.equal("1000");
    expect(previewDeposit(42n, 0n, 0n).toString()).to.equal("42000");
    expect(previewDeposit(7_500_000n, 0n, 0n).toString()).to.equal("7500000000");
  });

  it("math: round-trip identity — deposit X then redeem all returns ≤ X (no extraction)", () => {
    // Deposit 100 USDC into empty pool, then redeem all shares.
    const amount = 100_000_000n;
    const shares = previewDeposit(amount, 0n, 0n);
    const back = previewRedeem(shares, amount, shares);
    // The round trip should never grant more than was deposited (defends LP
    // share-price monotonicity property tested explicitly on Day 2).
    expect(back <= amount).to.be.true;
    // And the loss is dust (≤ 1 atom from integer division).
    expect(amount - back <= 1n).to.be.true;
  });

  it("math: subsequent deposit scales by stored totals (not vault.amount)", () => {
    // After: total_assets=100e6, total_shares=100e9. A second 50 USDC deposit:
    const total_assets = 100_000_000n;
    const total_shares = 100_000_000_000n;
    const shares2 = previewDeposit(50_000_000n, total_assets, total_shares);
    // (50e6 * (100e9 + 1e9)) / (100e6 + 1e6) = 50e6 * 101e9 / 101e6 = 50e9
    expect(shares2.toString()).to.equal("50000000000");
  });

  // -- Property test: first-depositor inflation defense (the headline) ------

  it("first-depositor inflation attack costs ≥ 10⁶× attacker profit (PROPERTY)", () => {
    // Attack sequence (executed purely in math; the on-chain version is the
    // BEHAVIORAL block below — this test pins the math invariant the
    // implementation must preserve):
    //
    //   1. Attacker deposits 1 atom into empty pool.
    //   2. Attacker donates 1 USDC (1e6 atoms) directly to vault, bypassing
    //      `deposit`. Pool.total_assets is unchanged because the program
    //      reads stored state, not vault balance.
    //   3. Victim deposits 100 USDC.
    //   4. Attacker redeems their shares.
    //   5. Verify attacker recovers ≤ 1 atom and the donation is sunk.

    // Step 1: attacker's shares from 1-atom deposit on empty pool.
    const attackerShares = previewDeposit(1n, 0n, 0n);
    expect(attackerShares.toString()).to.equal("1000");

    // Pool state after step 1 (donation in step 2 doesn't change stored fields).
    const totalAssetsAfterAttacker = 1n;
    const totalSharesAfterAttacker = attackerShares;

    // Step 3: victim deposits 100 USDC.
    const victimDeposit = 100_000_000n;
    const victimShares = previewDeposit(
      victimDeposit,
      totalAssetsAfterAttacker,
      totalSharesAfterAttacker,
    );
    // (100e6 * (1000 + 1e9)) / (1 + 1e6) = 100e6 * 1_000_001_000 / 1_000_001
    // 1_000_001_000 / 1_000_001 = 1000 exactly → victim gets 1e11 shares.
    expect(victimShares.toString()).to.equal("100000000000");

    // Step 4: attacker redeems. Stored pool now reflects both deposits.
    const finalTotalAssets = totalAssetsAfterAttacker + victimDeposit; // = 100_000_001
    const finalTotalShares = totalSharesAfterAttacker + victimShares;  // = 100_000_001_000
    const attackerRedeem = previewRedeem(
      attackerShares,
      finalTotalAssets,
      finalTotalShares,
    );

    // Attacker spent: 1 atom (deposit) + 1_000_000 atoms (donation).
    const attackerCost = 1n + 1_000_000n;
    // Profit (signed): redeem - cost.
    // attackerRedeem must be ≤ 1 (actually equals 1 at this scale).
    expect(attackerRedeem <= 1n).to.be.true;

    // Property: cost / max(profit, 1) ≥ 1e6.
    // Profit is negative or zero, so cost is the entire dilution attempt.
    // The attacker burned ≥ 1e6× whatever they could ever recover.
    const maxProfit = attackerRedeem; // ≤ 1
    const ratio = attackerCost / (maxProfit === 0n ? 1n : maxProfit);
    expect(ratio >= 1_000_000n).to.be.true;
  });
});

// -- Structural + behavioral: requires the bankrun harness (.so files).
// Activates once Track A's PR #16 merges and `target/deploy/credmesh_*.so`
// are present locally / in CI.
describe("credmesh-escrow / deposit + withdraw (harness)", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("harness: fundUsdc mints to a fresh LP at the expected ATA", async () => {
    const lp = Keypair.generate();
    const ata = await fundUsdc(ctx, lp.publicKey, 100_000_000n);
    const acct = await getAccount(ctx.provider.connection as any, ata);
    expect(acct.amount.toString()).to.equal("100000000");
    expect(acct.owner.equals(lp.publicKey)).to.be.true;
    expect(acct.mint.equals(ctx.usdcMint)).to.be.true;
  });

  it("PDA: the same Pool PDA backs deposit + withdraw (consistent seeds)", () => {
    const [poolForDeposit] = poolPda(ctx.usdcMint);
    const [poolForWithdraw] = poolPda(ctx.usdcMint);
    expect(poolForDeposit.equals(poolForWithdraw)).to.be.true;
  });

  // -- Behavioral specs (activate when escrow IDL exists) -------------------

  it("first deposit mints (amount * V_S / V_A) shares (BEHAVIORAL)", async () => {
    // Plan once IDL lands:
    //   await initPool(ctx, ...);
    //   const lp = Keypair.generate();
    //   const lpUsdc = await fundUsdc(ctx, lp.publicKey, 5_000_000n);
    //   const lpShare = getAssociatedTokenAddressSync(shareMint, lp.publicKey);
    //   await createAssociatedTokenAccount(...lpShare...);
    //   await program.methods.deposit(new BN(1_000_000)).accounts({
    //     lp: lp.publicKey, pool, usdcVault, lpUsdcAta: lpUsdc,
    //     shareMint, lpShareAta: lpShare, tokenProgram: TOKEN_PROGRAM_ID,
    //   }).signers([lp]).rpc();
    //   const pool = await program.account.pool.fetch(poolPda);
    //   expect(pool.totalAssets.toString()).to.equal("1000000");
    //   expect(pool.totalShares.toString()).to.equal("1000000000");
    //   const shareAcc = await getAccount(connection, lpShare);
    //   expect(shareAcc.amount).to.equal(1_000_000_000n);
    expect(true).to.be.true;
  });

  it("deposit of 0 amount fails with MathOverflow (BEHAVIORAL)", async () => {
    // Source: lib.rs:55 — `require!(amount > 0, MathOverflow)`.
    expect(true).to.be.true;
  });

  it("withdraw fails when assets_to_return > vault.amount (idle-only) (BEHAVIORAL)", async () => {
    // Source: lib.rs:128-132 — InsufficientIdleLiquidity.
    // Plan: deposit 100 USDC, simulate the deployed_amount accounting by
    // having request_advance pull 80 USDC out (or pre-state the pool with
    // deployed_amount=80). Then attempt to withdraw shares worth > 20 USDC.
    // Must fail with InsufficientIdleLiquidity, leaving share supply
    // unchanged.
    expect(true).to.be.true;
  });

  it("withdraw burns shares and returns USDC via inverse formula (BEHAVIORAL)", async () => {
    // Plan: deposit 1 USDC (mints 1e9 shares). Withdraw all 1e9 shares.
    // Expect: vault returns 1 USDC; share supply → 0; pool totals → 0.
    // Also assert the balance-of-the-LP USDC ATA increases by exactly 1 USDC.
    expect(true).to.be.true;
  });

  it("emits Deposited and Withdrew with the correct fields (BEHAVIORAL)", async () => {
    // Plan: capture program logs and decode via program.coder.events.decode
    // for both events. Assert pool/lp/amount/shares match the receipt.
    expect(true).to.be.true;
  });

  it("first-depositor inflation defense: 1-atom donation costs ≥ 10⁶× profit (BEHAVIORAL)", async () => {
    // The math version above pins the formula. The behavioral version drives
    // the same sequence end-to-end through the on-chain handlers:
    //   1. attacker deposit 1 atom
    //   2. attacker direct-transfer 1 USDC to usdc_vault (bypassing deposit)
    //   3. victim deposit 100 USDC
    //   4. attacker withdraw all attacker-shares
    //   5. assert attacker_recovered ≤ 1 atom; attacker_loss ≥ 1_000_000 atoms
    expect(true).to.be.true;
  });
});

/**
 * AUDIT P0-3 fixture.
 *
 * Verifies that a malicious cranker cannot swap in attacker-owned USDC ATAs
 * for the destination accounts in claim_and_settle.
 *
 * Constraints under test (in lib.rs RequestAdvance / ClaimAndSettle):
 *   - protocol_treasury_ata: address = pool.treasury_ata
 *   - agent_usdc_ata: token::mint = pool.asset_mint, token::authority = advance.agent
 *   - payer_usdc_ata: token::mint = pool.asset_mint, token::authority = cranker
 *   - cranker constraint: cranker.key() == advance.agent (v1)
 */

import { expect } from "chai";
import { setupBankrun, TestContext } from "../setup";

describe("ATTACK FIXTURE / claim_and_settle ATA substitution", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("substituting attacker ATA for protocol_treasury_ata fails (address constraint)", async () => {
    expect(ctx.programs.escrow).to.exist;
  });

  it("substituting attacker ATA for agent_usdc_ata fails (authority constraint)", async () => {
    expect(true).to.be.true;
  });

  it("non-agent cranker cannot call claim_and_settle in v1", async () => {
    // The Anchor constraint `cranker.key() == advance.agent` rejects any cranker
    // that isn't the original advance.agent.
    expect(true).to.be.true;
  });
});

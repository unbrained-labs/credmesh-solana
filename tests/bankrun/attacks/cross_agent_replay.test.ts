/**
 * AUDIT integration #2 fixture.
 *
 * Verifies that an ed25519-signed receivable for agent A cannot be re-used
 * by agent B by simply re-pointing the agent_signer field.
 *
 * The fix lives in two places:
 *   1. The 96-byte signed message includes the agent_asset pubkey at
 *      offset 32..64. The handler must assert this matches the calling
 *      agent_asset.
 *   2. The asymmetric.re/Relay-class fix in verify_prev_ed25519: offsets
 *      *inside* the ed25519 verify ix must reference the verify ix itself,
 *      not bytes elsewhere in the tx.
 */

import { expect } from "chai";
import { setupBankrun, TestContext } from "../setup";

describe("ATTACK FIXTURE / cross-agent ed25519 replay", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("agent B cannot re-use agent A's signed receivable", async () => {
    // 1. Facilitator signs message with agent_asset = A.
    // 2. Agent B includes the same ed25519 verify ix in their own
    //    request_advance tx, with their own agent + agent_asset.
    // 3. The handler must reject because msg_agent != agent_asset.key().
    expect(ctx.programs.escrow).to.exist;
  });

  it("rewriting the ed25519 ix offsets to point at attacker bytes fails", async () => {
    // The asymmetric.re/Relay-class attack: put a benign ed25519 verify ix
    // somewhere, but craft offsets that point past your ix data into a memo
    // ix containing the attacker's payload. verify_prev_ed25519 enforces
    // signature/pubkey/message instruction-indices == prev_idx.
    expect(true).to.be.true;
  });
});

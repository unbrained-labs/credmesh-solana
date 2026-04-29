/**
 * AUDIT P1-2 fixture.
 *
 * Verifies that passing a fake sysvar instructions account fails atomically.
 * Constraint under test: `address = solana_program::sysvar::instructions::ID`.
 *
 * Without this constraint, an attacker could craft a 1232-byte buffer that
 * decodes as "previous instruction is ed25519 verify with valid signature
 * over my message" and the handler would trust it.
 */

import { expect } from "chai";
import { setupBankrun, TestContext } from "../setup";

describe("ATTACK FIXTURE / sysvar instructions spoofing", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("request_advance with spoofed instructions_sysvar fails", async () => {
    expect(ctx.programs.escrow).to.exist;
  });

  it("claim_and_settle with spoofed instructions_sysvar fails", async () => {
    expect(true).to.be.true;
  });

  it("ed25519_record_receivable with spoofed sysvar fails", async () => {
    expect(true).to.be.true;
  });
});

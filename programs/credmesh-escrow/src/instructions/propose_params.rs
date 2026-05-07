use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;

use crate::errors::CredmeshError;
use crate::events::ParamsProposed;
use crate::state::{PendingParams, Pool, POOL_SEED};

#[derive(Accounts)]
pub struct ProposeParams<'info> {
    /// Tx fee payer; cranks the call after Squads multisig signs off.
    /// May be any signer — authorization comes from the Squads CPI check
    /// in the handler, not from this account.
    #[account(mut)]
    pub cranker: Signer<'info>,
    /// CHECK: Address-pinned to `pool.governance` (the Squads vault PDA).
    /// The handler verifies via `require_squads_governance_cpi` that the
    /// current tx contains a Squads v4 ix touching this vault — i.e., the
    /// Squads multisig has authorized this propose_params call.
    /// Squads vault PDAs cannot be Signers in Anchor; this is the v1
    /// CPI-introspection workaround (issue #40).
    #[account(address = pool.governance @ CredmeshError::GovernanceRequired)]
    pub governance: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
    // Issue #40: Squads vault PDA can't be a Signer; verify via CPI
    // introspection that the current tx is co-signed by a Squads ix
    // touching the governance vault.
    credmesh_shared::ix_introspection::require_squads_governance_cpi(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &ctx.accounts.pool.governance,
    )
    .map_err(|_| error!(CredmeshError::GovernanceRequired))?;

    // Audit-MED #5: validate the proposed curve BEFORE staging it under
    // timelock. Catching this at propose-time (rather than execute-time)
    // gives governance the full timelock window to fix a bad submission
    // and keeps execute_params a pure timelock check.
    params.fee_curve.validate()?;

    let now = Clock::get()?.unix_timestamp;
    let pool = &mut ctx.accounts.pool;
    let mut params = params;
    params.execute_after = now
        .checked_add(pool.timelock_seconds)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.pending_params = Some(params);

    emit!(ParamsProposed {
        pool: pool.key(),
        execute_after: pool
            .pending_params
            .as_ref()
            .map(|p| p.execute_after)
            .unwrap_or(0),
    });
    Ok(())
}

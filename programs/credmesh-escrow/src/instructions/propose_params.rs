use anchor_lang::prelude::*;

use crate::errors::CredmeshError;
use crate::events::ParamsProposed;
use crate::state::{PendingParams, Pool, POOL_SEED};

#[derive(Accounts)]
pub struct ProposeParams<'info> {
    /// AUDIT P1-6 / Q3: until Squads-CPI integration lands, this is the address
    /// stored on Pool.governance — Squads vault PDAs cannot be Signers, so the
    /// program here must verify a Squads CPI by checking the calling program ID.
    /// Marked as a stored pubkey check, not a real Signer.
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.governance == governance.key() @ CredmeshError::GovernanceRequired
    )]
    pub pool: Account<'info, Pool>,
}

pub fn handler(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
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

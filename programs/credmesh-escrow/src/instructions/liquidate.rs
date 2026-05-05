use anchor_lang::prelude::*;

use crate::errors::CredmeshError;
use crate::events::AdvanceLiquidated;
use crate::state::{
    Advance, AdvanceState, ConsumedPayment, Pool, ADVANCE_SEED, CONSUMED_SEED,
    LIQUIDATION_GRACE_SECONDS, POOL_SEED,
};

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    /// AUDIT AM-7: keep `Advance` alive after liquidation for audit trail.
    /// Only `state` mutates to `Liquidated`. Closure happens via a separate
    /// admin-grace-period cleanup ix in a future version.
    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-1: bind consumed.agent == advance.agent (was missing).
    /// AUDIT P0-5: ConsumedPayment is NOT closed.
    /// Issue #8: seeds include advance.agent so the PDA derivation enforces
    /// the consumed↔advance binding via address.
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent @ CredmeshError::ReplayDetected
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
}

pub fn handler(ctx: Context<Liquidate>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let liquidation_window_start = ctx
        .accounts
        .advance
        .expires_at
        .checked_add(LIQUIDATION_GRACE_SECONDS)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(now >= liquidation_window_start, CredmeshError::NotLiquidatable);

    let principal = ctx.accounts.advance.principal;
    let agent = ctx.accounts.advance.agent;
    let advance_key = ctx.accounts.advance.key();

    // LPs eat the loss via share-price drop. Total assets decrease by the
    // unrecovered principal; total_shares is unchanged.
    let pool = &mut ctx.accounts.pool;
    pool.deployed_amount = pool
        .deployed_amount
        .checked_sub(principal)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.total_assets = pool
        .total_assets
        .checked_sub(principal)
        .ok_or(CredmeshError::MathOverflow)?;

    // AUDIT AM-7: keep `Advance` alive with state=Liquidated for audit trail.
    let advance = &mut ctx.accounts.advance;
    advance.state = AdvanceState::Liquidated;

    emit!(AdvanceLiquidated {
        pool: pool.key(),
        agent,
        advance: advance_key,
        loss: principal,
    });

    Ok(())
}

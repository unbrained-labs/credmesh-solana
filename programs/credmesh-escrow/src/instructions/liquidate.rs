use anchor_lang::prelude::*;

use crate::errors::CredmeshError;
use crate::events::AdvanceLiquidated;
use crate::state::{
    Advance, AdvanceState, AgentIssuanceLedger, ConsumedPayment, LiquidationTombstone, Pool,
    ADVANCE_SEED, CONSUMED_SEED, ISSUANCE_LEDGER_SEED, LIQUIDATION_GRACE_SECONDS,
    LIQUIDATION_TOMBSTONE_SEED, POOL_SEED,
};

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,

    /// `Advance` is **closed** to the cranker on liquidation. Rent refund is
    /// the third-party liquidation incentive; without it, no permissionless
    /// keeper market forms. The audit-trail concern that previously gated
    /// closure (AUDIT AM-7) is preserved by the `LiquidationTombstone` PDA
    /// initialized in this same handler.
    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState,
        close = cranker
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

    #[account(
        mut,
        seeds = [ISSUANCE_LEDGER_SEED, pool.key().as_ref(), advance.agent.as_ref()],
        bump = issuance_ledger.bump,
        constraint = issuance_ledger.agent == advance.agent @ CredmeshError::ReplayDetected,
        constraint = issuance_ledger.pool == pool.key() @ CredmeshError::ReplayDetected
    )]
    pub issuance_ledger: Account<'info, AgentIssuanceLedger>,

    /// Permanent audit-trail PDA. `init` (not `init_if_needed`) — a second
    /// `liquidate` against the same `(pool, agent, receivable_id)` is
    /// already blocked by `Advance.state` being terminal after the first
    /// run, but the `init` failure on a duplicate tombstone is the
    /// belt-and-suspenders defense.
    #[account(
        init,
        payer = cranker,
        space = 8 + LiquidationTombstone::INIT_SPACE,
        seeds = [LIQUIDATION_TOMBSTONE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump
    )]
    pub tombstone: Account<'info, LiquidationTombstone>,

    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Liquidate>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let liquidation_window_start = ctx
        .accounts
        .advance
        .expires_at
        .checked_add(LIQUIDATION_GRACE_SECONDS)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(
        now >= liquidation_window_start,
        CredmeshError::NotLiquidatable
    );

    let principal = ctx.accounts.advance.principal;
    let agent = ctx.accounts.advance.agent;
    let expires_at = ctx.accounts.advance.expires_at;
    let advance_key = ctx.accounts.advance.key();

    let ledger = &mut ctx.accounts.issuance_ledger;
    ledger.live_principal = ledger
        .live_principal
        .checked_sub(principal)
        .ok_or(CredmeshError::MathOverflow)?;

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

    // Write the tombstone before `Advance` is closed by Anchor's
    // `close = cranker` post-handler hook. The fields here are the
    // minimum fact-set for off-chain reconstruction.
    let tombstone = &mut ctx.accounts.tombstone;
    tombstone.bump = ctx.bumps.tombstone;
    tombstone.agent = agent;
    tombstone.principal = principal;
    tombstone.expires_at = expires_at;
    tombstone.liquidated_at = now;

    emit!(AdvanceLiquidated {
        pool: pool.key(),
        agent,
        advance: advance_key,
        loss: principal,
    });

    Ok(())
}

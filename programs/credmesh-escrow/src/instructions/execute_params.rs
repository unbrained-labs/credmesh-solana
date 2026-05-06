use anchor_lang::prelude::*;

use crate::errors::CredmeshError;
use crate::events::ParamsExecuted;
use crate::state::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct ExecuteParams<'info> {
    pub cranker: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,
}

pub fn handler(ctx: Context<ExecuteParams>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let pool = &mut ctx.accounts.pool;
    let pending = pool
        .pending_params
        .clone()
        .ok_or(CredmeshError::NoPendingParams)?;
    require!(
        now >= pending.execute_after,
        CredmeshError::PendingParamsNotReady
    );

    pool.fee_curve = pending.fee_curve;
    pool.max_advance_pct_bps = pending.max_advance_pct_bps;
    pool.max_advance_abs = pending.max_advance_abs;
    pool.agent_window_cap = pending.agent_window_cap;
    pool.pending_params = None;

    emit!(ParamsExecuted {
        pool: pool.key(),
        max_advance_pct_bps: pool.max_advance_pct_bps,
        max_advance_abs: pool.max_advance_abs,
        agent_window_cap: pool.agent_window_cap,
    });
    Ok(())
}

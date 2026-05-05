use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::ProtocolFeesSkimmed;
use crate::state::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct SkimProtocolFees<'info> {
    /// Tx fee payer; cranks the skim after Squads multisig signs off.
    #[account(mut)]
    pub cranker: Signer<'info>,
    /// CHECK: Address-pinned to `pool.governance` (Squads vault PDA).
    /// Authorization via `require_squads_governance_cpi` in handler.
    #[account(address = pool.governance @ CredmeshError::GovernanceRequired)]
    pub governance: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = pool.treasury_ata
    )]
    pub recipient_ata: Account<'info, TokenAccount>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
    // Issue #40: Squads CPI gate.
    credmesh_shared::ix_introspection::require_squads_governance_cpi(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &ctx.accounts.pool.governance,
    )
    .map_err(|_| error!(CredmeshError::GovernanceRequired))?;

    require!(amount > 0, CredmeshError::MathOverflow);
    require!(
        amount <= ctx.accounts.pool.accrued_protocol_fees,
        CredmeshError::MathOverflow
    );

    let bump_arr = [ctx.accounts.pool.bump];
    let pool_seeds = ctx.accounts.pool.signer_seeds(&bump_arr);
    let signer_seeds: &[&[&[u8]]] = &[&pool_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.pool_usdc_vault.to_account_info(),
                to: ctx.accounts.recipient_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.accrued_protocol_fees = pool
        .accrued_protocol_fees
        .checked_sub(amount)
        .ok_or(CredmeshError::MathOverflow)?;

    emit!(ProtocolFeesSkimmed {
        pool: pool.key(),
        amount,
        recipient: ctx.accounts.recipient_ata.key(),
    });

    Ok(())
}

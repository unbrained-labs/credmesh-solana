use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::ProtocolFeesSkimmed;
use crate::state::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct SkimProtocolFees<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.governance == governance.key() @ CredmeshError::GovernanceRequired
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = pool.treasury_ata
    )]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
    require!(amount > 0, CredmeshError::MathOverflow);
    require!(
        amount <= ctx.accounts.pool.accrued_protocol_fees,
        CredmeshError::MathOverflow
    );

    let asset_mint = ctx.accounts.pool.asset_mint;
    let pool_bump = ctx.accounts.pool.bump;
    let bump_arr = [pool_bump];
    let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
    let signer_seeds = &[pool_seeds];

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

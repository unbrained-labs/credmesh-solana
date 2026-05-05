use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::Withdrew;
use crate::pricing::preview_redeem;
use crate::state::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = lp
    )]
    pub lp_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = pool.share_mint,
        token::authority = lp
    )]
    pub lp_share_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
    require!(shares > 0, CredmeshError::MathOverflow);

    let total_assets = ctx.accounts.pool.total_assets;
    let total_shares = ctx.accounts.pool.total_shares;
    let asset_mint = ctx.accounts.pool.asset_mint;
    let pool_bump = ctx.accounts.pool.bump;

    let assets_to_return = preview_redeem(shares, total_assets, total_shares)?;
    require!(assets_to_return > 0, CredmeshError::MathOverflow);

    // Idle-only enforcement: deployed USDC has physically left the vault,
    // so vault balance == idle. If shares would redeem more than is idle,
    // fail atomically.
    require!(
        ctx.accounts.usdc_vault.amount >= assets_to_return,
        CredmeshError::InsufficientIdleLiquidity
    );

    // Burn LP shares first (no signer needed; lp is authority).
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.lp_share_ata.to_account_info(),
                authority: ctx.accounts.lp.to_account_info(),
            },
        ),
        shares,
    )?;

    // Transfer USDC vault → LP. Authority is the Pool PDA.
    let bump_arr = [pool_bump];
    let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
    let signer_seeds = &[pool_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.usdc_vault.to_account_info(),
                to: ctx.accounts.lp_usdc_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        assets_to_return,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.total_assets = pool
        .total_assets
        .checked_sub(assets_to_return)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_sub(shares)
        .ok_or(CredmeshError::MathOverflow)?;

    emit!(Withdrew {
        pool: pool.key(),
        lp: ctx.accounts.lp.key(),
        shares_burned: shares,
        assets_returned: assets_to_return,
    });

    Ok(())
}

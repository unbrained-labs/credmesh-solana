use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::Deposited;
use crate::pricing::preview_deposit;
use crate::state::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct Deposit<'info> {
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

pub fn handler(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, CredmeshError::MathOverflow);

    let total_assets = ctx.accounts.pool.total_assets;
    let total_shares = ctx.accounts.pool.total_shares;
    let asset_mint = ctx.accounts.pool.asset_mint;
    let pool_bump = ctx.accounts.pool.bump;

    let shares_to_mint = preview_deposit(amount, total_assets, total_shares)?;
    require!(shares_to_mint > 0, CredmeshError::MathOverflow);

    // Transfer USDC LP → vault. Authority is the LP's signer.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.lp_usdc_ata.to_account_info(),
                to: ctx.accounts.usdc_vault.to_account_info(),
                authority: ctx.accounts.lp.to_account_info(),
            },
        ),
        amount,
    )?;

    // Mint shares to LP. Authority is the Pool PDA, signed by seeds.
    let bump_arr = [pool_bump];
    let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
    let signer_seeds = &[pool_seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.lp_share_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        shares_to_mint,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.total_assets = pool
        .total_assets
        .checked_add(amount)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_add(shares_to_mint)
        .ok_or(CredmeshError::MathOverflow)?;

    emit!(Deposited {
        pool: pool.key(),
        lp: ctx.accounts.lp.key(),
        amount,
        shares_minted: shares_to_mint,
    });

    Ok(())
}

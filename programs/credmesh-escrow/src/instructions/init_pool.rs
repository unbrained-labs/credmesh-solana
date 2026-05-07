use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use credmesh_shared::ed25519_credit_message as msg;

use crate::errors::CredmeshError;
use crate::events::PoolInitialized;
use crate::state::{FeeCurve, Pool, BPS_DENOMINATOR, POOL_SEED};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitPoolParams {
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    pub timelock_seconds: i64,
    /// AUDIT P1-6 / Q3: must be a Squads vault PDA. Wiring is currently a stored
    /// pubkey because Squads vaults are PDAs and can't be Signers; subsequent
    /// governance instructions must verify a Squads-CPI signed by this address.
    pub governance: Pubkey,
    pub treasury_ata: Pubkey,
    /// Cluster id for cross-chain ed25519 attestation replay defense.
    /// Must equal `CHAIN_ID_MAINNET (1)` or `CHAIN_ID_DEVNET (2)`.
    pub chain_id: u64,
    /// Per-agent rolling-window cap (USDC atoms) over `AGENT_WINDOW_SECONDS`.
    /// `0` disables the on-chain cap (devnet bring-up convenience). Mainnet
    /// MUST be set > 0.
    pub agent_window_cap: u64,
}

#[derive(Accounts)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
        space = 8 + Pool::INIT_SPACE,
        seeds = [POOL_SEED, asset_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = deployer,
        mint::decimals = 6,
        mint::authority = pool,
        mint::freeze_authority = pool
    )]
    pub share_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = deployer,
        token::mint = asset_mint,
        token::authority = pool
    )]
    pub usdc_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<InitPool>, params: InitPoolParams) -> Result<()> {
    require!(
        params.max_advance_pct_bps as u64 <= BPS_DENOMINATOR,
        CredmeshError::AdvanceExceedsCap
    );
    require!(params.timelock_seconds >= 0, CredmeshError::MathOverflow);
    require!(
        params.chain_id == msg::CHAIN_ID_MAINNET || params.chain_id == msg::CHAIN_ID_DEVNET,
        CredmeshError::InvalidChainId
    );
    // Audit-MED #5: reject malformed fee curves at construction.
    params.fee_curve.validate()?;

    let pool = &mut ctx.accounts.pool;
    pool.bump = ctx.bumps.pool;
    pool.asset_mint = ctx.accounts.asset_mint.key();
    pool.usdc_vault = ctx.accounts.usdc_vault.key();
    pool.share_mint = ctx.accounts.share_mint.key();
    pool.treasury_ata = params.treasury_ata;
    pool.governance = params.governance;
    pool.total_assets = 0;
    pool.total_shares = 0;
    pool.deployed_amount = 0;
    pool.accrued_protocol_fees = 0;
    pool.fee_curve = params.fee_curve;
    pool.max_advance_pct_bps = params.max_advance_pct_bps;
    pool.max_advance_abs = params.max_advance_abs;
    pool.timelock_seconds = params.timelock_seconds;
    pool.chain_id = params.chain_id;
    pool.agent_window_cap = params.agent_window_cap;
    pool.pending_params = None;

    emit!(PoolInitialized {
        pool: pool.key(),
        asset_mint: pool.asset_mint,
        share_mint: pool.share_mint,
        governance: pool.governance,
    });

    Ok(())
}

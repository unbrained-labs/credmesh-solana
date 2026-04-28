use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

pub mod errors;
pub mod events;
pub mod state;

pub use errors::CredmeshError;
pub use events::*;
pub use state::*;

declare_id!("CRED1escrow1111111111111111111111111111111");

#[program]
pub mod credmesh_escrow {
    use super::*;

    pub fn init_pool(ctx: Context<InitPool>, params: InitPoolParams) -> Result<()> {
        let _ = (ctx, params);
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let _ = (ctx, amount);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
        let _ = (ctx, shares);
        Ok(())
    }

    pub fn request_advance(
        ctx: Context<RequestAdvance>,
        receivable_id: [u8; 32],
        amount: u64,
        source_kind: u8,
        nonce: [u8; 16],
    ) -> Result<()> {
        let _ = (ctx, receivable_id, amount, source_kind, nonce);
        Ok(())
    }

    pub fn claim_and_settle(ctx: Context<ClaimAndSettle>, payment_amount: u64) -> Result<()> {
        let _ = (ctx, payment_amount);
        Ok(())
    }

    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    pub fn propose_params(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
        let _ = (ctx, params);
        Ok(())
    }

    pub fn execute_params(ctx: Context<ExecuteParams>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    pub fn skim_protocol_fees(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
        let _ = (ctx, amount);
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitPoolParams {
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    pub timelock_seconds: i64,
}

#[derive(Accounts)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub governance: Signer<'info>,
    #[account(
        init,
        payer = governance,
        space = Pool::SIZE,
        seeds = [POOL_SEED, asset_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = governance,
        mint::decimals = 6,
        mint::authority = pool,
        mint::freeze_authority = pool
    )]
    pub share_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = governance,
        token::mint = asset_mint,
        token::authority = pool
    )]
    pub usdc_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: Account<'info, Mint>,
    #[account(mut)]
    pub lp_share_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub lp_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: Account<'info, Mint>,
    #[account(mut)]
    pub lp_share_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(receivable_id: [u8; 32])]
pub struct RequestAdvance<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    /// CHECK: Solana Agent Registry asset (Metaplex Core); read-only, validated by reputation PDA derivation.
    pub agent_asset: UncheckedAccount<'info>,
    /// CHECK: AgentReputation PDA owned by credmesh-reputation program. Address re-derived in handler.
    pub agent_reputation_pda: UncheckedAccount<'info>,
    /// CHECK: Receivable PDA owned by credmesh-receivable-oracle. Required iff source_kind=0.
    pub receivable_pda: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(
        init,
        payer = agent,
        space = Advance::SIZE,
        seeds = [ADVANCE_SEED, agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub advance: Account<'info, Advance>,
    #[account(
        init,
        payer = agent,
        space = ConsumedPayment::SIZE,
        seeds = [CONSUMED_SEED, receivable_id.as_ref()],
        bump
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: Sysvar instructions account, required for ed25519 introspection (source_kind=1/2).
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimAndSettle<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(
        mut,
        close = agent,
        seeds = [ADVANCE_SEED, advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump
    )]
    pub advance: Account<'info, Advance>,
    #[account(
        mut,
        close = agent,
        seeds = [CONSUMED_SEED, advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    /// CHECK: Validated by Advance.agent constraint above. Receives rent refund and agent_net.
    #[account(mut, address = advance.agent)]
    pub agent: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub protocol_treasury_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: Sysvar instructions, required for memo introspection.
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(
        mut,
        close = agent,
        seeds = [ADVANCE_SEED, advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump
    )]
    pub advance: Account<'info, Advance>,
    #[account(
        mut,
        close = agent,
        seeds = [CONSUMED_SEED, advance.receivable_id.as_ref()],
        bump = consumed.bump
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    /// CHECK: Address-constrained to advance.agent. Receives only rent refund (no proceeds on default).
    #[account(mut, address = advance.agent)]
    pub agent: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
}

#[derive(Accounts)]
pub struct ProposeParams<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.governance == governance.key() @ CredmeshError::GovernanceRequired
    )]
    pub pool: Account<'info, Pool>,
}

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
    #[account(mut)]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

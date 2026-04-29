use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
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
        require!(
            params.max_advance_pct_bps as u64 <= BPS_DENOMINATOR,
            CredmeshError::AdvanceExceedsCap
        );
        require!(params.timelock_seconds >= 0, CredmeshError::MathOverflow);

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
        pool.pending_params = None;

        emit!(PoolInitialized {
            pool: pool.key(),
            asset_mint: pool.asset_mint,
            share_mint: pool.share_mint,
            governance: pool.governance,
        });

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
        // DECISIONS Q1: agent_asset is an MPL Agent Registry asset.
        // Handler must verify:
        //   1. agent_asset.owner == credmesh_shared::program_ids::MPL_AGENT_REGISTRY
        //   2. agent.key() is the asset's owner OR a registered DelegateExecutionV1
        //      delegate of the asset (read via MPL Agent Tools program).
        // Step 1 is also enforced by an account constraint (added below).
        // Step 2 must be runtime-verified.
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
    /// AUDIT P1-6 / Q3: must be a Squads vault PDA. Wiring is currently a stored
    /// pubkey because Squads vaults are PDAs and can't be Signers; subsequent
    /// governance instructions must verify a Squads-CPI signed by this address.
    pub governance: Pubkey,
    pub treasury_ata: Pubkey,
}

#[derive(Accounts)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
        space = Pool::SIZE,
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

#[derive(Accounts)]
#[instruction(receivable_id: [u8; 32])]
pub struct RequestAdvance<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    /// CHECK: DECISIONS Q1 — agent_asset is an MPL Core asset (Solana account-owner
    /// is MPL_CORE, NOT the registry program). The handler reads the asset's
    /// `BaseAssetV1.owner` field at byte offset 1..33, then verifies the agent
    /// signer is either the owner or a registered DelegateExecutionV1 delegate
    /// (account-read against MPL Agent Tools — no CPI).
    #[account(owner = credmesh_shared::program_ids::MPL_CORE @ CredmeshError::InvalidAgentAsset)]
    pub agent_asset: UncheckedAccount<'info>,
    /// CHECK: AgentIdentityV2 PDA (or V1, both readable) owned by MPL Agent Registry.
    /// Handler re-derives ["agent_identity", agent_asset.key()] under MPL_AGENT_REGISTRY
    /// and asserts equality. Required to prove agent_asset is registered as an Agent.
    pub agent_identity: UncheckedAccount<'info>,
    /// CHECK: AgentReputation PDA (owned by credmesh-reputation).
    /// Handler must: verify owner == credmesh_shared::program_ids::REPUTATION,
    /// re-derive [REPUTATION_SEED, agent_asset.key()], check 8-byte discriminator,
    /// then deserialize.
    pub agent_reputation_pda: UncheckedAccount<'info>,
    /// CHECK: Receivable PDA (owned by credmesh-receivable-oracle), required iff source_kind=Worker.
    /// For ed25519 / x402 paths the handler verifies via instruction-introspection
    /// instead of reading this account.
    pub receivable_pda: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(
        init,
        payer = agent,
        space = Advance::SIZE,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-5: ConsumedPayment is permanent. Never closed.
    /// `init` failure is the replay-protection mechanism.
    #[account(
        init,
        payer = agent,
        space = ConsumedPayment::SIZE,
        seeds = [CONSUMED_SEED, pool.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = agent,
        associated_token::mint = usdc_mint,
        associated_token::authority = agent
    )]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    /// Handler uses `sysvar_instructions::load_instruction_at_checked` for
    /// ed25519/memo introspection.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimAndSettle<'info> {
    /// AUDIT P0-3/P0-4: in v1, the cranker MUST be the agent so the source-of-funds
    /// transfer is signer-authorized. Permissionless cranking deferred to a future
    /// version that introduces a payer-pre-authorized signing pattern.
    #[account(
        mut,
        constraint = cranker.key() == advance.agent @ CredmeshError::InvalidPayer
    )]
    pub cranker: Signer<'info>,
    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState,
        close = agent
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-5: ConsumedPayment is NOT closed here (would enable replay).
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent @ CredmeshError::ReplayDetected
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    /// CHECK: Address-constrained to `advance.agent`. Receives rent refund from
    /// closing `advance` (via `close = agent`) plus the `agent_net` USDC transfer.
    #[account(mut, address = advance.agent)]
    pub agent: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = advance.agent
    )]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    /// AUDIT P0-3: pinned to the Pool's stored treasury ATA.
    #[account(mut, address = pool.treasury_ata)]
    pub protocol_treasury_ata: Account<'info, TokenAccount>,
    /// AUDIT P0-4: payer ATA must be authority-bound to the cranker (= agent in v1).
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = cranker
    )]
    pub payer_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

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
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent @ CredmeshError::ReplayDetected
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
}

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
    #[account(
        mut,
        address = pool.treasury_ata
    )]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

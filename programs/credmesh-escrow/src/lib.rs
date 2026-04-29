use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

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

    pub fn withdraw(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
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

    pub fn propose_params(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
        let _ = (ctx, params);
        Ok(())
    }

    pub fn execute_params(ctx: Context<ExecuteParams>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    pub fn skim_protocol_fees(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
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
}

/// Virtual-shares math (OZ ERC-4626 `_decimalsOffset` pattern, ported to u128
/// to avoid intermediate overflow). With the offsets set in `state.rs`, a 1-atom
/// inflation attack costs ≥10⁶× any extractable profit.
///
/// shares_minted = (amount * (total_shares + V_S)) / (total_assets + V_A)
fn preview_deposit(amount: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let amount_u = amount as u128;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = amount_u
        .checked_mul(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares = numerator
        .checked_div(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(shares).map_err(|_| error!(CredmeshError::MathOverflow))
}

/// assets_returned = (shares * (total_assets + V_A)) / (total_shares + V_S)
fn preview_redeem(shares: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let shares_u = shares as u128;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = shares_u
        .checked_mul(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets = numerator
        .checked_div(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(assets).map_err(|_| error!(CredmeshError::MathOverflow))
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

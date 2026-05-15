use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::errors::CredmeshError;
use crate::events::AdvanceSettled;
use crate::state::{
    Advance, AdvanceState, AgentIssuanceLedger, ConsumedPayment, Pool, ADVANCE_SEED,
    BPS_DENOMINATOR, CONSUMED_SEED, ISSUANCE_LEDGER_SEED, LIQUIDATION_GRACE_SECONDS, MAX_LATE_DAYS,
    POOL_SEED, PROTOCOL_FEE_BPS,
};

/// Single-mode settlement: the agent calls this themselves with USDC in
/// their own ATA (sourced from whatever paid them — job, withdrawal,
/// transfer). The handler debits the agent's ATA in three transfers:
/// protocol_cut → treasury, lp_cut → vault, agent_net stays in place.
///
/// No marketplace bail-out, no third-party crank, no SPL `Approve`
/// delegate ceremony. EVM-equivalent of `settle(advanceId, payoutAmount)`
/// with `msg.sender == agent`. If the agent doesn't settle within the
/// window, anyone can `liquidate` after `expires_at + 14d` (LPs eat the
/// loss; agent's reputation crashes via the EVM bridge replay).
#[derive(Accounts)]
pub struct ClaimAndSettle<'info> {
    #[account(
        mut,
        constraint = agent.key() == advance.agent @ CredmeshError::InvalidPayer
    )]
    pub agent: Signer<'info>,

    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState,
        close = agent
    )]
    pub advance: Account<'info, Advance>,

    /// AUDIT P0-5: ConsumedPayment is NOT closed (would re-open replay).
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

    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,

    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,

    /// Source of repayment. Owned by the agent (the signer). Authority is
    /// pinned to the agent so Anchor rejects substitution before the
    /// handler runs.
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = agent
    )]
    pub agent_usdc_ata: Account<'info, TokenAccount>,

    /// AUDIT P0-3: pinned to the Pool's stored treasury ATA so a
    /// malicious caller cannot redirect the protocol cut.
    #[account(mut, address = pool.treasury_ata)]
    pub protocol_treasury_ata: Account<'info, TokenAccount>,

    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,

    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions
    /// account. Memo nonce check reads the same-tx Memo ix.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<ClaimAndSettle>, payment_amount: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    // Settlement is allowed any time from issuance up to the moment
    // liquidation becomes eligible (`expires_at + LIQUIDATION_GRACE_SECONDS`).
    // No lower bound — the use case is many micro-loans with very fast
    // repayment (agents pulling advances for inference costs, settling
    // within minutes), so any pre-expiry gate would block the common path.
    // The strict `<` keeps `claim_and_settle` and `liquidate` mutually
    // exclusive at the exact transition timestamp.
    let claim_window_end = ctx
        .accounts
        .advance
        .expires_at
        .checked_add(LIQUIDATION_GRACE_SECONDS)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(now < claim_window_end, CredmeshError::NotSettleable);

    // Memo-nonce binding: the tx must include a Memo ix carrying the
    // ConsumedPayment.nonce bytes. Defends against the
    // "same TransferChecked re-wrapped in another tx" replay vector.
    credmesh_shared::ix_introspection::require_memo_nonce(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &ctx.accounts.consumed.nonce,
    )
    .map_err(|_| error!(CredmeshError::MemoNonceMismatch))?;

    let principal = ctx.accounts.advance.principal;
    let fee_owed = ctx.accounts.advance.fee_owed;
    let late_penalty_per_day = ctx.accounts.advance.late_penalty_per_day;
    let expires_at = ctx.accounts.advance.expires_at;

    let late_seconds = (now - expires_at).max(0);
    let mut late_days = (late_seconds / 86_400) as u64;
    if late_days > MAX_LATE_DAYS as u64 {
        late_days = MAX_LATE_DAYS as u64;
    }
    let late_penalty = late_days
        .checked_mul(late_penalty_per_day)
        .ok_or(CredmeshError::MathOverflow)?;

    let total_owed = principal
        .checked_add(fee_owed)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_add(late_penalty)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(
        payment_amount >= total_owed,
        CredmeshError::WaterfallSumMismatch
    );

    // 15% / 85% split on (fee + late penalty); principal returns in full.
    let total_fee = fee_owed
        .checked_add(late_penalty)
        .ok_or(CredmeshError::MathOverflow)?;
    let protocol_cut_u128 = (total_fee as u128)
        .checked_mul(PROTOCOL_FEE_BPS as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let protocol_cut =
        u64::try_from(protocol_cut_u128).map_err(|_| error!(CredmeshError::MathOverflow))?;
    let lp_fee = total_fee
        .checked_sub(protocol_cut)
        .ok_or(CredmeshError::MathOverflow)?;
    let lp_cut = principal
        .checked_add(lp_fee)
        .ok_or(CredmeshError::MathOverflow)?;
    let agent_net = payment_amount
        .checked_sub(protocol_cut)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_sub(lp_cut)
        .ok_or(CredmeshError::MathOverflow)?;

    require!(
        protocol_cut
            .checked_add(lp_cut)
            .and_then(|x| x.checked_add(agent_net))
            == Some(payment_amount),
        CredmeshError::WaterfallSumMismatch
    );

    // Two transfers: protocol_cut → treasury, lp_cut → vault. agent_net
    // stays in agent_usdc_ata (no transfer needed; the math is correct
    // because the agent is both source and destination — the ATA balance
    // delta after this handler is exactly -(protocol_cut + lp_cut)).
    let usdc_mint_ai = ctx.accounts.usdc_mint.to_account_info();
    let usdc_decimals = ctx.accounts.usdc_mint.decimals;
    let token_program_ai = ctx.accounts.token_program.to_account_info();
    let agent_ata_ai = ctx.accounts.agent_usdc_ata.to_account_info();
    let agent_signer_ai = ctx.accounts.agent.to_account_info();

    if protocol_cut > 0 {
        token::transfer_checked(
            CpiContext::new(
                token_program_ai.clone(),
                TransferChecked {
                    from: agent_ata_ai.clone(),
                    mint: usdc_mint_ai.clone(),
                    to: ctx.accounts.protocol_treasury_ata.to_account_info(),
                    authority: agent_signer_ai.clone(),
                },
            ),
            protocol_cut,
            usdc_decimals,
        )?;
    }
    if lp_cut > 0 {
        token::transfer_checked(
            CpiContext::new(
                token_program_ai,
                TransferChecked {
                    from: agent_ata_ai,
                    mint: usdc_mint_ai,
                    to: ctx.accounts.pool_usdc_vault.to_account_info(),
                    authority: agent_signer_ai,
                },
            ),
            lp_cut,
            usdc_decimals,
        )?;
    }

    let ledger = &mut ctx.accounts.issuance_ledger;
    ledger.live_principal = ledger
        .live_principal
        .checked_sub(principal)
        .ok_or(CredmeshError::MathOverflow)?;

    let pool = &mut ctx.accounts.pool;
    pool.deployed_amount = pool
        .deployed_amount
        .checked_sub(principal)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.total_assets = pool
        .total_assets
        .checked_add(lp_fee)
        .ok_or(CredmeshError::MathOverflow)?;
    pool.accrued_protocol_fees = pool
        .accrued_protocol_fees
        .checked_add(protocol_cut)
        .ok_or(CredmeshError::MathOverflow)?;

    let advance_key = ctx.accounts.advance.key();
    let agent_key = ctx.accounts.advance.agent;
    ctx.accounts.advance.state = AdvanceState::Settled;

    emit!(AdvanceSettled {
        pool: pool.key(),
        agent: agent_key,
        advance: advance_key,
        principal,
        lp_cut,
        protocol_cut,
        agent_net,
        late_days: late_days as u32,
    });

    Ok(())
}

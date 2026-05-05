use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::AdvanceSettled;
use crate::state::{
    Advance, AdvanceState, ConsumedPayment, Pool, ADVANCE_SEED, BPS_DENOMINATOR,
    CLAIM_WINDOW_SECONDS, CONSUMED_SEED, MAX_LATE_DAYS, POOL_SEED, PROTOCOL_FEE_BPS,
};

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
    /// Issue #8: seeds include advance.agent so the PDA derivation enforces
    /// the consumed↔advance binding via address (the explicit
    /// consumed.agent == advance.agent constraint stays as belt-and-suspenders).
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
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

pub fn handler(ctx: Context<ClaimAndSettle>, payment_amount: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    // Settlement window opens at `expires_at - CLAIM_WINDOW_SECONDS`.
    let claim_window_start = ctx
        .accounts
        .advance
        .expires_at
        .checked_sub(CLAIM_WINDOW_SECONDS)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(now >= claim_window_start, CredmeshError::NotSettleable);

    // Memo nonce binding: payment tx must include a memo with the
    // ConsumedPayment.nonce bytes. Defends the "same TransferChecked
    // re-wrapped in a different outer tx" replay vector flagged in audit.
    let consumed_nonce = ctx.accounts.consumed.nonce;
    credmesh_shared::ix_introspection::require_memo_nonce(
        &ctx.accounts.instructions_sysvar.to_account_info(),
        &consumed_nonce,
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
    require!(payment_amount >= total_owed, CredmeshError::WaterfallSumMismatch);

    // Compute three cuts. Fee + late penalty splits 15/85; principal
    // returns to LP vault in full.
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

    // Sum invariant check.
    require!(
        protocol_cut
            .checked_add(lp_cut)
            .and_then(|x| x.checked_add(agent_net))
            == Some(payment_amount),
        CredmeshError::WaterfallSumMismatch
    );

    // CPIs. Authority is the cranker (which == advance.agent in v1 per the
    // account-struct constraint).
    let cranker_ai = ctx.accounts.cranker.to_account_info();
    let payer_ata_ai = ctx.accounts.payer_usdc_ata.to_account_info();
    let token_program_ai = ctx.accounts.token_program.to_account_info();

    if protocol_cut > 0 {
        token::transfer(
            CpiContext::new(
                token_program_ai.clone(),
                Transfer {
                    from: payer_ata_ai.clone(),
                    to: ctx.accounts.protocol_treasury_ata.to_account_info(),
                    authority: cranker_ai.clone(),
                },
            ),
            protocol_cut,
        )?;
    }

    if lp_cut > 0 {
        token::transfer(
            CpiContext::new(
                token_program_ai.clone(),
                Transfer {
                    from: payer_ata_ai.clone(),
                    to: ctx.accounts.pool_usdc_vault.to_account_info(),
                    authority: cranker_ai.clone(),
                },
            ),
            lp_cut,
        )?;
    }

    // agent_net: the cranker IS the agent in v1; if payer_usdc_ata ==
    // agent_usdc_ata (typical), this is a self-transfer that we skip.
    // If the payer ATA is distinct from the agent's USDC ATA, transfer.
    if agent_net > 0
        && ctx.accounts.payer_usdc_ata.key() != ctx.accounts.agent_usdc_ata.key()
    {
        token::transfer(
            CpiContext::new(
                token_program_ai.clone(),
                Transfer {
                    from: payer_ata_ai.clone(),
                    to: ctx.accounts.agent_usdc_ata.to_account_info(),
                    authority: cranker_ai.clone(),
                },
            ),
            agent_net,
        )?;
    }

    // Update Pool: principal returns to vault, lp_fee accrues to LPs via
    // share-price increase, protocol_cut is tracked for skim.
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

    // The Anchor `close = agent` constraint on the Advance account
    // closes it at end-of-handler; rent goes to agent (neutralizes MEV).
    // We still set state for the (zero-data) post-close visibility.
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

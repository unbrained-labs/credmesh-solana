use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::token::{self, Mint, Revoke, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::AdvanceSettled;
use crate::state::{
    Advance, AdvanceState, ConsumedPayment, Pool, ADVANCE_SEED, BPS_DENOMINATOR,
    CLAIM_WINDOW_SECONDS, CONSUMED_SEED, MAX_LATE_DAYS, POOL_SEED, PROTOCOL_FEE_BPS,
};

#[derive(Accounts)]
pub struct ClaimAndSettle<'info> {
    /// Any signer; pays tx fee. Mode A (cranker == agent) signs as ATA
    /// owner; Mode B (any cranker) settles via the pool-PDA SPL delegate
    /// granted in `request_advance`. Substitution defenses are enforced
    /// by the per-account constraints below — none depend on cranker
    /// identity. See DECISIONS Q9 + `research/CONTRARIAN-permissionless-settle.md`.
    #[account(mut)]
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
    /// Source of settlement funds. Pinned to `token::authority = advance.agent`
    /// so the cranker (who may be any relayer in Mode B) cannot substitute an
    /// attacker-owned ATA. In Mode B the handler additionally requires
    /// `payer_usdc_ata.key() == agent_usdc_ata.key()`; in Mode A a distinct
    /// payer ATA owned by the agent is still allowed (e.g., agent settles from
    /// a treasury ATA different from the disbursement ATA).
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = advance.agent
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

    // Two-mode dispatch (see ClaimAndSettle doc): Mode A signs with the
    // cranker; Mode B signs with the pool PDA via the SPL delegate.
    let is_self_crank = ctx.accounts.cranker.key() == ctx.accounts.advance.agent;
    let payer_eq_agent =
        ctx.accounts.payer_usdc_ata.key() == ctx.accounts.agent_usdc_ata.key();

    let pool_pda_key = ctx.accounts.pool.key();
    let bump_arr = [ctx.accounts.pool.bump];
    let pool_seeds = ctx.accounts.pool.signer_seeds(&bump_arr);
    let pool_signer_seeds: &[&[&[u8]]] = &[&pool_seeds];

    if !is_self_crank {
        // Belt-and-suspenders: SPL Token enforces these too, but failing
        // here yields a typed error and skips the failed CPI's CU cost.
        require!(payer_eq_agent, CredmeshError::PayerMustBeAgentInPermissionless);
        let delegate = ctx
            .accounts
            .agent_usdc_ata
            .delegate
            .ok_or(error!(CredmeshError::DelegateNotApproved))?;
        require!(delegate == pool_pda_key, CredmeshError::DelegateNotApproved);
        require!(
            ctx.accounts.agent_usdc_ata.delegated_amount >= total_owed,
            CredmeshError::DelegateAmountInsufficient
        );
    }

    let token_program_ai = ctx.accounts.token_program.to_account_info();
    let payer_ata_ai = ctx.accounts.payer_usdc_ata.to_account_info();
    let authority_ai = if is_self_crank {
        ctx.accounts.cranker.to_account_info()
    } else {
        ctx.accounts.pool.to_account_info()
    };
    let signer_seeds: Option<&[&[&[u8]]]> = if is_self_crank {
        None
    } else {
        Some(pool_signer_seeds)
    };

    settle_transfer(
        token_program_ai.clone(),
        payer_ata_ai.clone(),
        ctx.accounts.protocol_treasury_ata.to_account_info(),
        authority_ai.clone(),
        signer_seeds,
        protocol_cut,
    )?;
    settle_transfer(
        token_program_ai.clone(),
        payer_ata_ai.clone(),
        ctx.accounts.pool_usdc_vault.to_account_info(),
        authority_ai.clone(),
        signer_seeds,
        lp_cut,
    )?;
    if !payer_eq_agent {
        settle_transfer(
            token_program_ai.clone(),
            payer_ata_ai.clone(),
            ctx.accounts.agent_usdc_ata.to_account_info(),
            authority_ai.clone(),
            signer_seeds,
            agent_net,
        )?;
    }

    // Mode A: zero out the SPL delegation that `request_advance` granted.
    // Mode B's transfers naturally decrement `delegated_amount` via the
    // delegate-authorized path, leaving only the bounded late-penalty
    // residual; off-chain Revoke happens when the agent is online. Mode A
    // signs as owner, so the program can CPI Revoke directly here.
    if is_self_crank {
        token::revoke(CpiContext::new(
            token_program_ai,
            Revoke {
                source: ctx.accounts.agent_usdc_ata.to_account_info(),
                authority: authority_ai,
            },
        ))?;
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
    let cranker_key = ctx.accounts.cranker.key();

    // The Anchor `close = agent` constraint on the Advance account
    // closes it at end-of-handler; rent goes to agent (neutralizes MEV
    // even when the cranker is a third-party relayer in Mode B).
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
        cranker: cranker_key,
    });

    Ok(())
}

/// One-shot SPL transfer that picks signer-vs-non-signer CPI shape based on
/// whether the caller passed PDA seeds. Used by `claim_and_settle`'s two-mode
/// waterfall: Mode A passes `None` (cranker-signed); Mode B passes the pool
/// PDA seeds (program-signed via the SPL `Approve` delegate). Free function
/// rather than a closure because `Transfer<'info>` is invariant over `'info`
/// and closures cannot name an outer lifetime they don't capture.
fn settle_transfer<'info>(
    token_program: AccountInfo<'info>,
    from: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    signer_seeds: Option<&[&[&[u8]]]>,
    amount: u64,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let accounts = Transfer { from, to, authority };
    match signer_seeds {
        Some(seeds) => token::transfer(
            CpiContext::new_with_signer(token_program, accounts, seeds),
            amount,
        ),
        None => token::transfer(CpiContext::new(token_program, accounts), amount),
    }
}

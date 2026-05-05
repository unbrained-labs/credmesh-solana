use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Approve, Mint, Token, TokenAccount, Transfer};

use crate::errors::CredmeshError;
use crate::events::{AdvanceIssued, SettlementDelegateApproved};
use crate::pricing::{
    compute_fee_amount, compute_late_penalty_per_day, compute_utilization_bps, credit_from_score_ema,
};
use crate::state::{
    Advance, AdvanceState, ConsumedPayment, Pool, ADVANCE_SEED, BPS_DENOMINATOR, CONSUMED_SEED,
    MAX_LATE_DAYS, MIN_ADVANCE_ATOMS, POOL_SEED,
};

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
    /// AgentReputation PDA owned by credmesh-reputation. Issue #4: Anchor's
    /// typed Account + seeds::program does the four-step verify (owner →
    /// address → discriminator → deserialize) declaratively. The handler
    /// reads `score_ema` / `default_count` directly off this typed account.
    #[account(
        seeds = [credmesh_shared::seeds::REPUTATION_SEED, agent_asset.key().as_ref()],
        seeds::program = credmesh_reputation::ID,
        bump,
    )]
    pub agent_reputation_pda: Account<'info, credmesh_reputation::AgentReputation>,
    /// Receivable PDA owned by credmesh-receivable-oracle, required iff
    /// `source_kind = Worker`. For Ed25519 / X402 paths the handler verifies
    /// via instruction-introspection instead of reading this account, so the
    /// caller passes `None` (encoded as a missing account); a typed `Account`
    /// without `Option` would fail the discriminator check there. Anchor
    /// runs the four-step verify on `Some` only (issue #4).
    /// Audit-MED #3 fix: the source_kind byte (`[0]` for Worker) is part of
    /// the receivable PDA seed, so the address Anchor verifies here can only
    /// resolve to a Worker-created receivable — an ed25519-attested receivable
    /// (kind=1/2) lives at a different address and cannot pass this gate.
    #[account(
        seeds = [credmesh_shared::seeds::RECEIVABLE_SEED, &[0u8], agent.key().as_ref(), receivable_id.as_ref()],
        seeds::program = credmesh_receivable_oracle::ID,
        bump,
    )]
    pub receivable_pda: Option<Account<'info, credmesh_receivable_oracle::Receivable>>,
    /// CHECK: Optional ExecutiveProfileV1 PDA (MPL Agent Tools) for delegate path.
    /// Pass `None` (Anchor encodes as missing) when agent.key() is the asset's owner.
    pub executive_profile: Option<UncheckedAccount<'info>>,
    /// CHECK: Optional ExecutionDelegateRecordV1 PDA. Pair with executive_profile.
    pub execution_delegate_record: Option<UncheckedAccount<'info>>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(
        init,
        payer = agent,
        space = 8 + Advance::INIT_SPACE,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-5: ConsumedPayment is permanent. Never closed.
    /// `init` failure is the replay-protection mechanism.
    /// Issue #8: agent.key() in seeds so cross-agent receivable_id reuse
    /// doesn't collide on a shared PDA address.
    #[account(
        init,
        payer = agent,
        space = 8 + ConsumedPayment::INIT_SPACE,
        seeds = [CONSUMED_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
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
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<RequestAdvance>,
    receivable_id: [u8; 32],
    amount: u64,
    source_kind: u8,
    nonce: [u8; 16],
) -> Result<()> {
    require!(amount >= MIN_ADVANCE_ATOMS, CredmeshError::AdvanceExceedsCap);
    let kind = credmesh_shared::SourceKind::from_u8(source_kind)
        .ok_or(CredmeshError::ReceivableStale)?;

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let slot = clock.slot;

    // (1) Verify agent identity (MPL Core asset + DelegateExecutionV1).
    // Per DECISIONS Q1; account-read only, no CPI.
    let exec_profile = ctx.accounts.executive_profile.as_ref().map(|a| a.as_ref());
    let exec_record = ctx.accounts.execution_delegate_record.as_ref().map(|a| a.as_ref());
    credmesh_shared::mpl_identity::verify_agent_signer(
        &ctx.accounts.agent.key(),
        &ctx.accounts.agent_asset.to_account_info(),
        &ctx.accounts.agent_identity.to_account_info(),
        exec_profile,
        exec_record,
    )
    .map_err(|e| match e {
        credmesh_shared::mpl_identity::IdentityError::NotACoreAsset
        | credmesh_shared::mpl_identity::IdentityError::AgentNotRegistered => {
            error!(CredmeshError::InvalidAgentAsset)
        }
        _ => error!(CredmeshError::AgentBindingMismatch),
    })?;

    // (2) Read AgentReputation cross-program. Issue #4: typed `Account` with
    // `seeds::program` runs Anchor's owner+address+discriminator+deserialize
    // verify automatically — no handler-side manual read.
    let reputation = &ctx.accounts.agent_reputation_pda;

    // (3) Read Receivable cross-program (Worker path) OR verify ed25519
    // signed receivable (Ed25519/X402 paths). Issue #4: same Anchor-typed
    // pattern; the Worker path resolves the `Option<Account>` to `Some`.
    let (receivable_amount, receivable_expires_at, source_signer) = match kind {
        credmesh_shared::SourceKind::Worker => {
            let receivable = ctx
                .accounts
                .receivable_pda
                .as_ref()
                .ok_or(error!(CredmeshError::ReceivablePdaMismatch))?;

            let staleness = slot.saturating_sub(receivable.last_updated_slot);
            require!(
                staleness <= credmesh_receivable_oracle::MAX_STALENESS_SLOTS,
                CredmeshError::ReceivableStale
            );
            (receivable.amount, receivable.expires_at, None)
        }
        credmesh_shared::SourceKind::Ed25519 | credmesh_shared::SourceKind::X402 => {
            let (signed_pubkey, signed_msg) =
                credmesh_shared::ix_introspection::verify_prev_ed25519(
                    &ctx.accounts.instructions_sysvar.to_account_info(),
                )
                .map_err(|e| match e {
                    credmesh_shared::ix_introspection::IxIntrospectionError::Ed25519OffsetMismatch => {
                        error!(CredmeshError::Ed25519MessageMismatch)
                    }
                    _ => error!(CredmeshError::Ed25519Missing),
                })?;

            require!(
                signed_msg.len() == credmesh_shared::ed25519_message::TOTAL_LEN,
                CredmeshError::Ed25519MessageMismatch
            );

            use credmesh_shared::ed25519_message as M;
            let msg_recv_id = &signed_msg[M::RECEIVABLE_ID_OFFSET
                ..M::RECEIVABLE_ID_OFFSET + M::RECEIVABLE_ID_LEN];
            let msg_agent =
                &signed_msg[M::AGENT_OFFSET..M::AGENT_OFFSET + M::AGENT_LEN];
            let mut amount_buf = [0u8; 8];
            amount_buf.copy_from_slice(
                &signed_msg[M::AMOUNT_OFFSET..M::AMOUNT_OFFSET + M::AMOUNT_LEN],
            );
            let msg_amount = u64::from_le_bytes(amount_buf);
            let mut expires_buf = [0u8; 8];
            expires_buf.copy_from_slice(
                &signed_msg[M::EXPIRES_AT_OFFSET..M::EXPIRES_AT_OFFSET + M::EXPIRES_AT_LEN],
            );
            let msg_expires_at = i64::from_le_bytes(expires_buf);
            let msg_nonce = &signed_msg[M::NONCE_OFFSET..M::NONCE_OFFSET + M::NONCE_LEN];

            require!(
                msg_recv_id == receivable_id.as_ref(),
                CredmeshError::Ed25519MessageMismatch
            );
            require!(
                msg_agent == ctx.accounts.agent_asset.key().as_ref(),
                CredmeshError::Ed25519MessageMismatch
            );
            require!(msg_nonce == nonce.as_ref(), CredmeshError::Ed25519MessageMismatch);

            (msg_amount, msg_expires_at, Some(signed_pubkey))
        }
    };

    require!(receivable_expires_at > now, CredmeshError::ReceivableExpired);

    // (4) Cap checks: amount <= min(receivable * pct_bps / 10000, abs_cap, credit_from_score).
    let pool = &ctx.accounts.pool;
    let pct_cap = (receivable_amount as u128)
        .checked_mul(pool.max_advance_pct_bps as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let pct_cap = u64::try_from(pct_cap).map_err(|_| error!(CredmeshError::MathOverflow))?;
    require!(amount <= pct_cap, CredmeshError::AdvanceExceedsCap);
    require!(amount <= pool.max_advance_abs, CredmeshError::AdvanceExceedsCap);

    let credit_from_score = credit_from_score_ema(reputation.score_ema, &pool.fee_curve)?;
    require!(amount <= credit_from_score, CredmeshError::AdvanceExceedsCredit);

    // (5) Compute fee from the on-chain curve. The off-chain server's
    // pricing.ts must produce the same number; tests assert the equality.
    let duration_seconds = receivable_expires_at.saturating_sub(now).max(0) as u64;
    let utilization = compute_utilization_bps(pool)?;
    let fee_owed = compute_fee_amount(
        amount,
        duration_seconds,
        utilization,
        reputation.default_count,
        &pool.fee_curve,
    )?;

    // (6) Transfer USDC vault → agent. PDA-signed.
    let bump_arr = [pool.bump];
    let pool_seeds = pool.signer_seeds(&bump_arr);
    let signer_seeds: &[&[&[u8]]] = &[&pool_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.pool_usdc_vault.to_account_info(),
                to: ctx.accounts.agent_usdc_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    // (6b) Grant pool PDA delegate authority on agent's USDC ATA so Mode B
    // crankers can settle without the agent's key. Approval cap covers
    // worst-case settlement (principal + fee + max-late). Bundling into
    // request_advance avoids a UX race where the agent issues but forgets
    // to approve, leaving the advance unsettleable until liquidation.
    // Lingering residual after settlement is bounded by the late-penalty
    // curve and revocable by the agent. See DECISIONS Q9.
    let late_penalty_per_day = compute_late_penalty_per_day(amount, &pool.fee_curve)?;
    let max_late_penalty = (MAX_LATE_DAYS as u64)
        .checked_mul(late_penalty_per_day)
        .ok_or(CredmeshError::MathOverflow)?;
    let settle_delegate_amount = amount
        .checked_add(fee_owed)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_add(max_late_penalty)
        .ok_or(CredmeshError::MathOverflow)?;

    token::approve(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Approve {
                to: ctx.accounts.agent_usdc_ata.to_account_info(),
                delegate: ctx.accounts.pool.to_account_info(),
                authority: ctx.accounts.agent.to_account_info(),
            },
        ),
        settle_delegate_amount,
    )?;

    // (7) Init Advance + ConsumedPayment (Anchor handled via `init`).
    let advance = &mut ctx.accounts.advance;
    advance.bump = ctx.bumps.advance;
    advance.agent = ctx.accounts.agent.key();
    advance.receivable_id = receivable_id;
    advance.principal = amount;
    advance.fee_owed = fee_owed;
    advance.late_penalty_per_day = late_penalty_per_day;
    advance.issued_at = now;
    advance.expires_at = receivable_expires_at;
    advance.source_kind = source_kind;
    advance.source_signer = source_signer;
    advance.state = AdvanceState::Issued;
    let advance_key = advance.key();

    let consumed = &mut ctx.accounts.consumed;
    consumed.bump = ctx.bumps.consumed;
    consumed.nonce = nonce;
    consumed.agent = ctx.accounts.agent.key();
    consumed.created_at = now;

    // (8) Update Pool deployed_amount.
    let pool = &mut ctx.accounts.pool;
    pool.deployed_amount = pool
        .deployed_amount
        .checked_add(amount)
        .ok_or(CredmeshError::MathOverflow)?;

    // Post-state invariant: deployed never exceeds total_assets.
    require!(
        pool.deployed_amount <= pool.total_assets,
        CredmeshError::InsufficientIdleLiquidity
    );

    let pool_key = pool.key();
    emit!(AdvanceIssued {
        pool: pool_key,
        agent: ctx.accounts.agent.key(),
        advance: advance_key,
        principal: amount,
        fee_owed,
        expires_at: receivable_expires_at,
        source_kind,
    });
    emit!(SettlementDelegateApproved {
        pool: pool_key,
        agent: ctx.accounts.agent.key(),
        advance: advance_key,
        agent_usdc_ata: ctx.accounts.agent_usdc_ata.key(),
        approved_amount: settle_delegate_amount,
    });

    Ok(())
}

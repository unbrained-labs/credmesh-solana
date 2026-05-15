use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::errors::CredmeshError;
use crate::events::AdvanceIssued;
use crate::pricing::{compute_fee_amount, compute_late_penalty_per_day, compute_utilization_bps};
use crate::state::{
    Advance, AdvanceState, AgentIssuanceLedger, ConsumedPayment, Pool, ADVANCE_SEED,
    AGENT_WINDOW_SECONDS, CONSUMED_SEED, ISSUANCE_LEDGER_SEED, MIN_ADVANCE_ATOMS, POOL_SEED,
};

#[derive(Accounts)]
#[instruction(receivable_id: [u8; 32])]
pub struct RequestAdvance<'info> {
    /// Agent's primary signing key. Sole identity surface on Solana —
    /// agent identity + reputation live on EVM, attested by the bridge.
    #[account(mut)]
    pub agent: Signer<'info>,

    /// AllowedSigner PDA owned by credmesh-attestor-registry. The signer
    /// pubkey stored here is the bridge's ed25519 public key. Anchor's
    /// typed Account + seeds::program runs the four-step verify (owner,
    /// address, discriminator, deserialize). The handler additionally
    /// confirms (a) the prior ed25519 ix's signed-by pubkey equals
    /// `allowed_signer.signer`, (b) `allowed_signer.kind ==
    /// AttestorKind::CreditBridge`.
    #[account(
        seeds = [credmesh_shared::seeds::ALLOWED_SIGNER_SEED, allowed_signer.signer.as_ref()],
        seeds::program = credmesh_attestor_registry::ID,
        bump = allowed_signer.bump,
    )]
    pub allowed_signer: Account<'info, credmesh_attestor_registry::AllowedSigner>,

    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,

    /// Per-advance PDA. `init` failure on the seed `(pool, agent,
    /// receivable_id)` is the replay defense: a second tx with the same
    /// receivable_id collides on the address and fails to init.
    #[account(
        init,
        payer = agent,
        space = 8 + Advance::INIT_SPACE,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub advance: Account<'info, Advance>,

    /// AUDIT P0-5: ConsumedPayment is permanent — never closed. The
    /// `init` failure on a second request_advance with the same
    /// receivable_id is the replay-protection mechanism.
    #[account(
        init,
        payer = agent,
        space = 8 + ConsumedPayment::INIT_SPACE,
        seeds = [CONSUMED_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub consumed: Account<'info, ConsumedPayment>,

    /// Per-agent rolling-window issuance ledger. `init_if_needed` is
    /// safe here — this is NOT a replay-protection PDA (replay protection
    /// is `consumed` above; AUDIT P0-5 only forbids init_if_needed for
    /// replay PDAs). Initialized once per (pool, agent) pair, then
    /// updated in-place by every subsequent `request_advance`.
    #[account(
        init_if_needed,
        payer = agent,
        space = 8 + AgentIssuanceLedger::INIT_SPACE,
        seeds = [ISSUANCE_LEDGER_SEED, pool.key().as_ref(), agent.key().as_ref()],
        bump
    )]
    pub issuance_ledger: Account<'info, AgentIssuanceLedger>,

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

    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions
    /// account. Handler reads the prior ed25519 ix to extract the bridge's
    /// signed credit attestation.
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
    nonce: [u8; 16],
) -> Result<()> {
    require!(
        amount >= MIN_ADVANCE_ATOMS,
        CredmeshError::AdvanceExceedsCap
    );

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;

    // (1) Verify the bridge's ed25519 credit attestation. The bridge reads
    // EVM ReputationRegistry + ReputationCreditOracle, computes the agent's
    // current credit_limit + outstanding, signs a 128-byte canonical
    // message. We extract + verify here.
    let (signed_pubkey, signed_msg) = credmesh_shared::ix_introspection::verify_prev_ed25519(
        &ctx.accounts.instructions_sysvar.to_account_info(),
    )
    .map_err(|e| match e {
        credmesh_shared::ix_introspection::IxIntrospectionError::Ed25519OffsetMismatch => {
            error!(CredmeshError::Ed25519MessageMismatch)
        }
        _ => error!(CredmeshError::Ed25519Missing),
    })?;

    require!(
        signed_msg.len() == credmesh_shared::ed25519_credit_message::TOTAL_LEN,
        CredmeshError::Ed25519MessageMismatch
    );
    require_keys_eq!(
        signed_pubkey,
        ctx.accounts.allowed_signer.signer,
        CredmeshError::Ed25519SignerUnknown
    );
    require!(
        ctx.accounts.allowed_signer.kind == credmesh_shared::AttestorKind::CreditBridge.as_u8(),
        CredmeshError::Ed25519SignerUnknown
    );

    // (2) Decode the canonical 128-byte attestation against the layout in
    // credmesh_shared::ed25519_credit_message.
    use credmesh_shared::ed25519_credit_message as M;

    let msg_agent = pubkey_at(&signed_msg, M::AGENT_OFFSET);
    require_keys_eq!(
        msg_agent,
        ctx.accounts.agent.key(),
        CredmeshError::Ed25519MessageMismatch
    );

    let msg_pool = pubkey_at(&signed_msg, M::POOL_OFFSET);
    require_keys_eq!(
        msg_pool,
        ctx.accounts.pool.key(),
        CredmeshError::Ed25519MessageMismatch
    );

    let attested_credit_limit = u64_le(&signed_msg, M::CREDIT_LIMIT_OFFSET);
    let attested_outstanding = u64_le(&signed_msg, M::OUTSTANDING_OFFSET);
    let expires_at = i64_le(&signed_msg, M::EXPIRES_AT_OFFSET);
    let attested_at = i64_le(&signed_msg, M::ATTESTED_AT_OFFSET);
    let msg_nonce = &signed_msg[M::NONCE_OFFSET..M::NONCE_OFFSET + M::NONCE_LEN];
    let msg_chain_id = u64_le(&signed_msg, M::CHAIN_ID_OFFSET);
    let version = u64_le(&signed_msg, M::VERSION_OFFSET);

    require!(version == M::VERSION, CredmeshError::Ed25519MessageMismatch);
    require!(
        msg_nonce == nonce.as_ref(),
        CredmeshError::Ed25519MessageMismatch
    );
    // Cross-cluster replay defense: a devnet attestation MUST NOT verify
    // against a mainnet pool, and vice versa, even when the same bridge
    // signer is whitelisted on both. pool.chain_id is set at init_pool.
    require!(
        msg_chain_id == ctx.accounts.pool.chain_id,
        CredmeshError::InvalidChainId
    );

    // (3) Freshness checks — short-TTL bounds the blast radius of a
    // compromised bridge signer key.
    require!(
        attested_at <= now && (now - attested_at) <= M::MAX_ATTESTATION_AGE_SECONDS,
        CredmeshError::ReceivableStale
    );
    require!(expires_at > now, CredmeshError::ReceivableExpired);

    let pool = &ctx.accounts.pool;

    // (4) Roll the per-agent issuance ledger forward and enforce the
    // window cap. Bounds the principal a single agent can pull in
    // `AGENT_WINDOW_SECONDS` — bridge-key-compromise blast-radius bound.
    let ledger = &mut ctx.accounts.issuance_ledger;
    if ledger.bump == 0 {
        // Freshly init'd by Anchor — fields are zero-initialized. Pin the
        // PDA's relational fields once.
        ledger.bump = ctx.bumps.issuance_ledger;
        ledger.agent = ctx.accounts.agent.key();
        ledger.pool = pool.key();
        ledger.window_start = now;
        ledger.issued_in_window = 0;
    }
    if now.saturating_sub(ledger.window_start) >= AGENT_WINDOW_SECONDS {
        ledger.window_start = now;
        ledger.issued_in_window = 0;
    }
    let new_issued = ledger
        .issued_in_window
        .checked_add(amount)
        .ok_or(CredmeshError::MathOverflow)?;
    if pool.agent_window_cap > 0 {
        require!(
            new_issued <= pool.agent_window_cap,
            CredmeshError::AgentWindowCapExceeded
        );
    }
    ledger.issued_in_window = new_issued;

    // (4b) Underwrite. The attested credit_limit is the EVM-derived cap.
    // attested_outstanding is EVM-lane outstanding only; this Solana lane
    // adds live_principal on-chain so replayed Solana exposure is never
    // counted twice.
    let combined_outstanding = attested_outstanding
        .checked_add(ledger.live_principal)
        .ok_or(CredmeshError::MathOverflow)?;
    let available_credit = attested_credit_limit.saturating_sub(combined_outstanding);
    require!(
        amount <= available_credit,
        CredmeshError::AdvanceExceedsCredit
    );
    require!(
        amount <= pool.max_advance_abs,
        CredmeshError::AdvanceExceedsCap
    );
    ledger.live_principal = ledger
        .live_principal
        .checked_add(amount)
        .ok_or(CredmeshError::MathOverflow)?;

    // (5) Fee computation against the pool's curve. duration_seconds
    // = (expires_at - now), used as the loan tenor for fee math.
    let duration_seconds = expires_at.saturating_sub(now).max(0) as u64;
    let utilization = compute_utilization_bps(pool)?;
    let fee_owed = compute_fee_amount(
        amount,
        duration_seconds,
        utilization,
        0, // default_count: per-agent default history is on EVM; the
        // bridge could fold a defaults proxy into trust_score later.
        &pool.fee_curve,
    )?;
    let late_penalty_per_day = compute_late_penalty_per_day(amount, &pool.fee_curve)?;

    // (6) Disburse: vault → agent's USDC ATA, PDA-signed.
    let bump_arr = [pool.bump];
    let pool_seeds = pool.signer_seeds(&bump_arr);
    let signer_seeds: &[&[&[u8]]] = &[&pool_seeds];

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.pool_usdc_vault.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.agent_usdc_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.usdc_mint.decimals,
    )?;

    // (7) Init the Advance + ConsumedPayment PDAs.
    let advance = &mut ctx.accounts.advance;
    advance.bump = ctx.bumps.advance;
    advance.agent = ctx.accounts.agent.key();
    advance.receivable_id = receivable_id;
    advance.principal = amount;
    advance.fee_owed = fee_owed;
    advance.late_penalty_per_day = late_penalty_per_day;
    advance.issued_at = now;
    advance.expires_at = expires_at;
    advance.attestor = signed_pubkey;
    advance.state = AdvanceState::Issued;
    let advance_key = advance.key();

    let consumed = &mut ctx.accounts.consumed;
    consumed.bump = ctx.bumps.consumed;
    consumed.nonce = nonce;
    consumed.agent = ctx.accounts.agent.key();
    consumed.created_at = now;

    // (8) Update pool exposure.
    let pool = &mut ctx.accounts.pool;
    pool.deployed_amount = pool
        .deployed_amount
        .checked_add(amount)
        .ok_or(CredmeshError::MathOverflow)?;
    require!(
        pool.deployed_amount <= pool.total_assets,
        CredmeshError::InsufficientIdleLiquidity
    );

    emit!(AdvanceIssued {
        pool: pool.key(),
        agent: ctx.accounts.agent.key(),
        advance: advance_key,
        principal: amount,
        fee_owed,
        expires_at,
        attestor: signed_pubkey,
    });

    Ok(())
}

fn pubkey_at(msg: &[u8], offset: usize) -> Pubkey {
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&msg[offset..offset + 32]);
    Pubkey::new_from_array(buf)
}

fn u64_le(msg: &[u8], offset: usize) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&msg[offset..offset + 8]);
    u64::from_le_bytes(buf)
}

fn i64_le(msg: &[u8], offset: usize) -> i64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&msg[offset..offset + 8]);
    i64::from_le_bytes(buf)
}

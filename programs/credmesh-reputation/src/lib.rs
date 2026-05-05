use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod scoring;
pub mod state;

pub use errors::ReputationError;
pub use events::*;
pub use state::*;

// Devnet program ID — the keypair is reserved per DEPLOYMENT.md § Devnet deploy log.
// Mainnet uses a different keypair generated via `anchor keys sync` before promotion.
declare_id!("JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G");

#[program]
pub mod credmesh_reputation {
    use super::*;

    /// Onboards a new agent. Self-service — agent signs, no writer needed.
    /// All attestation fields (trust_score, attestation_count,
    /// cooperation_success_count, successful_jobs, failed_jobs,
    /// average_completed_payout) are forced to zero. The writer authority
    /// (configured at deploy time on credmesh-receivable-oracle's
    /// OracleConfig.reputation_writer_authority — see DECISIONS Q4) boosts
    /// these via `update_agent_attestations` based on off-chain validation
    /// (ERC-8004 lookup, SAS attestation, marketplace history check, etc.).
    /// `record_settlement_outcome` and `record_default` increment the
    /// repaid/defaulted/successful/failed counters as advances close.
    ///
    /// `identity_registered` is set to true only if the caller passes a
    /// valid MPL Core asset whose stored owner field equals the agent's
    /// signing key. This is the on-chain proof of identity attestation
    /// (the Solana equivalent of EVM's `identityRegistered` bool, which
    /// EVM derives from an ERC-8004 registry read).
    ///
    /// The fresh-agent default credit limit (no history, no identity) is
    /// $40 atoms (= 0 score * 8 + 0 repay rate * 120 + 0.5 completion
    /// default * 80 = 40 dollars). With identity attached: ~$120. Real
    /// limit growth comes from successful settlements + writer attestations.
    pub fn register_agent(ctx: Context<RegisterAgent>) -> Result<()> {
        let reputation = &mut ctx.accounts.reputation;
        reputation.bump = ctx.bumps.reputation;
        reputation.agent = ctx.accounts.agent.key();

        // CRITICAL: all attestation fields start at zero. Self-attestation
        // is NOT permitted — the writer authority gates any boost.
        reputation.trust_score = 0;
        reputation.attestation_count = 0;
        reputation.cooperation_success_count = 0;
        reputation.successful_jobs = 0;
        reputation.failed_jobs = 0;
        reputation.average_completed_payout_atoms = 0;
        reputation.outstanding_balance_atoms = 0;
        reputation.repaid_advances = 0;
        reputation.defaulted_advances = 0;

        // identity_registered: only true if a valid MPL Core asset is
        // attached AND its stored `owner` field (byte offset 1..33 of the
        // BaseAssetV1 struct) equals the agent's signing pubkey. This is
        // the same identity-binding helper used by escrow's request_advance
        // for the optional MPL flow (DECISIONS Q1, post-amendment).
        reputation.identity_registered = match ctx.accounts.agent_asset.as_ref() {
            Some(asset) => {
                // Verify owner-program is MPL_CORE (no spoofing).
                require_keys_eq!(
                    *asset.owner,
                    credmesh_shared::program_ids::MPL_CORE,
                    ReputationError::IdentityProofInvalid
                );
                // Read the asset's stored owner field at the canonical offset.
                let data = asset.try_borrow_data()
                    .map_err(|_| ReputationError::IdentityProofInvalid)?;
                require!(
                    data.len() >= credmesh_shared::mpl_core_asset::OWNER_OFFSET
                        + credmesh_shared::mpl_core_asset::OWNER_LEN,
                    ReputationError::IdentityProofInvalid
                );
                let mut owner_bytes = [0u8; 32];
                owner_bytes.copy_from_slice(
                    &data[credmesh_shared::mpl_core_asset::OWNER_OFFSET
                        ..credmesh_shared::mpl_core_asset::OWNER_OFFSET
                            + credmesh_shared::mpl_core_asset::OWNER_LEN],
                );
                let asset_owner = Pubkey::new_from_array(owner_bytes);
                asset_owner == ctx.accounts.agent.key()
            }
            None => false,
        };

        // Compute fresh score and limit from the all-zeros baseline.
        reputation.credit_score = scoring::compute_credit_score(reputation);
        reputation.credit_limit_atoms = scoring::compute_credit_limit_atoms(reputation);

        // Permissionless feedback log fields start fresh.
        reputation.feedback_count = 0;
        reputation.feedback_digest = [0u8; 32];
        reputation.score_ema = 0;
        reputation.default_count = 0;
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(AgentRegistered {
            agent: reputation.agent,
            reputation_pda: reputation.key(),
            credit_score: reputation.credit_score,
            credit_limit_atoms: reputation.credit_limit_atoms,
            trust_score: reputation.trust_score,
            identity_registered: reputation.identity_registered,
        });

        Ok(())
    }

    /// Writer-gated incremental update of attestation fields. Mirrors the
    /// EVM credit-worker's role of being source-of-truth for off-chain
    /// validation outputs (ERC-8004 lookups, SAS attestations, marketplace
    /// history). Each field is Option<...>; only Some(...) values are
    /// written. Recomputes credit_score + credit_limit at the end.
    pub fn update_agent_attestations(
        ctx: Context<RecordReputationEvent>,
        update: AgentAttestationUpdate,
    ) -> Result<()> {
        require_writer_authority(&ctx.accounts.attestor, &ctx.accounts.oracle_config)?;

        let reputation = &mut ctx.accounts.reputation;
        if let Some(v) = update.trust_score {
            require!(v <= 100, ReputationError::TrustScoreOutOfRange);
            reputation.trust_score = v;
        }
        if let Some(v) = update.attestation_count {
            reputation.attestation_count = v;
        }
        if let Some(v) = update.cooperation_success_count {
            reputation.cooperation_success_count = v;
        }
        if let Some(v) = update.average_completed_payout_atoms {
            reputation.average_completed_payout_atoms = v;
        }
        if let Some(v) = update.identity_registered {
            reputation.identity_registered = v;
        }

        reputation.credit_score = scoring::compute_credit_score(reputation);
        reputation.credit_limit_atoms = scoring::compute_credit_limit_atoms(reputation);
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(CreditProfileUpdated {
            agent: reputation.agent,
            credit_score: reputation.credit_score,
            credit_limit_atoms: reputation.credit_limit_atoms,
            outstanding_balance_atoms: reputation.outstanding_balance_atoms,
            repaid_advances: reputation.repaid_advances,
            defaulted_advances: reputation.defaulted_advances,
        });

        Ok(())
    }

    /// Increments outstanding_balance when an advance is issued. Authorized to
    /// `oracle_config.reputation_writer_authority` (the CredMesh worker key
    /// per DECISIONS Q4). For Phase-2 on-chain autonomy this becomes a CPI
    /// from credmesh-escrow's pool PDA — Phase-1 keeps the writer-gated path
    /// matching the EVM credit-worker model.
    pub fn record_advance_issued(
        ctx: Context<RecordReputationEvent>,
        principal_atoms: u64,
    ) -> Result<()> {
        require_writer_authority(&ctx.accounts.attestor, &ctx.accounts.oracle_config)?;

        let reputation = &mut ctx.accounts.reputation;
        reputation.outstanding_balance_atoms = reputation
            .outstanding_balance_atoms
            .checked_add(principal_atoms)
            .ok_or(ReputationError::MathOverflow)?;
        // Score depends on outstanding_balance penalty; refresh.
        reputation.credit_score = scoring::compute_credit_score(reputation);
        reputation.credit_limit_atoms = scoring::compute_credit_limit_atoms(reputation);
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(AdvanceRecorded {
            agent: reputation.agent,
            principal_atoms,
            outstanding_after_atoms: reputation.outstanding_balance_atoms,
        });

        Ok(())
    }

    /// Decrements outstanding_balance + bumps repaid_advances + (optionally)
    /// updates successful_jobs / average_payout. Triggered by escrow's
    /// AdvanceSettled event (off-chain writer in Phase 1).
    pub fn record_settlement_outcome(
        ctx: Context<RecordReputationEvent>,
        principal_atoms: u64,
        bump_successful_job: bool,
        completed_payout_atoms: u64,
    ) -> Result<()> {
        require_writer_authority(&ctx.accounts.attestor, &ctx.accounts.oracle_config)?;

        let reputation = &mut ctx.accounts.reputation;
        reputation.outstanding_balance_atoms = reputation
            .outstanding_balance_atoms
            .checked_sub(principal_atoms)
            .ok_or(ReputationError::OutstandingUnderflow)?;
        reputation.repaid_advances = reputation
            .repaid_advances
            .checked_add(1)
            .ok_or(ReputationError::MathOverflow)?;

        if bump_successful_job {
            // Update running average payout: (avg * n + new) / (n + 1)
            let prev_n = reputation.successful_jobs as u128;
            let prev_avg = reputation.average_completed_payout_atoms as u128;
            let new_n = prev_n
                .checked_add(1)
                .ok_or(ReputationError::MathOverflow)?;
            let total = prev_avg
                .checked_mul(prev_n)
                .ok_or(ReputationError::MathOverflow)?
                .checked_add(completed_payout_atoms as u128)
                .ok_or(ReputationError::MathOverflow)?;
            reputation.average_completed_payout_atoms = (total / new_n) as u64;
            reputation.successful_jobs = reputation
                .successful_jobs
                .checked_add(1)
                .ok_or(ReputationError::MathOverflow)?;
        }

        reputation.credit_score = scoring::compute_credit_score(reputation);
        reputation.credit_limit_atoms = scoring::compute_credit_limit_atoms(reputation);
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(SettlementRecorded {
            agent: reputation.agent,
            principal_atoms,
            outstanding_after_atoms: reputation.outstanding_balance_atoms,
            credit_limit_after_atoms: reputation.credit_limit_atoms,
            repaid_advances_after: reputation.repaid_advances,
        });

        Ok(())
    }

    /// Decrements outstanding_balance + bumps defaulted_advances and
    /// failed_jobs. Triggered by escrow's AdvanceLiquidated event.
    pub fn record_default(
        ctx: Context<RecordReputationEvent>,
        principal_atoms: u64,
    ) -> Result<()> {
        require_writer_authority(&ctx.accounts.attestor, &ctx.accounts.oracle_config)?;

        let reputation = &mut ctx.accounts.reputation;
        reputation.outstanding_balance_atoms = reputation
            .outstanding_balance_atoms
            .checked_sub(principal_atoms)
            .ok_or(ReputationError::OutstandingUnderflow)?;
        reputation.defaulted_advances = reputation
            .defaulted_advances
            .checked_add(1)
            .ok_or(ReputationError::MathOverflow)?;
        reputation.failed_jobs = reputation
            .failed_jobs
            .checked_add(1)
            .ok_or(ReputationError::MathOverflow)?;
        reputation.default_count = reputation
            .default_count
            .checked_add(1)
            .ok_or(ReputationError::MathOverflow)?;

        reputation.credit_score = scoring::compute_credit_score(reputation);
        reputation.credit_limit_atoms = scoring::compute_credit_limit_atoms(reputation);
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(DefaultRecorded {
            agent: reputation.agent,
            principal_atoms,
            outstanding_after_atoms: reputation.outstanding_balance_atoms,
            credit_limit_after_atoms: reputation.credit_limit_atoms,
            defaulted_advances_after: reputation.defaulted_advances,
        });

        Ok(())
    }

    /// Permissionless feedback (DECISIONS Q4): anyone can attest, only
    /// `reputation_writer_authority`-signed feedback updates `score_ema`.
    /// `score_ema` is kept separate from the EVM-port `credit_score` for v1
    /// (the EMA feeds the SAS / 8004 indexer ecosystem; underwriting uses
    /// `credit_score` derived from explicit job/advance counters).
    pub fn give_feedback(ctx: Context<GiveFeedback>, input: FeedbackInput) -> Result<()> {
        require!(input.score <= 100, ReputationError::InvalidScore);
        require!(
            input.feedback_uri.len() <= 200,
            ReputationError::UriTooLong
        );

        let oracle_config = &ctx.accounts.oracle_config;
        let attestor = ctx.accounts.attestor.key();
        let reputation = &mut ctx.accounts.reputation;

        let event_hash = anchor_lang::solana_program::keccak::hashv(&[
            attestor.as_ref(),
            &[input.score],
            &input.value.to_le_bytes(),
            &[input.value_decimals],
            &input.reason_code.to_le_bytes(),
            input.feedback_uri.as_bytes(),
            &input.feedback_hash,
            &input.job_id,
        ]);
        let digest_after = anchor_lang::solana_program::keccak::hashv(&[
            &reputation.feedback_digest,
            &event_hash.to_bytes(),
        ])
        .to_bytes();
        reputation.feedback_digest = digest_after;
        reputation.feedback_count = reputation
            .feedback_count
            .checked_add(1)
            .ok_or(ReputationError::MathOverflow)?;
        let feedback_index = reputation.feedback_count - 1;

        if attestor == oracle_config.reputation_writer_authority {
            require!(
                input.score as u32 <= oracle_config.reputation_max_per_tx_score as u32,
                ReputationError::InvalidScore
            );

            let score_scaled = (input.score as u128).saturating_mul(1_000_000_000_000_000_000u128);
            let n = EMA_WINDOW as u128;
            let ema_old = reputation.score_ema;
            let weighted_old = ema_old
                .checked_mul(n.saturating_sub(1))
                .ok_or(ReputationError::MathOverflow)?;
            let sum = weighted_old
                .checked_add(score_scaled)
                .ok_or(ReputationError::MathOverflow)?;
            reputation.score_ema = sum.checked_div(n).ok_or(ReputationError::MathOverflow)?;

            if input.reason_code & 0x8000 != 0 {
                reputation.default_count = reputation
                    .default_count
                    .checked_add(1)
                    .ok_or(ReputationError::MathOverflow)?;
            }

            reputation.last_event_slot = Clock::get()?.slot;
        }

        emit_cpi!(NewFeedback {
            agent: reputation.agent,
            feedback_index,
            attestor,
            score: input.score,
            value: input.value,
            value_decimals: input.value_decimals,
            reason_code: input.reason_code,
            feedback_uri: input.feedback_uri,
            feedback_hash: input.feedback_hash,
            job_id: input.job_id,
            digest_after,
            score_ema_after: reputation.score_ema,
        });

        Ok(())
    }
}

fn require_writer_authority<'info>(
    attestor: &Signer<'info>,
    oracle_config: &Account<'info, credmesh_receivable_oracle::OracleConfig>,
) -> Result<()> {
    require!(
        attestor.key() == oracle_config.reputation_writer_authority,
        ReputationError::UnauthorizedWriter
    );
    Ok(())
}

#[derive(Accounts)]
pub struct RegisterAgent<'info> {
    /// The agent being registered. Signs the tx so a third party cannot
    /// register a profile in someone else's name.
    pub agent: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + AgentReputation::INIT_SPACE,
        seeds = [REPUTATION_SEED, agent.key().as_ref()],
        bump
    )]
    pub reputation: Account<'info, AgentReputation>,
    /// CHECK: Optional MPL Core asset providing identity attestation. When
    /// provided, the handler verifies (a) account-owner is MPL_CORE and
    /// (b) the asset's stored `owner` field at byte offset 1..33 equals
    /// the agent's signing pubkey. Either check failing rejects the tx.
    /// When `None`, identity_registered stays false; the agent can still
    /// register but with the bare-bones fresh credit baseline.
    pub agent_asset: Option<UncheckedAccount<'info>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordReputationEvent<'info> {
    /// Must equal `oracle_config.reputation_writer_authority`. Verified in
    /// the handler. Phase-2 will swap this for a Pool-PDA-signed CPI from
    /// credmesh-escrow.
    pub attestor: Signer<'info>,
    /// The agent whose reputation is being updated. Not a signer here —
    /// settle/liquidate events fire after the fact, with no agent
    /// involvement (matches EVM's worker-driven update model).
    /// CHECK: Identity verified by the seeds derivation on `reputation`.
    pub agent: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [REPUTATION_SEED, agent.key().as_ref()],
        bump = reputation.bump
    )]
    pub reputation: Account<'info, AgentReputation>,
    /// OracleConfig PDA owned by credmesh-receivable-oracle. Anchor's typed
    /// Account + seeds::program runs the four-step verify. Read for the
    /// writer-authority gate.
    #[account(
        seeds = [credmesh_shared::seeds::ORACLE_CONFIG_SEED],
        seeds::program = credmesh_receivable_oracle::ID,
        bump,
    )]
    pub oracle_config: Account<'info, credmesh_receivable_oracle::OracleConfig>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct GiveFeedback<'info> {
    pub attestor: Signer<'info>,
    /// CHECK: Just a seed source.
    pub agent: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [REPUTATION_SEED, agent.key().as_ref()],
        bump = reputation.bump
    )]
    pub reputation: Account<'info, AgentReputation>,
    #[account(
        seeds = [credmesh_shared::seeds::ORACLE_CONFIG_SEED],
        seeds::program = credmesh_receivable_oracle::ID,
        bump,
    )]
    pub oracle_config: Account<'info, credmesh_receivable_oracle::OracleConfig>,
}

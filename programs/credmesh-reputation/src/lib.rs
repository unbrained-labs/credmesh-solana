use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod state;

pub use errors::ReputationError;
pub use events::*;
pub use state::*;

// PLACEHOLDER — replace before deploy via `anchor keys sync`. See DEPLOYMENT.md.
declare_id!("11111111111111111111111111111113");

#[program]
pub mod credmesh_reputation {
    use super::*;

    pub fn init_reputation(ctx: Context<InitReputation>) -> Result<()> {
        let reputation = &mut ctx.accounts.reputation;
        reputation.bump = ctx.bumps.reputation;
        reputation.agent_asset = ctx.accounts.agent_asset.key();
        reputation.feedback_count = 0;
        reputation.feedback_digest = [0u8; 32];
        reputation.score_ema = 0;
        reputation.default_count = 0;
        reputation.last_event_slot = Clock::get()?.slot;

        emit!(ReputationInitialized {
            agent_asset: reputation.agent_asset,
            reputation_pda: reputation.key(),
        });

        Ok(())
    }

    pub fn give_feedback(ctx: Context<GiveFeedback>, input: FeedbackInput) -> Result<()> {
        require!(input.score <= 100, ReputationError::InvalidScore);
        require!(
            input.feedback_uri.len() <= 200,
            ReputationError::UriTooLong
        );

        let now = Clock::get()?.unix_timestamp;
        let slot = Clock::get()?.slot;

        // (1) Read OracleConfig cross-program ONLY for the writer-authority
        // gate. Per-period cap state can NOT be persisted from this handler
        // (we only own the AgentReputation PDA, not OracleConfig). v1 enforces
        // reputation write rate-limits OFF-CHAIN at the worker — the on-chain
        // gate is purely the writer-authority equality check below.
        // v1.5 will move the per-period state to a dedicated ReputationConfig
        // PDA owned by credmesh-reputation. See V1_ACCEPTANCE.md.
        let oracle_config_pda = credmesh_shared::cross_program::derive_pda(
            &[credmesh_receivable_oracle::ORACLE_CONFIG_SEED],
            &credmesh_shared::program_ids::RECEIVABLE_ORACLE,
        );
        let oracle_config = credmesh_shared::cross_program::read_cross_program_account::<
            credmesh_receivable_oracle::OracleConfig,
        >(
            &ctx.accounts.oracle_config.to_account_info(),
            &credmesh_shared::program_ids::RECEIVABLE_ORACLE,
            &oracle_config_pda,
        )
        .map_err(|_| ReputationError::MathOverflow)?;

        // (2) Always: append to digest + bump count + emit event.
        let attestor = ctx.accounts.attestor.key();
        let reputation = &mut ctx.accounts.reputation;

        // Rolling keccak digest = keccak(prev_digest || event_hash).
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

        // (3) Writer-gating per DECISIONS Q4: only the configured writer's
        // feedback updates `score_ema`. Permissionless writes still emit and
        // update the digest (8004 ergonomics) but don't move the score.
        if attestor == oracle_config.reputation_writer_authority {
            require!(
                input.score as u32 <= oracle_config.reputation_max_per_tx_score as u32,
                ReputationError::InvalidScore
            );

            // EMA update with N = EMA_WINDOW. score is u8 0..100; multiply by
            // 1e18 to get the 18-decimal representation, then EMA.
            let score_scaled = (input.score as u128).saturating_mul(1_000_000_000_000_000_000u128);
            let n = EMA_WINDOW as u128;
            // ema_new = (ema_old * (n - 1) + new_score) / n
            let ema_old = reputation.score_ema;
            let weighted_old = ema_old
                .checked_mul(n.saturating_sub(1))
                .ok_or(ReputationError::MathOverflow)?;
            let sum = weighted_old
                .checked_add(score_scaled)
                .ok_or(ReputationError::MathOverflow)?;
            reputation.score_ema = sum.checked_div(n).ok_or(ReputationError::MathOverflow)?;

            if input.reason_code & 0x8000 != 0 {
                // Convention: high bit of reason_code indicates a default event.
                reputation.default_count = reputation
                    .default_count
                    .checked_add(1)
                    .ok_or(ReputationError::MathOverflow)?;
            }

            reputation.last_event_slot = slot;
        }

        emit!(NewFeedback {
            agent_asset: reputation.agent_asset,
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

    pub fn append_response(
        ctx: Context<AppendResponse>,
        feedback_index: u64,
        response_uri: String,
        response_hash: [u8; 32],
    ) -> Result<()> {
        let _ = (ctx, feedback_index, response_uri, response_hash);
        Ok(())
    }

    pub fn revoke_feedback(ctx: Context<RevokeFeedback>, feedback_index: u64) -> Result<()> {
        let _ = (ctx, feedback_index);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitReputation<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Agent registry asset; just a key seed. Q1 will determine which
    /// program owns this account; once Q1 lands, add `owner = agent_registry::ID`.
    pub agent_asset: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = AgentReputation::SIZE,
        seeds = [REPUTATION_SEED, agent_asset.key().as_ref()],
        bump
    )]
    pub reputation: Account<'info, AgentReputation>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GiveFeedback<'info> {
    pub attestor: Signer<'info>,
    /// CHECK: Just a seed source.
    pub agent_asset: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [REPUTATION_SEED, agent_asset.key().as_ref()],
        bump = reputation.bump
    )]
    pub reputation: Account<'info, AgentReputation>,
    /// CHECK: OracleConfig PDA (owned by credmesh-receivable-oracle).
    /// Handler re-derives [ORACLE_CONFIG_SEED] under RECEIVABLE_ORACLE program ID
    /// and verifies the discriminator.
    pub oracle_config: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct AppendResponse<'info> {
    pub responder: Signer<'info>,
    /// CHECK: Seed source.
    pub agent_asset: UncheckedAccount<'info>,
    #[account(
        seeds = [REPUTATION_SEED, agent_asset.key().as_ref()],
        bump = reputation.bump
    )]
    pub reputation: Account<'info, AgentReputation>,
}

#[derive(Accounts)]
pub struct RevokeFeedback<'info> {
    pub original_attestor: Signer<'info>,
    /// CHECK: Seed source.
    pub agent_asset: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [REPUTATION_SEED, agent_asset.key().as_ref()],
        bump = reputation.bump
    )]
    pub reputation: Account<'info, AgentReputation>,
}

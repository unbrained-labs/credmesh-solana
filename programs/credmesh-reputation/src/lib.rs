use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod state;

pub use errors::ReputationError;
pub use events::*;
pub use state::*;

declare_id!("CRED1rep1111111111111111111111111111111111");

#[program]
pub mod credmesh_reputation {
    use super::*;

    pub fn init_reputation(ctx: Context<InitReputation>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    pub fn give_feedback(ctx: Context<GiveFeedback>, input: FeedbackInput) -> Result<()> {
        // DECISIONS Q4: permissionless writes are recorded in feedback_count
        // and feedback_digest (and emitted as NewFeedback events for indexers),
        // but only writes signed by the OracleConfig.reputation_writer_authority
        // update score_ema. Handler logic must:
        //   1. Always update feedback_count + feedback_digest + emit event.
        //   2. If attestor.key() == oracle_config.reputation_writer_authority:
        //        update score_ema, default_count, last_event_slot.
        //      Else: leave score fields untouched.
        //   3. Apply per-tx and per-period caps from OracleConfig before
        //        updating score state.
        let _ = (ctx, input);
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

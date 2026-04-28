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
    /// CHECK: Solana Agent Registry asset; just a key seed.
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

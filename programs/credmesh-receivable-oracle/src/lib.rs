use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod state;

pub use errors::OracleError;
pub use events::*;
pub use state::*;

declare_id!("CRED1recv11111111111111111111111111111111");

#[program]
pub mod credmesh_receivable_oracle {
    use super::*;

    pub fn init_oracle(ctx: Context<InitOracle>, params: InitOracleParams) -> Result<()> {
        let _ = (ctx, params);
        Ok(())
    }

    pub fn worker_update_receivable(
        ctx: Context<WorkerUpdateReceivable>,
        source_id: [u8; 32],
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        let _ = (ctx, source_id, amount, expires_at);
        Ok(())
    }

    pub fn ed25519_record_receivable(
        ctx: Context<Ed25519RecordReceivable>,
        source_id: [u8; 32],
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        let _ = (ctx, source_id, amount, expires_at);
        Ok(())
    }

    pub fn add_allowed_signer(
        ctx: Context<AddAllowedSigner>,
        kind: u8,
        max_per_receivable: u64,
        max_per_period: u64,
        period_seconds: i64,
    ) -> Result<()> {
        let _ = (ctx, kind, max_per_receivable, max_per_period, period_seconds);
        Ok(())
    }

    pub fn remove_allowed_signer(ctx: Context<RemoveAllowedSigner>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitOracleParams {
    pub worker_authority: Pubkey,
    pub worker_max_per_tx: u64,
    pub worker_max_per_period: u64,
    pub worker_period_seconds: i64,
}

#[derive(Accounts)]
pub struct InitOracle<'info> {
    #[account(mut)]
    pub governance: Signer<'info>,
    #[account(
        init,
        payer = governance,
        space = OracleConfig::SIZE,
        seeds = [ORACLE_CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, OracleConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(source_id: [u8; 32])]
pub struct WorkerUpdateReceivable<'info> {
    #[account(mut)]
    pub worker: Signer<'info>,
    #[account(
        mut,
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.worker_authority == worker.key() @ OracleError::NotWorkerAuthority
    )]
    pub config: Account<'info, OracleConfig>,
    /// CHECK: Just an address used as a seed.
    pub agent: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer = worker,
        space = Receivable::SIZE,
        seeds = [RECEIVABLE_SEED, agent.key().as_ref(), source_id.as_ref()],
        bump
    )]
    pub receivable: Account<'info, Receivable>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(source_id: [u8; 32])]
pub struct Ed25519RecordReceivable<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Just an address used as a seed.
    pub agent: UncheckedAccount<'info>,
    #[account(
        seeds = [ALLOWED_SIGNER_SEED, allowed_signer.signer.as_ref()],
        bump = allowed_signer.bump
    )]
    pub allowed_signer: Account<'info, AllowedSigner>,
    #[account(
        init_if_needed,
        payer = payer,
        space = Receivable::SIZE,
        seeds = [RECEIVABLE_SEED, agent.key().as_ref(), source_id.as_ref()],
        bump
    )]
    pub receivable: Account<'info, Receivable>,
    /// CHECK: Sysvar instructions, used to introspect the prior ed25519 verification ix.
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddAllowedSigner<'info> {
    #[account(mut)]
    pub governance: Signer<'info>,
    #[account(
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.governance == governance.key() @ OracleError::NotGovernance
    )]
    pub config: Account<'info, OracleConfig>,
    /// CHECK: Just an address used as a seed.
    pub signer_to_add: UncheckedAccount<'info>,
    #[account(
        init,
        payer = governance,
        space = AllowedSigner::SIZE,
        seeds = [ALLOWED_SIGNER_SEED, signer_to_add.key().as_ref()],
        bump
    )]
    pub allowed_signer: Account<'info, AllowedSigner>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveAllowedSigner<'info> {
    #[account(mut)]
    pub governance: Signer<'info>,
    #[account(
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.governance == governance.key() @ OracleError::NotGovernance
    )]
    pub config: Account<'info, OracleConfig>,
    #[account(
        mut,
        close = governance,
        seeds = [ALLOWED_SIGNER_SEED, allowed_signer.signer.as_ref()],
        bump = allowed_signer.bump
    )]
    pub allowed_signer: Account<'info, AllowedSigner>,
}

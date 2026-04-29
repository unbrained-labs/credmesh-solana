use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;

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
        signer_pubkey: Pubkey,
        source_id: [u8; 32],
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        // AUDIT AM-5: lazy period reset — top of handler should set
        //   if now >= signer.period_start + signer.period_seconds {
        //     signer.period_start = now; signer.period_used = 0;
        //   }
        // Then check per-receivable + per-period caps.
        // AUDIT integration #2: also verify the ed25519 ix offsets all point
        // at the verify ix itself (not at attacker-controlled bytes elsewhere).
        let _ = (ctx, signer_pubkey, source_id, amount, expires_at);
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
    pub governance: Pubkey,
    pub worker_authority: Pubkey,
    pub worker_max_per_tx: u64,
    pub worker_max_per_period: u64,
    pub worker_period_seconds: i64,
}

#[derive(Accounts)]
pub struct InitOracle<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
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
#[instruction(signer_pubkey: Pubkey, source_id: [u8; 32])]
pub struct Ed25519RecordReceivable<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: Just an address used as a seed.
    pub agent: UncheckedAccount<'info>,
    /// AUDIT P1-4: seed sourced from the ix arg, not the account's own field.
    /// Handler must additionally verify allowed_signer.signer == signer_pubkey
    /// AND that the ed25519 verify instruction signed-by matches signer_pubkey.
    #[account(
        mut,
        seeds = [ALLOWED_SIGNER_SEED, signer_pubkey.as_ref()],
        bump = allowed_signer.bump,
        constraint = allowed_signer.signer == signer_pubkey @ OracleError::SignerNotAllowed
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
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
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

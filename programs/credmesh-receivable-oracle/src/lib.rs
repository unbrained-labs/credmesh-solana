use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;

pub mod errors;
pub mod events;
pub mod state;

pub use errors::OracleError;
pub use events::*;
pub use state::*;

// PLACEHOLDER — replace before deploy via `anchor keys sync`. See DEPLOYMENT.md.
declare_id!("ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk");

#[program]
pub mod credmesh_receivable_oracle {
    use super::*;

    pub fn init_oracle(ctx: Context<InitOracle>, params: InitOracleParams) -> Result<()> {
        require!(params.worker_period_seconds > 0, OracleError::MathOverflow);

        let now = Clock::get()?.unix_timestamp;
        let config = &mut ctx.accounts.config;
        config.bump = ctx.bumps.config;
        config.governance = params.governance;
        config.worker_authority = params.worker_authority;
        config.worker_max_per_tx = params.worker_max_per_tx;
        config.worker_max_per_period = params.worker_max_per_period;
        config.worker_period_seconds = params.worker_period_seconds;
        config.worker_period_start = now;
        config.worker_period_used = 0;
        // DECISIONS Q4: reputation_writer_authority defaults to governance until
        // a separate `set_reputation_writer` ix lands. Off-chain, this is rotated
        // independently of the worker_authority — they MUST never be the same key.
        config.reputation_writer_authority = params.governance;
        config.reputation_max_per_tx_score = 100;
        config.reputation_max_per_period_count = 1_000;
        config.reputation_period_seconds = params.worker_period_seconds;
        config.reputation_period_start = now;
        config.reputation_period_used = 0;

        emit!(OracleInitialized {
            governance: config.governance,
            worker_authority: config.worker_authority,
        });

        Ok(())
    }

    pub fn worker_update_receivable(
        ctx: Context<WorkerUpdateReceivable>,
        source_id: [u8; 32],
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let slot = Clock::get()?.slot;

        require!(expires_at > now, OracleError::ReceivableExpired);

        let config = &mut ctx.accounts.config;

        // Lazy per-period reset.
        if now >= config.worker_period_start.saturating_add(config.worker_period_seconds) {
            config.worker_period_start = now;
            config.worker_period_used = 0;
        }

        // Per-tx cap.
        require!(
            amount <= config.worker_max_per_tx,
            OracleError::PerReceivableCapExceeded
        );

        // Per-period cap.
        let new_period_used = config
            .worker_period_used
            .checked_add(amount)
            .ok_or(OracleError::MathOverflow)?;
        require!(
            new_period_used <= config.worker_max_per_period,
            OracleError::PerPeriodCapExceeded
        );
        config.worker_period_used = new_period_used;

        let receivable = &mut ctx.accounts.receivable;
        receivable.bump = ctx.bumps.receivable;
        receivable.agent = ctx.accounts.agent.key();
        receivable.source_id = source_id;
        receivable.source_kind = 0; // SourceKind::Worker
        receivable.source_signer = None;
        receivable.amount = amount;
        receivable.expires_at = expires_at;
        receivable.last_updated_slot = slot;
        receivable.authority = ctx.accounts.worker.key();

        emit!(ReceivableUpdated {
            agent: receivable.agent,
            source_id,
            source_kind: 0,
            source_signer: None,
            amount,
            expires_at,
            authority: receivable.authority,
        });

        Ok(())
    }

    pub fn ed25519_record_receivable(
        ctx: Context<Ed25519RecordReceivable>,
        signer_pubkey: Pubkey,
        source_id: [u8; 32],
        amount: u64,
        expires_at: i64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let slot = Clock::get()?.slot;

        require!(expires_at > now, OracleError::ReceivableExpired);

        // (1) Verify the prior ed25519 ix and extract its signed pubkey + message.
        // The asymmetric.re/Relay-class fix is enforced inside the helper.
        let (verified_pubkey, signed_message) =
            credmesh_shared::ix_introspection::verify_prev_ed25519(
                &ctx.accounts.instructions_sysvar.to_account_info(),
            )
            .map_err(|_| OracleError::Ed25519Missing)?;

        // (2) The verified pubkey must equal the ix-arg signer_pubkey AND match
        // the AllowedSigner record (the latter is also enforced by the account
        // constraint, but we double-check here for defense-in-depth).
        require_keys_eq!(verified_pubkey, signer_pubkey, OracleError::SignerNotAllowed);
        require_keys_eq!(
            ctx.accounts.allowed_signer.signer,
            signer_pubkey,
            OracleError::SignerNotAllowed
        );

        // (3) Decode the message bytes per the canonical 96-byte layout.
        require!(
            signed_message.len() == credmesh_shared::ed25519_message::TOTAL_LEN,
            OracleError::Ed25519Missing
        );
        use credmesh_shared::ed25519_message as M;
        let msg_recv_id =
            &signed_message[M::RECEIVABLE_ID_OFFSET..M::RECEIVABLE_ID_OFFSET + M::RECEIVABLE_ID_LEN];
        let msg_agent =
            &signed_message[M::AGENT_OFFSET..M::AGENT_OFFSET + M::AGENT_LEN];
        let mut amount_buf = [0u8; 8];
        amount_buf.copy_from_slice(
            &signed_message[M::AMOUNT_OFFSET..M::AMOUNT_OFFSET + M::AMOUNT_LEN],
        );
        let msg_amount = u64::from_le_bytes(amount_buf);
        let mut expires_buf = [0u8; 8];
        expires_buf.copy_from_slice(
            &signed_message[M::EXPIRES_AT_OFFSET..M::EXPIRES_AT_OFFSET + M::EXPIRES_AT_LEN],
        );
        let msg_expires_at = i64::from_le_bytes(expires_buf);

        // The signed message must match the ix args bit-for-bit.
        // We don't compare msg_recv_id to source_id directly: we hash
        // (source_id || agent || amount || expires_at) into the message; receivable_id
        // CAN equal source_id when the source defines it that way, but in general
        // the signed receivable_id is what's authoritative, and source_id is the
        // PDA seed we use locally. We still require source_id to be consistent.
        require!(
            msg_recv_id == source_id.as_ref(),
            OracleError::Ed25519Missing
        );
        require!(
            msg_agent == ctx.accounts.agent.key().as_ref(),
            OracleError::Ed25519Missing
        );
        require!(msg_amount == amount, OracleError::Ed25519Missing);
        require!(msg_expires_at == expires_at, OracleError::Ed25519Missing);

        // (4) Cap enforcement on the AllowedSigner.
        let signer_acc = &mut ctx.accounts.allowed_signer;
        if now >= signer_acc.period_start.saturating_add(signer_acc.period_seconds) {
            signer_acc.period_start = now;
            signer_acc.period_used = 0;
        }
        require!(
            amount <= signer_acc.max_per_receivable,
            OracleError::PerReceivableCapExceeded
        );
        let new_period_used = signer_acc
            .period_used
            .checked_add(amount)
            .ok_or(OracleError::MathOverflow)?;
        require!(
            new_period_used <= signer_acc.max_per_period,
            OracleError::PerPeriodCapExceeded
        );
        signer_acc.period_used = new_period_used;

        // (5) Persist the Receivable PDA.
        let receivable = &mut ctx.accounts.receivable;
        receivable.bump = ctx.bumps.receivable;
        receivable.agent = ctx.accounts.agent.key();
        receivable.source_id = source_id;
        receivable.source_kind = signer_acc.kind; // 1=exchange, 2=x402_facilitator
        receivable.source_signer = Some(signer_pubkey);
        receivable.amount = amount;
        receivable.expires_at = expires_at;
        receivable.last_updated_slot = slot;
        receivable.authority = ctx.accounts.payer.key();

        emit!(ReceivableUpdated {
            agent: receivable.agent,
            source_id,
            source_kind: receivable.source_kind,
            source_signer: Some(signer_pubkey),
            amount,
            expires_at,
            authority: receivable.authority,
        });

        Ok(())
    }

    pub fn add_allowed_signer(
        ctx: Context<AddAllowedSigner>,
        kind: u8,
        max_per_receivable: u64,
        max_per_period: u64,
        period_seconds: i64,
    ) -> Result<()> {
        require!(period_seconds > 0, OracleError::MathOverflow);
        require!(kind == 1 || kind == 2, OracleError::SignerNotAllowed);

        let now = Clock::get()?.unix_timestamp;
        let signer = &mut ctx.accounts.allowed_signer;
        signer.bump = ctx.bumps.allowed_signer;
        signer.signer = ctx.accounts.signer_to_add.key();
        signer.kind = kind;
        signer.max_per_receivable = max_per_receivable;
        signer.max_per_period = max_per_period;
        signer.period_seconds = period_seconds;
        signer.period_start = now;
        signer.period_used = 0;

        emit!(AllowedSignerAdded {
            signer: signer.signer,
            kind,
            max_per_receivable,
            max_per_period,
        });

        Ok(())
    }

    pub fn remove_allowed_signer(ctx: Context<RemoveAllowedSigner>) -> Result<()> {
        let removed = ctx.accounts.allowed_signer.signer;
        emit!(AllowedSignerRemoved { signer: removed });
        Ok(())
    }

    /// Rotate the worker authority. Governance-gated; takes immediate effect.
    /// Caps and per-period state are reset.
    pub fn set_worker_authority(
        ctx: Context<SetWorkerAuthority>,
        new_authority: Pubkey,
        new_max_per_tx: u64,
        new_max_per_period: u64,
        new_period_seconds: i64,
    ) -> Result<()> {
        require!(new_period_seconds > 0, OracleError::MathOverflow);
        let now = Clock::get()?.unix_timestamp;
        let config = &mut ctx.accounts.config;
        config.worker_authority = new_authority;
        config.worker_max_per_tx = new_max_per_tx;
        config.worker_max_per_period = new_max_per_period;
        config.worker_period_seconds = new_period_seconds;
        config.worker_period_start = now;
        config.worker_period_used = 0;
        Ok(())
    }

    /// Rotate the reputation writer authority. Governance-gated.
    pub fn set_reputation_writer(
        ctx: Context<SetReputationWriter>,
        new_authority: Pubkey,
        new_max_per_tx_score: u8,
        new_max_per_period_count: u32,
        new_period_seconds: i64,
    ) -> Result<()> {
        require!(new_period_seconds > 0, OracleError::MathOverflow);
        require!(new_max_per_tx_score <= 100, OracleError::MathOverflow);
        let now = Clock::get()?.unix_timestamp;
        let config = &mut ctx.accounts.config;
        config.reputation_writer_authority = new_authority;
        config.reputation_max_per_tx_score = new_max_per_tx_score;
        config.reputation_max_per_period_count = new_max_per_period_count;
        config.reputation_period_seconds = new_period_seconds;
        config.reputation_period_start = now;
        config.reputation_period_used = 0;
        Ok(())
    }

    /// Rotate the governance authority itself. Use with extreme care — once
    /// changed, only the new governance can rotate it back. Gated by current
    /// governance.
    pub fn set_governance(ctx: Context<SetGovernance>, new_governance: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.governance = new_governance;
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

#[derive(Accounts)]
pub struct SetWorkerAuthority<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.governance == governance.key() @ OracleError::NotGovernance
    )]
    pub config: Account<'info, OracleConfig>,
}

#[derive(Accounts)]
pub struct SetReputationWriter<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.governance == governance.key() @ OracleError::NotGovernance
    )]
    pub config: Account<'info, OracleConfig>,
}

#[derive(Accounts)]
pub struct SetGovernance<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [ORACLE_CONFIG_SEED],
        bump = config.bump,
        constraint = config.governance == governance.key() @ OracleError::NotGovernance
    )]
    pub config: Account<'info, OracleConfig>,
}

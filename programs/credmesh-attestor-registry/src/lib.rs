use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;

pub mod errors;
pub mod events;
pub mod state;

pub use errors::AttestorRegistryError;
pub use events::*;
pub use state::*;

// Devnet program ID.
declare_id!("ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk");

/// Whitelist of bridge signers that produce ed25519 credit attestations
/// relayed from EVM `ReputationRegistry` / `ReputationCreditOracle`.
///
/// EVM holds the canonical agent identity, multi-attestor reputation,
/// score formula, and timelocked attestor whitelist. Solana credmesh-escrow
/// trusts EVM-attested credit limits via short-TTL ed25519-signed messages
/// produced by the bridge. This program is just the attestor whitelist —
/// nothing else.
///
/// Every state-changing ix is governance-gated via Squads CPI introspection
/// (a Squads ix against `config.governance` must appear in the same tx).
/// The bridge signer's add/remove flow is therefore timelocked and
/// multi-sig-approved.
#[program]
pub mod credmesh_attestor_registry {
    use super::*;

    pub fn init_registry(ctx: Context<InitRegistry>, governance: Pubkey) -> Result<()> {
        // Governance MUST be a real Squads vault PDA — the Pubkey::default()
        // (all-zero) value would render the registry permanently
        // ungovernable AND would be trivially impersonatable by any tx that
        // happens to list the zero pubkey among its accounts.
        require!(
            governance != Pubkey::default(),
            AttestorRegistryError::GovernanceRequired
        );
        let config = &mut ctx.accounts.config;
        config.bump = ctx.bumps.config;
        config.governance = governance;
        emit!(AttestorRegistryInitialized { governance });
        Ok(())
    }

    pub fn add_allowed_signer(
        ctx: Context<AddAllowedSigner>,
        signer: Pubkey,
        kind: u8,
    ) -> Result<()> {
        // Governance gate first (consistent ordering with the other three
        // handlers in this module — auth before payload validation).
        require_governance_cpi(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.config.governance,
        )?;
        require!(
            credmesh_shared::AttestorKind::from_u8(kind).is_some(),
            AttestorRegistryError::InvalidKind
        );

        let allowed = &mut ctx.accounts.allowed_signer;
        allowed.bump = ctx.bumps.allowed_signer;
        allowed.signer = signer;
        allowed.kind = kind;
        allowed.added_at = Clock::get()?.unix_timestamp;

        emit!(AllowedSignerAdded { signer, kind });
        Ok(())
    }

    pub fn remove_allowed_signer(ctx: Context<RemoveAllowedSigner>) -> Result<()> {
        require_governance_cpi(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.config.governance,
        )?;
        let signer = ctx.accounts.allowed_signer.signer;
        emit!(AllowedSignerRemoved { signer });
        Ok(())
    }

    pub fn set_governance(ctx: Context<SetGovernance>, new_governance: Pubkey) -> Result<()> {
        require_governance_cpi(
            &ctx.accounts.instructions_sysvar,
            &ctx.accounts.config.governance,
        )?;
        require!(
            new_governance != Pubkey::default(),
            AttestorRegistryError::GovernanceRequired
        );
        let old_governance = ctx.accounts.config.governance;
        ctx.accounts.config.governance = new_governance;
        emit!(GovernanceUpdated {
            old_governance,
            new_governance,
        });
        Ok(())
    }
}

fn require_governance_cpi(
    sysvar_ai: &UncheckedAccount<'_>,
    expected_vault: &Pubkey,
) -> Result<()> {
    credmesh_shared::ix_introspection::require_squads_governance_cpi(
        &sysvar_ai.to_account_info(),
        expected_vault,
    )
    .map_err(|_| error!(AttestorRegistryError::GovernanceRequired))
}

#[derive(Accounts)]
pub struct InitRegistry<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
        space = AttestorConfig::SIZE,
        seeds = [ATTESTOR_CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, AttestorConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(signer: Pubkey, _kind: u8)]
pub struct AddAllowedSigner<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(seeds = [ATTESTOR_CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AttestorConfig>,
    #[account(
        init,
        payer = cranker,
        space = AllowedSigner::SIZE,
        seeds = [ALLOWED_SIGNER_SEED, signer.as_ref()],
        bump
    )]
    pub allowed_signer: Account<'info, AllowedSigner>,
    /// CHECK: Pinned to the canonical sysvar instructions account; used by
    /// `require_squads_governance_cpi` to verify the tx contains a Squads
    /// ix against `config.governance`.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveAllowedSigner<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    /// CHECK: Rent-refund recipient on close. Cranker by default — keeps
    /// rotation gas-free for whoever runs the keeper.
    #[account(mut)]
    pub rent_refund: UncheckedAccount<'info>,
    #[account(seeds = [ATTESTOR_CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AttestorConfig>,
    #[account(
        mut,
        seeds = [ALLOWED_SIGNER_SEED, allowed_signer.signer.as_ref()],
        bump = allowed_signer.bump,
        close = rent_refund
    )]
    pub allowed_signer: Account<'info, AllowedSigner>,
    /// CHECK: see AddAllowedSigner.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SetGovernance<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    #[account(mut, seeds = [ATTESTOR_CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AttestorConfig>,
    /// CHECK: see AddAllowedSigner.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod pricing;
pub mod state;

pub use errors::CredmeshError;
pub use events::*;
pub use instructions::*;
pub use state::*;

// PLACEHOLDER — replace before deploy via `anchor keys sync`. See DEPLOYMENT.md.
declare_id!("DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF");

#[program]
pub mod credmesh_escrow {
    use super::*;

    pub fn init_pool(ctx: Context<InitPool>, params: InitPoolParams) -> Result<()> {
        instructions::init_pool::handler(ctx, params)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        instructions::deposit::handler(ctx, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
        instructions::withdraw::handler(ctx, shares)
    }

    pub fn request_advance(
        ctx: Context<RequestAdvance>,
        receivable_id: [u8; 32],
        amount: u64,
        source_kind: u8,
        nonce: [u8; 16],
    ) -> Result<()> {
        instructions::request_advance::handler(ctx, receivable_id, amount, source_kind, nonce)
    }

    pub fn claim_and_settle(ctx: Context<ClaimAndSettle>, payment_amount: u64) -> Result<()> {
        instructions::claim_and_settle::handler(ctx, payment_amount)
    }

    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        instructions::liquidate::handler(ctx)
    }

    pub fn propose_params(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
        instructions::propose_params::handler(ctx, params)
    }

    pub fn execute_params(ctx: Context<ExecuteParams>) -> Result<()> {
        instructions::execute_params::handler(ctx)
    }

    pub fn skim_protocol_fees(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
        instructions::skim_protocol_fees::handler(ctx, amount)
    }
}

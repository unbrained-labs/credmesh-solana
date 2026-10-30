//! Token-2022 forward-compat spike (issue #5).
//!
//! **NOT ENABLED in v1.** The default build uses classic SPL Token via
//! `anchor_spl::token`. This module is gated behind the `token-2022` Cargo
//! feature and demonstrates the planned migration to
//! `anchor_spl::token_interface`, which provides type wrappers compatible
//! with both classic SPL Token mints AND Token-2022 mints in the same
//! handler. The spike is a parallel, dormant code path; it does not
//! reroute any v1 handler.
//!
//! # Why this exists
//!
//! Mainnet USDC on Solana is currently classic SPL Token, but PYUSD and
//! several institutional stablecoins are Token-2022, and Circle has migrated
//! USDC to Token-2022 on other Solana-equivalent chains. v2 multi-asset
//! pools will need Token-2022 support, and pinning `Program<'info, Token>`
//! everywhere makes that migration painful. We cut the spike now so the
//! activation path is documented and reviewable.
//!
//! # Migration deltas
//!
//! | v1 (classic) | post-migration |
//! |---|---|
//! | `Program<'info, Token>` | `Interface<'info, TokenInterface>` |
//! | `Account<'info, TokenAccount>` | `InterfaceAccount<'info, TokenAccount>` |
//! | `Account<'info, Mint>` | `InterfaceAccount<'info, Mint>` |
//! | `token::transfer` | `token_interface::transfer_checked` |
//! | `token::transfer_with_signer` | `token_interface::transfer_checked` (CpiContext::new_with_signer) |
//! | `token::mint_to` | `token_interface::mint_to` |
//! | `token::burn` | `token_interface::burn` |
//!
//! `transfer_checked` is the load-bearing change: it requires the mint
//! account and the mint's decimals as additional CPI args. Token-2022
//! transfers MUST go through `transfer_checked` (the bare `transfer`
//! instruction is deprecated and will reject extension-bearing mints). The
//! spike CPI helper below shows the call shape; v1 handlers should be
//! ported by replacing each `token::transfer(...)` site with the helper.
//!
//! # Activation plan
//!
//! 1. v1 ships with this module dormant.
//! 2. Track the upstream USDC migration on Solana (Circle has not announced
//!    a date as of repo head). When Circle confirms, run the migration:
//!    enable the `token-2022` feature in the mainnet build, update every
//!    handler in `lib.rs` to use `Interface` / `InterfaceAccount` /
//!    `transfer_checked`, and re-deploy.
//! 3. v2 multi-asset pools immediately use the Token-2022 path.
//!
//! # Test plan when activated
//!
//! - Deploy two pools in the same Bankrun fixture: one classic SPL Token
//!   USDC, one Token-2022 mint. Run the full advance lifecycle on both.
//!   Assert identical pool-state mutations.
//! - Property fixture: a Token-2022 mint with a `transfer_fee` extension
//!   should cause the LP/agent receive amounts to differ from the
//!   transferred amount (the fee is collected by the mint authority).
//!   The escrow handler must compute the fee-net amount, NOT assume
//!   sender-amount == receiver-amount.
//!
//! # Out of scope for this spike
//!
//! - Re-routing v1 handlers (would risk the v1 ship; explicitly deferred).
//! - The off-chain TS client's token-program detection (Codama-generated
//!   client picks up the IDL change automatically once the migration lands).
//! - `transfer_fee` extension handling in the waterfall math (a v2 design
//!   issue — fee-bearing mints break the waterfall sum invariant; the v2
//!   handler must subtract the mint-side fee before computing protocol cut
//!   / lp cut / agent net).

#![cfg(feature = "token-2022")]

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::{Pool, POOL_SEED};

/// Token-2022-ready variant of the `Deposit` accounts struct.
///
/// Same on-chain account layout as v1 — `Pool`, `usdc_vault`, `share_mint`,
/// `lp_share_ata`, `lp_usdc_ata` are unchanged. Only the SPL-side type
/// wrappers swap to the interface forms so the same struct accepts both
/// classic and Token-2022 mints. v1's `Deposit` struct stays in `lib.rs`
/// for the default build.
#[derive(Accounts)]
pub struct DepositV2<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = lp,
    )]
    pub lp_usdc_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = pool.share_mint,
        token::authority = lp,
    )]
    pub lp_share_ata: InterfaceAccount<'info, TokenAccount>,
    /// AssetMint is read-only here, but the CPI to `transfer_checked` needs
    /// its account info. Pull it in once and reuse.
    #[account(address = pool.asset_mint)]
    pub asset_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// Spike helper: invoke `transfer_checked` on the LP→vault leg of a
/// deposit. The signature shape mirrors v1's `token::transfer(...)` site
/// in `lib.rs::deposit`; v1 handlers can be ported one-for-one.
///
/// The `decimals` argument MUST come from the mint at runtime (mints with
/// extensions can carry a different decimal count than callers expect).
/// Fetch via `ctx.accounts.asset_mint.decimals` and pass through.
pub fn lp_to_vault(ctx: &Context<DepositV2>, amount: u64) -> Result<()> {
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.lp_usdc_ata.to_account_info(),
        mint: ctx.accounts.asset_mint.to_account_info(),
        to: ctx.accounts.usdc_vault.to_account_info(),
        authority: ctx.accounts.lp.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );
    transfer_checked(cpi_ctx, amount, ctx.accounts.asset_mint.decimals)
}

/// Spike helper: PDA-signed transfer from the pool vault. Mirrors the
/// `token::transfer` + `CpiContext::new_with_signer` shape used in v1's
/// `withdraw` and `request_advance` handlers.
pub fn vault_to_recipient<'info>(
    token_program: AccountInfo<'info>,
    vault: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    recipient: AccountInfo<'info>,
    pool_authority: AccountInfo<'info>,
    pool_seeds: &[&[&[u8]]],
    amount: u64,
    mint_decimals: u8,
) -> Result<()> {
    let cpi_accounts = TransferChecked {
        from: vault,
        mint,
        to: recipient,
        authority: pool_authority,
    };
    let cpi_ctx = CpiContext::new_with_signer(token_program, cpi_accounts, pool_seeds);
    transfer_checked(cpi_ctx, amount, mint_decimals)
}

#[cfg(test)]
mod compile_only_tests {
    //! These tests don't run; their existence forces `cargo check
    //! --features token-2022` to type-check the module in CI when the
    //! feature is enabled.
    use super::*;

    #[allow(dead_code)]
    fn _ensure_token_interface_in_scope(
        _: token_interface::TransferChecked<'_>,
    ) {
    }
}

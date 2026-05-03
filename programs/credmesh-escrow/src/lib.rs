use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions as sysvar_instructions;
// Issue #15: bring `AssociatedToken` into module scope so the IDL-extraction
// build (`cargo test --features idl-build`) resolves the type. Anchor 0.30's
// `Program<'info, T>` generic does not see `anchor_spl::associated_token` via
// the fully-qualified path under that profile even with `anchor-spl/associated_token`
// active on the `idl-build` feature group.
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

pub mod errors;
pub mod events;
pub mod state;

pub use errors::CredmeshError;
pub use events::*;
pub use state::*;

// PLACEHOLDER — replace before deploy via `anchor keys sync`. See DEPLOYMENT.md.
declare_id!("DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF");

#[program]
pub mod credmesh_escrow {
    use super::*;

    pub fn init_pool(ctx: Context<InitPool>, params: InitPoolParams) -> Result<()> {
        require!(
            params.max_advance_pct_bps as u64 <= BPS_DENOMINATOR,
            CredmeshError::AdvanceExceedsCap
        );
        require!(params.timelock_seconds >= 0, CredmeshError::MathOverflow);
        // Audit-MED #5: reject malformed fee curves at construction.
        params.fee_curve.validate()?;

        let pool = &mut ctx.accounts.pool;
        pool.bump = ctx.bumps.pool;
        pool.asset_mint = ctx.accounts.asset_mint.key();
        pool.usdc_vault = ctx.accounts.usdc_vault.key();
        pool.share_mint = ctx.accounts.share_mint.key();
        pool.treasury_ata = params.treasury_ata;
        pool.governance = params.governance;
        pool.total_assets = 0;
        pool.total_shares = 0;
        pool.deployed_amount = 0;
        pool.accrued_protocol_fees = 0;
        pool.fee_curve = params.fee_curve;
        pool.max_advance_pct_bps = params.max_advance_pct_bps;
        pool.max_advance_abs = params.max_advance_abs;
        pool.timelock_seconds = params.timelock_seconds;
        pool.pending_params = None;

        emit!(PoolInitialized {
            pool: pool.key(),
            asset_mint: pool.asset_mint,
            share_mint: pool.share_mint,
            governance: pool.governance,
        });

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, CredmeshError::MathOverflow);

        let total_assets = ctx.accounts.pool.total_assets;
        let total_shares = ctx.accounts.pool.total_shares;
        let asset_mint = ctx.accounts.pool.asset_mint;
        let pool_bump = ctx.accounts.pool.bump;

        let shares_to_mint = preview_deposit(amount, total_assets, total_shares)?;
        require!(shares_to_mint > 0, CredmeshError::MathOverflow);

        // Transfer USDC LP → vault. Authority is the LP's signer.
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.lp_usdc_ata.to_account_info(),
                    to: ctx.accounts.usdc_vault.to_account_info(),
                    authority: ctx.accounts.lp.to_account_info(),
                },
            ),
            amount,
        )?;

        // Mint shares to LP. Authority is the Pool PDA, signed by seeds.
        let bump_arr = [pool_bump];
        let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
        let signer_seeds = &[pool_seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    to: ctx.accounts.lp_share_ata.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            shares_to_mint,
        )?;

        let pool = &mut ctx.accounts.pool;
        pool.total_assets = pool
            .total_assets
            .checked_add(amount)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.total_shares = pool
            .total_shares
            .checked_add(shares_to_mint)
            .ok_or(CredmeshError::MathOverflow)?;

        emit!(Deposited {
            pool: pool.key(),
            lp: ctx.accounts.lp.key(),
            amount,
            shares_minted: shares_to_mint,
        });

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, shares: u64) -> Result<()> {
        require!(shares > 0, CredmeshError::MathOverflow);

        let total_assets = ctx.accounts.pool.total_assets;
        let total_shares = ctx.accounts.pool.total_shares;
        let asset_mint = ctx.accounts.pool.asset_mint;
        let pool_bump = ctx.accounts.pool.bump;

        let assets_to_return = preview_redeem(shares, total_assets, total_shares)?;
        require!(assets_to_return > 0, CredmeshError::MathOverflow);

        // Idle-only enforcement: deployed USDC has physically left the vault,
        // so vault balance == idle. If shares would redeem more than is idle,
        // fail atomically.
        require!(
            ctx.accounts.usdc_vault.amount >= assets_to_return,
            CredmeshError::InsufficientIdleLiquidity
        );

        // Burn LP shares first (no signer needed; lp is authority).
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    from: ctx.accounts.lp_share_ata.to_account_info(),
                    authority: ctx.accounts.lp.to_account_info(),
                },
            ),
            shares,
        )?;

        // Transfer USDC vault → LP. Authority is the Pool PDA.
        let bump_arr = [pool_bump];
        let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
        let signer_seeds = &[pool_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.usdc_vault.to_account_info(),
                    to: ctx.accounts.lp_usdc_ata.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            assets_to_return,
        )?;

        let pool = &mut ctx.accounts.pool;
        pool.total_assets = pool
            .total_assets
            .checked_sub(assets_to_return)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.total_shares = pool
            .total_shares
            .checked_sub(shares)
            .ok_or(CredmeshError::MathOverflow)?;

        emit!(Withdrew {
            pool: pool.key(),
            lp: ctx.accounts.lp.key(),
            shares_burned: shares,
            assets_returned: assets_to_return,
        });

        Ok(())
    }

    pub fn request_advance(
        ctx: Context<RequestAdvance>,
        receivable_id: [u8; 32],
        amount: u64,
        source_kind: u8,
        nonce: [u8; 16],
    ) -> Result<()> {
        require!(amount >= MIN_ADVANCE_ATOMS, CredmeshError::AdvanceExceedsCap);
        let kind = credmesh_shared::SourceKind::from_u8(source_kind)
            .ok_or(CredmeshError::ReceivableStale)?;

        let now = Clock::get()?.unix_timestamp;
        let slot = Clock::get()?.slot;

        // (1) Verify agent identity (MPL Core asset + DelegateExecutionV1).
        // Per DECISIONS Q1; account-read only, no CPI.
        let exec_profile = ctx.accounts.executive_profile.as_ref().map(|a| a.as_ref());
        let exec_record = ctx.accounts.execution_delegate_record.as_ref().map(|a| a.as_ref());
        credmesh_shared::mpl_identity::verify_agent_signer(
            &ctx.accounts.agent.key(),
            &ctx.accounts.agent_asset.to_account_info(),
            &ctx.accounts.agent_identity.to_account_info(),
            exec_profile,
            exec_record,
        )
        .map_err(|e| match e {
            credmesh_shared::mpl_identity::IdentityError::NotACoreAsset
            | credmesh_shared::mpl_identity::IdentityError::AgentNotRegistered => {
                error!(CredmeshError::InvalidAgentAsset)
            }
            _ => error!(CredmeshError::AgentBindingMismatch),
        })?;

        // (2) Read AgentReputation cross-program. Issue #4: typed `Account` with
        // `seeds::program` runs Anchor's owner+address+discriminator+deserialize
        // verify automatically — no handler-side manual read.
        let reputation = &ctx.accounts.agent_reputation_pda;

        // (3) Read Receivable cross-program (Worker path) OR verify ed25519
        // signed receivable (Ed25519/X402 paths). Issue #4: same Anchor-typed
        // pattern; the Worker path resolves the `Option<Account>` to `Some`.
        let (receivable_amount, receivable_expires_at, source_signer) = match kind {
            credmesh_shared::SourceKind::Worker => {
                let receivable = ctx
                    .accounts
                    .receivable_pda
                    .as_ref()
                    .ok_or(error!(CredmeshError::ReceivablePdaMismatch))?;

                let staleness =
                    slot.saturating_sub(receivable.last_updated_slot);
                require!(
                    staleness <= credmesh_receivable_oracle::MAX_STALENESS_SLOTS,
                    CredmeshError::ReceivableStale
                );
                (receivable.amount, receivable.expires_at, None)
            }
            credmesh_shared::SourceKind::Ed25519 | credmesh_shared::SourceKind::X402 => {
                let (signed_pubkey, signed_msg) =
                    credmesh_shared::ix_introspection::verify_prev_ed25519(
                        &ctx.accounts.instructions_sysvar.to_account_info(),
                    )
                    .map_err(|e| match e {
                        credmesh_shared::ix_introspection::IxIntrospectionError::Ed25519OffsetMismatch => {
                            error!(CredmeshError::Ed25519MessageMismatch)
                        }
                        _ => error!(CredmeshError::Ed25519Missing),
                    })?;

                require!(
                    signed_msg.len() == credmesh_shared::ed25519_message::TOTAL_LEN,
                    CredmeshError::Ed25519MessageMismatch
                );

                use credmesh_shared::ed25519_message as M;
                let msg_recv_id = &signed_msg[M::RECEIVABLE_ID_OFFSET
                    ..M::RECEIVABLE_ID_OFFSET + M::RECEIVABLE_ID_LEN];
                let msg_agent =
                    &signed_msg[M::AGENT_OFFSET..M::AGENT_OFFSET + M::AGENT_LEN];
                let mut amount_buf = [0u8; 8];
                amount_buf.copy_from_slice(
                    &signed_msg[M::AMOUNT_OFFSET..M::AMOUNT_OFFSET + M::AMOUNT_LEN],
                );
                let msg_amount = u64::from_le_bytes(amount_buf);
                let mut expires_buf = [0u8; 8];
                expires_buf.copy_from_slice(
                    &signed_msg[M::EXPIRES_AT_OFFSET..M::EXPIRES_AT_OFFSET + M::EXPIRES_AT_LEN],
                );
                let msg_expires_at = i64::from_le_bytes(expires_buf);
                let msg_nonce = &signed_msg[M::NONCE_OFFSET..M::NONCE_OFFSET + M::NONCE_LEN];

                require!(
                    msg_recv_id == receivable_id.as_ref(),
                    CredmeshError::Ed25519MessageMismatch
                );
                require!(
                    msg_agent == ctx.accounts.agent_asset.key().as_ref(),
                    CredmeshError::Ed25519MessageMismatch
                );
                require!(msg_nonce == nonce.as_ref(), CredmeshError::Ed25519MessageMismatch);

                (msg_amount, msg_expires_at, Some(signed_pubkey))
            }
        };

        require!(receivable_expires_at > now, CredmeshError::ReceivableExpired);

        // (4) Cap checks: amount <= min(receivable * pct_bps / 10000, abs_cap, credit_from_score).
        let pool = &ctx.accounts.pool;
        let pct_cap = (receivable_amount as u128)
            .checked_mul(pool.max_advance_pct_bps as u128)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(CredmeshError::MathOverflow)?;
        let pct_cap = u64::try_from(pct_cap).map_err(|_| error!(CredmeshError::MathOverflow))?;
        require!(amount <= pct_cap, CredmeshError::AdvanceExceedsCap);
        require!(amount <= pool.max_advance_abs, CredmeshError::AdvanceExceedsCap);

        let credit_from_score = credit_from_score_ema(reputation.score_ema, &pool.fee_curve)?;
        require!(amount <= credit_from_score, CredmeshError::AdvanceExceedsCredit);

        // (5) Compute fee from the on-chain curve. The off-chain server's
        // pricing.ts must produce the same number; tests assert the equality.
        let duration_seconds = receivable_expires_at.saturating_sub(now).max(0) as u64;
        let utilization = compute_utilization_bps(pool)?;
        let fee_owed = compute_fee_amount(
            amount,
            duration_seconds,
            utilization,
            reputation.default_count,
            &pool.fee_curve,
        )?;

        // (6) Transfer USDC vault → agent. PDA-signed.
        let asset_mint = pool.asset_mint;
        let pool_bump = pool.bump;
        let bump_arr = [pool_bump];
        let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
        let signer_seeds = &[pool_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_usdc_vault.to_account_info(),
                    to: ctx.accounts.agent_usdc_ata.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        // (7) Init Advance + ConsumedPayment (Anchor handled via `init`).
        let advance = &mut ctx.accounts.advance;
        advance.bump = ctx.bumps.advance;
        advance.agent = ctx.accounts.agent.key();
        advance.receivable_id = receivable_id;
        advance.principal = amount;
        advance.fee_owed = fee_owed;
        advance.late_penalty_per_day = compute_late_penalty_per_day(amount, &pool.fee_curve)?;
        advance.issued_at = now;
        advance.expires_at = receivable_expires_at;
        advance.source_kind = source_kind;
        advance.source_signer = source_signer;
        advance.state = AdvanceState::Issued;
        let advance_key = advance.key();

        let consumed = &mut ctx.accounts.consumed;
        consumed.bump = ctx.bumps.consumed;
        consumed.nonce = nonce;
        consumed.agent = ctx.accounts.agent.key();
        consumed.created_at = now;

        // (8) Update Pool deployed_amount.
        let pool = &mut ctx.accounts.pool;
        pool.deployed_amount = pool
            .deployed_amount
            .checked_add(amount)
            .ok_or(CredmeshError::MathOverflow)?;

        // Post-state invariant: deployed never exceeds total_assets.
        require!(
            pool.deployed_amount <= pool.total_assets,
            CredmeshError::InsufficientIdleLiquidity
        );

        emit!(AdvanceIssued {
            pool: pool.key(),
            agent: ctx.accounts.agent.key(),
            advance: advance_key,
            principal: amount,
            fee_owed,
            expires_at: receivable_expires_at,
            source_kind,
        });

        Ok(())
    }

    pub fn claim_and_settle(ctx: Context<ClaimAndSettle>, payment_amount: u64) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        // Settlement window opens at `expires_at - CLAIM_WINDOW_SECONDS`.
        let claim_window_start = ctx
            .accounts
            .advance
            .expires_at
            .checked_sub(CLAIM_WINDOW_SECONDS)
            .ok_or(CredmeshError::MathOverflow)?;
        require!(now >= claim_window_start, CredmeshError::NotSettleable);

        // Memo nonce binding: payment tx must include a memo with the
        // ConsumedPayment.nonce bytes. Defends the "same TransferChecked
        // re-wrapped in a different outer tx" replay vector flagged in audit.
        let consumed_nonce = ctx.accounts.consumed.nonce;
        credmesh_shared::ix_introspection::require_memo_nonce(
            &ctx.accounts.instructions_sysvar.to_account_info(),
            &consumed_nonce,
        )
        .map_err(|_| error!(CredmeshError::MemoNonceMismatch))?;

        let principal = ctx.accounts.advance.principal;
        let fee_owed = ctx.accounts.advance.fee_owed;
        let late_penalty_per_day = ctx.accounts.advance.late_penalty_per_day;
        let expires_at = ctx.accounts.advance.expires_at;

        let late_seconds = (now - expires_at).max(0);
        let mut late_days = (late_seconds / 86_400) as u64;
        if late_days > MAX_LATE_DAYS as u64 {
            late_days = MAX_LATE_DAYS as u64;
        }
        let late_penalty = late_days
            .checked_mul(late_penalty_per_day)
            .ok_or(CredmeshError::MathOverflow)?;

        let total_owed = principal
            .checked_add(fee_owed)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_add(late_penalty)
            .ok_or(CredmeshError::MathOverflow)?;
        require!(payment_amount >= total_owed, CredmeshError::WaterfallSumMismatch);

        // Compute three cuts. Fee + late penalty splits 15/85; principal
        // returns to LP vault in full.
        let total_fee = fee_owed
            .checked_add(late_penalty)
            .ok_or(CredmeshError::MathOverflow)?;
        let protocol_cut_u128 = (total_fee as u128)
            .checked_mul(PROTOCOL_FEE_BPS as u128)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(CredmeshError::MathOverflow)?;
        let protocol_cut =
            u64::try_from(protocol_cut_u128).map_err(|_| error!(CredmeshError::MathOverflow))?;
        let lp_fee = total_fee
            .checked_sub(protocol_cut)
            .ok_or(CredmeshError::MathOverflow)?;
        let lp_cut = principal
            .checked_add(lp_fee)
            .ok_or(CredmeshError::MathOverflow)?;
        let agent_net = payment_amount
            .checked_sub(protocol_cut)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_sub(lp_cut)
            .ok_or(CredmeshError::MathOverflow)?;

        // Sum invariant check.
        require!(
            protocol_cut
                .checked_add(lp_cut)
                .and_then(|x| x.checked_add(agent_net))
                == Some(payment_amount),
            CredmeshError::WaterfallSumMismatch
        );

        // CPIs. Authority is the cranker (which == advance.agent in v1 per the
        // account-struct constraint).
        let cranker_ai = ctx.accounts.cranker.to_account_info();
        let payer_ata_ai = ctx.accounts.payer_usdc_ata.to_account_info();
        let token_program_ai = ctx.accounts.token_program.to_account_info();

        if protocol_cut > 0 {
            token::transfer(
                CpiContext::new(
                    token_program_ai.clone(),
                    Transfer {
                        from: payer_ata_ai.clone(),
                        to: ctx.accounts.protocol_treasury_ata.to_account_info(),
                        authority: cranker_ai.clone(),
                    },
                ),
                protocol_cut,
            )?;
        }

        if lp_cut > 0 {
            token::transfer(
                CpiContext::new(
                    token_program_ai.clone(),
                    Transfer {
                        from: payer_ata_ai.clone(),
                        to: ctx.accounts.pool_usdc_vault.to_account_info(),
                        authority: cranker_ai.clone(),
                    },
                ),
                lp_cut,
            )?;
        }

        // agent_net: the cranker IS the agent in v1; if payer_usdc_ata ==
        // agent_usdc_ata (typical), this is a self-transfer that we skip.
        // If the payer ATA is distinct from the agent's USDC ATA, transfer.
        if agent_net > 0
            && ctx.accounts.payer_usdc_ata.key() != ctx.accounts.agent_usdc_ata.key()
        {
            token::transfer(
                CpiContext::new(
                    token_program_ai.clone(),
                    Transfer {
                        from: payer_ata_ai.clone(),
                        to: ctx.accounts.agent_usdc_ata.to_account_info(),
                        authority: cranker_ai.clone(),
                    },
                ),
                agent_net,
            )?;
        }

        // Update Pool: principal returns to vault, lp_fee accrues to LPs via
        // share-price increase, protocol_cut is tracked for skim.
        let pool = &mut ctx.accounts.pool;
        pool.deployed_amount = pool
            .deployed_amount
            .checked_sub(principal)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.total_assets = pool
            .total_assets
            .checked_add(lp_fee)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.accrued_protocol_fees = pool
            .accrued_protocol_fees
            .checked_add(protocol_cut)
            .ok_or(CredmeshError::MathOverflow)?;

        let advance_key = ctx.accounts.advance.key();
        let agent_key = ctx.accounts.advance.agent;

        // The Anchor `close = agent` constraint on the Advance account
        // closes it at end-of-handler; rent goes to agent (neutralizes MEV).
        // We still set state for the (zero-data) post-close visibility.
        ctx.accounts.advance.state = AdvanceState::Settled;

        emit!(AdvanceSettled {
            pool: pool.key(),
            agent: agent_key,
            advance: advance_key,
            principal,
            lp_cut,
            protocol_cut,
            agent_net,
            late_days: late_days as u32,
        });

        Ok(())
    }

    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let liquidation_window_start = ctx
            .accounts
            .advance
            .expires_at
            .checked_add(LIQUIDATION_GRACE_SECONDS)
            .ok_or(CredmeshError::MathOverflow)?;
        require!(now >= liquidation_window_start, CredmeshError::NotLiquidatable);

        let principal = ctx.accounts.advance.principal;
        let agent = ctx.accounts.advance.agent;
        let advance_key = ctx.accounts.advance.key();

        // LPs eat the loss via share-price drop. Total assets decrease by the
        // unrecovered principal; total_shares is unchanged.
        let pool = &mut ctx.accounts.pool;
        pool.deployed_amount = pool
            .deployed_amount
            .checked_sub(principal)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.total_assets = pool
            .total_assets
            .checked_sub(principal)
            .ok_or(CredmeshError::MathOverflow)?;

        // AUDIT AM-7: keep `Advance` alive with state=Liquidated for audit trail.
        let advance = &mut ctx.accounts.advance;
        advance.state = AdvanceState::Liquidated;

        emit!(AdvanceLiquidated {
            pool: pool.key(),
            agent,
            advance: advance_key,
            loss: principal,
        });

        Ok(())
    }

    pub fn propose_params(ctx: Context<ProposeParams>, params: PendingParams) -> Result<()> {
        // Audit-MED #5: validate the proposed curve BEFORE staging it under
        // timelock. Catching this at propose-time (rather than execute-time)
        // gives governance the full timelock window to fix a bad submission
        // and keeps execute_params a pure timelock check.
        params.fee_curve.validate()?;

        let now = Clock::get()?.unix_timestamp;
        let pool = &mut ctx.accounts.pool;
        let mut params = params;
        params.execute_after = now
            .checked_add(pool.timelock_seconds)
            .ok_or(CredmeshError::MathOverflow)?;
        pool.pending_params = Some(params);

        emit!(ParamsProposed {
            pool: pool.key(),
            execute_after: pool
                .pending_params
                .as_ref()
                .map(|p| p.execute_after)
                .unwrap_or(0),
        });
        Ok(())
    }

    pub fn execute_params(ctx: Context<ExecuteParams>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let pool = &mut ctx.accounts.pool;
        let pending = pool
            .pending_params
            .clone()
            .ok_or(CredmeshError::NoPendingParams)?;
        require!(
            now >= pending.execute_after,
            CredmeshError::PendingParamsNotReady
        );

        pool.fee_curve = pending.fee_curve;
        pool.max_advance_pct_bps = pending.max_advance_pct_bps;
        pool.max_advance_abs = pending.max_advance_abs;
        pool.pending_params = None;

        emit!(ParamsExecuted { pool: pool.key() });
        Ok(())
    }

    pub fn skim_protocol_fees(ctx: Context<SkimProtocolFees>, amount: u64) -> Result<()> {
        require!(amount > 0, CredmeshError::MathOverflow);
        require!(
            amount <= ctx.accounts.pool.accrued_protocol_fees,
            CredmeshError::MathOverflow
        );

        let asset_mint = ctx.accounts.pool.asset_mint;
        let pool_bump = ctx.accounts.pool.bump;
        let bump_arr = [pool_bump];
        let pool_seeds: &[&[u8]] = &[POOL_SEED, asset_mint.as_ref(), &bump_arr];
        let signer_seeds = &[pool_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_usdc_vault.to_account_info(),
                    to: ctx.accounts.recipient_ata.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        let pool = &mut ctx.accounts.pool;
        pool.accrued_protocol_fees = pool
            .accrued_protocol_fees
            .checked_sub(amount)
            .ok_or(CredmeshError::MathOverflow)?;

        emit!(ProtocolFeesSkimmed {
            pool: pool.key(),
            amount,
            recipient: ctx.accounts.recipient_ata.key(),
        });

        Ok(())
    }
}

/// Virtual-shares math (OZ ERC-4626 `_decimalsOffset` pattern, ported to u128
/// to avoid intermediate overflow). With the offsets set in `state.rs`, a 1-atom
/// inflation attack costs ≥10⁶× any extractable profit.
///
/// shares_minted = (amount * (total_shares + V_S)) / (total_assets + V_A)
fn preview_deposit(amount: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let amount_u = amount as u128;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = amount_u
        .checked_mul(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares = numerator
        .checked_div(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(shares).map_err(|_| error!(CredmeshError::MathOverflow))
}

/// Map agent's reputation score (u128 with 18 decimals) into a max-credit cap
/// in USDC atoms. Tier curve per DECISIONS Q6 (no ML in v1).
///
/// Curve: score 0 → $0; score 50 (10⁶ × 50 in 18-dec representation) → $25;
///         score 80 → $100; score 95+ → $250 (KYC tier).
/// Hard ceiling = pool.max_advance_abs.
fn credit_from_score_ema(score_ema: u128, _curve: &FeeCurve) -> Result<u64> {
    // score_ema is u128 with 18 decimals — divide by 10^18 to get integer 0..100.
    let score_int = (score_ema / 1_000_000_000_000_000_000u128) as u64;
    let credit_usd = match score_int {
        0..=20 => 0u64,
        21..=49 => 10_000_000,   // $10
        50..=69 => 25_000_000,   // $25
        70..=84 => 100_000_000,  // $100
        85..=94 => 200_000_000,  // $200
        _ => 250_000_000,        // $250 (95-100; KYC-tier-equivalent)
    };
    Ok(credit_usd)
}

/// Compute the per-issuance fee. Mirrors `pricing.ts` shape:
/// utilization premium + duration premium + risk premium + (pool loss surcharge omitted in v1).
/// Returns USDC atoms (6 decimals).
fn compute_fee_amount(
    principal: u64,
    duration_seconds: u64,
    utilization_bps: u64,
    default_count: u32,
    curve: &FeeCurve,
) -> Result<u64> {
    let mut rate_bps: u64 = curve.base_rate_bps as u64;

    // Utilization kink (linear above kink → max).
    let kink = curve.utilization_kink_bps as u64;
    if utilization_bps > kink && (BPS_DENOMINATOR.saturating_sub(kink)) > 0 {
        let extra = utilization_bps - kink;
        let span = BPS_DENOMINATOR - kink;
        let kink_to_max = (curve.max_rate_bps as u64).saturating_sub(curve.kink_rate_bps as u64);
        rate_bps = curve.kink_rate_bps as u64
            + extra
                .checked_mul(kink_to_max)
                .ok_or(CredmeshError::MathOverflow)?
                / span;
    } else {
        let kink_minus_base =
            (curve.kink_rate_bps as u64).saturating_sub(curve.base_rate_bps as u64);
        let scaled = utilization_bps
            .checked_mul(kink_minus_base)
            .ok_or(CredmeshError::MathOverflow)?;
        rate_bps += if kink > 0 { scaled / kink } else { 0 };
    }

    // Duration premium.
    let duration_days = duration_seconds / 86_400;
    rate_bps = rate_bps
        .checked_add(duration_days.saturating_mul(curve.duration_per_day_bps as u64))
        .ok_or(CredmeshError::MathOverflow)?;

    // Risk premium scales with default_count (clamped at 5).
    let risk_factor = (default_count as u64).min(5);
    rate_bps = rate_bps
        .checked_add(risk_factor.saturating_mul(curve.risk_premium_bps as u64))
        .ok_or(CredmeshError::MathOverflow)?;

    rate_bps = rate_bps.min(curve.max_rate_bps as u64);

    let fee_u128 = (principal as u128)
        .checked_mul(rate_bps as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(fee_u128).map_err(|_| error!(CredmeshError::MathOverflow))
}

fn compute_late_penalty_per_day(principal: u64, curve: &FeeCurve) -> Result<u64> {
    // 0.1% per day of principal, multiplied by pool_loss_surcharge_bps if active.
    let base = (principal as u128)
        .checked_mul(10) // 0.1% = 10 bps
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let surcharge = curve.pool_loss_surcharge_bps as u128;
    let total = if surcharge > 0 {
        base.checked_mul(BPS_DENOMINATOR as u128 + surcharge)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(CredmeshError::MathOverflow)?
    } else {
        base
    };
    u64::try_from(total).map_err(|_| error!(CredmeshError::MathOverflow))
}

fn compute_utilization_bps(pool: &Pool) -> Result<u64> {
    if pool.total_assets == 0 {
        return Ok(BPS_DENOMINATOR);
    }
    let utilization_u128 = (pool.deployed_amount as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(pool.total_assets as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    Ok(u64::try_from(utilization_u128).unwrap_or(BPS_DENOMINATOR))
}

/// assets_returned = (shares * (total_assets + V_A)) / (total_shares + V_S)
fn preview_redeem(shares: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let shares_u = shares as u128;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = shares_u
        .checked_mul(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets = numerator
        .checked_div(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(assets).map_err(|_| error!(CredmeshError::MathOverflow))
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitPoolParams {
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    pub timelock_seconds: i64,
    /// AUDIT P1-6 / Q3: must be a Squads vault PDA. Wiring is currently a stored
    /// pubkey because Squads vaults are PDAs and can't be Signers; subsequent
    /// governance instructions must verify a Squads-CPI signed by this address.
    pub governance: Pubkey,
    pub treasury_ata: Pubkey,
}

#[derive(Accounts)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,
    #[account(
        init,
        payer = deployer,
        space = 8 + Pool::INIT_SPACE,
        seeds = [POOL_SEED, asset_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,
    pub asset_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = deployer,
        mint::decimals = 6,
        mint::authority = pool,
        mint::freeze_authority = pool
    )]
    pub share_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = deployer,
        token::mint = asset_mint,
        token::authority = pool
    )]
    pub usdc_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = lp
    )]
    pub lp_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = pool.share_mint,
        token::authority = lp
    )]
    pub lp_share_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub lp: Signer<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = lp
    )]
    pub lp_usdc_ata: Account<'info, TokenAccount>,
    #[account(mut, address = pool.share_mint)]
    pub share_mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = pool.share_mint,
        token::authority = lp
    )]
    pub lp_share_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(receivable_id: [u8; 32])]
pub struct RequestAdvance<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    /// CHECK: DECISIONS Q1 — agent_asset is an MPL Core asset (Solana account-owner
    /// is MPL_CORE, NOT the registry program). The handler reads the asset's
    /// `BaseAssetV1.owner` field at byte offset 1..33, then verifies the agent
    /// signer is either the owner or a registered DelegateExecutionV1 delegate
    /// (account-read against MPL Agent Tools — no CPI).
    #[account(owner = credmesh_shared::program_ids::MPL_CORE @ CredmeshError::InvalidAgentAsset)]
    pub agent_asset: UncheckedAccount<'info>,
    /// CHECK: AgentIdentityV2 PDA (or V1, both readable) owned by MPL Agent Registry.
    /// Handler re-derives ["agent_identity", agent_asset.key()] under MPL_AGENT_REGISTRY
    /// and asserts equality. Required to prove agent_asset is registered as an Agent.
    pub agent_identity: UncheckedAccount<'info>,
    /// AgentReputation PDA owned by credmesh-reputation. Issue #4: Anchor's
    /// typed Account + seeds::program does the four-step verify (owner →
    /// address → discriminator → deserialize) declaratively. The handler
    /// reads `score_ema` / `default_count` directly off this typed account.
    #[account(
        seeds = [credmesh_shared::seeds::REPUTATION_SEED, agent_asset.key().as_ref()],
        seeds::program = credmesh_reputation::ID,
        bump,
    )]
    pub agent_reputation_pda: Account<'info, credmesh_reputation::AgentReputation>,
    /// Receivable PDA owned by credmesh-receivable-oracle, required iff
    /// `source_kind = Worker`. For Ed25519 / X402 paths the handler verifies
    /// via instruction-introspection instead of reading this account, so the
    /// caller passes `None` (encoded as a missing account); a typed `Account`
    /// without `Option` would fail the discriminator check there. Anchor
    /// runs the four-step verify on `Some` only (issue #4).
    /// Audit-MED #3 fix: the source_kind byte (`[0]` for Worker) is part of
    /// the receivable PDA seed, so the address Anchor verifies here can only
    /// resolve to a Worker-created receivable — an ed25519-attested receivable
    /// (kind=1/2) lives at a different address and cannot pass this gate.
    #[account(
        seeds = [credmesh_shared::seeds::RECEIVABLE_SEED, &[0u8], agent.key().as_ref(), receivable_id.as_ref()],
        seeds::program = credmesh_receivable_oracle::ID,
        bump,
    )]
    pub receivable_pda: Option<Account<'info, credmesh_receivable_oracle::Receivable>>,
    /// CHECK: Optional ExecutiveProfileV1 PDA (MPL Agent Tools) for delegate path.
    /// Pass `None` (Anchor encodes as missing) when agent.key() is the asset's owner.
    pub executive_profile: Option<UncheckedAccount<'info>>,
    /// CHECK: Optional ExecutionDelegateRecordV1 PDA. Pair with executive_profile.
    pub execution_delegate_record: Option<UncheckedAccount<'info>>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(
        init,
        payer = agent,
        space = 8 + Advance::INIT_SPACE,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-5: ConsumedPayment is permanent. Never closed.
    /// `init` failure is the replay-protection mechanism.
    /// Issue #8: agent.key() in seeds so cross-agent receivable_id reuse
    /// doesn't collide on a shared PDA address.
    #[account(
        init,
        payer = agent,
        space = 8 + ConsumedPayment::INIT_SPACE,
        seeds = [CONSUMED_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id.as_ref()],
        bump
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = agent,
        associated_token::mint = usdc_mint,
        associated_token::authority = agent
    )]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    /// Handler uses `sysvar_instructions::load_instruction_at_checked` for
    /// ed25519/memo introspection.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimAndSettle<'info> {
    /// AUDIT P0-3/P0-4: in v1, the cranker MUST be the agent so the source-of-funds
    /// transfer is signer-authorized. Permissionless cranking deferred to a future
    /// version that introduces a payer-pre-authorized signing pattern.
    #[account(
        mut,
        constraint = cranker.key() == advance.agent @ CredmeshError::InvalidPayer
    )]
    pub cranker: Signer<'info>,
    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState,
        close = agent
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-5: ConsumedPayment is NOT closed here (would enable replay).
    /// Issue #8: seeds include advance.agent so the PDA derivation enforces
    /// the consumed↔advance binding via address (the explicit
    /// consumed.agent == advance.agent constraint stays as belt-and-suspenders).
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent @ CredmeshError::ReplayDetected
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    /// CHECK: Address-constrained to `advance.agent`. Receives rent refund from
    /// closing `advance` (via `close = agent`) plus the `agent_net` USDC transfer.
    #[account(mut, address = advance.agent)]
    pub agent: UncheckedAccount<'info>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = advance.agent
    )]
    pub agent_usdc_ata: Account<'info, TokenAccount>,
    /// AUDIT P0-3: pinned to the Pool's stored treasury ATA.
    #[account(mut, address = pool.treasury_ata)]
    pub protocol_treasury_ata: Account<'info, TokenAccount>,
    /// AUDIT P0-4: payer ATA must be authority-bound to the cranker (= agent in v1).
    #[account(
        mut,
        token::mint = pool.asset_mint,
        token::authority = cranker
    )]
    pub payer_usdc_ata: Account<'info, TokenAccount>,
    #[account(address = pool.asset_mint)]
    pub usdc_mint: Account<'info, Mint>,
    /// CHECK: AUDIT P1-2 — pinned to the canonical sysvar instructions account.
    #[account(address = sysvar_instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,
    /// AUDIT AM-7: keep `Advance` alive after liquidation for audit trail.
    /// Only `state` mutates to `Liquidated`. Closure happens via a separate
    /// admin-grace-period cleanup ix in a future version.
    #[account(
        mut,
        seeds = [ADVANCE_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = advance.bump,
        constraint = advance.state == AdvanceState::Issued @ CredmeshError::InvalidAdvanceState
    )]
    pub advance: Account<'info, Advance>,
    /// AUDIT P0-1: bind consumed.agent == advance.agent (was missing).
    /// AUDIT P0-5: ConsumedPayment is NOT closed.
    /// Issue #8: seeds include advance.agent so the PDA derivation enforces
    /// the consumed↔advance binding via address.
    #[account(
        seeds = [CONSUMED_SEED, pool.key().as_ref(), advance.agent.as_ref(), advance.receivable_id.as_ref()],
        bump = consumed.bump,
        constraint = consumed.agent == advance.agent @ CredmeshError::ReplayDetected
    )]
    pub consumed: Account<'info, ConsumedPayment>,
    #[account(mut, seeds = [POOL_SEED, pool.asset_mint.as_ref()], bump = pool.bump)]
    pub pool: Account<'info, Pool>,
}

#[derive(Accounts)]
pub struct ProposeParams<'info> {
    /// AUDIT P1-6 / Q3: until Squads-CPI integration lands, this is the address
    /// stored on Pool.governance — Squads vault PDAs cannot be Signers, so the
    /// program here must verify a Squads CPI by checking the calling program ID.
    /// Marked as a stored pubkey check, not a real Signer.
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.governance == governance.key() @ CredmeshError::GovernanceRequired
    )]
    pub pool: Account<'info, Pool>,
}

#[derive(Accounts)]
pub struct ExecuteParams<'info> {
    pub cranker: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,
}

#[derive(Accounts)]
pub struct SkimProtocolFees<'info> {
    pub governance: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.asset_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.governance == governance.key() @ CredmeshError::GovernanceRequired
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut, address = pool.usdc_vault)]
    pub pool_usdc_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        address = pool.treasury_ata
    )]
    pub recipient_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

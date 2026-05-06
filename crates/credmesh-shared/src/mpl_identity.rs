/// MPL Agent Registry identity verification — account-read only, no CPI.
///
/// The canonical handler-side helper to confirm that a Solana signer is the
/// rightful agent owner of an MPL Core asset OR a registered DelegateExecutionV1
/// delegate.
///
/// This module hardcodes the field offsets of three account types from the
/// `metaplex-foundation/mpl-agent` source as of SDK 0.2.5 (commit pinned in
/// `DECISIONS.md` Q1). Callers pass `AccountInfo`s; the helper validates owner,
/// discriminator key, and PDA derivation.
///
/// Compatibility: AgentIdentityV1 (40 bytes) and V2 (104 bytes) are both
/// readable; we only need the asset linkage which both versions store.
use anchor_lang::prelude::*;

use crate::mpl_core_asset;
use crate::mpl_delegate_record;
use crate::program_ids::{MPL_AGENT_REGISTRY, MPL_AGENT_TOOLS, MPL_CORE};

/// Minimal error surface for identity verification. Callers map these into
/// their own program error enum.
#[derive(Clone, Copy, Debug)]
pub enum IdentityError {
    NotACoreAsset,
    AgentNotRegistered,
    NotDelegated,
    Unauthorized,
}

/// Verifies that `agent_signer` is authorized to act for `agent_asset`.
///
/// Authorization is granted iff:
///   (a) `agent_signer.key()` equals the asset's `BaseAssetV1.owner` field, OR
///   (b) `(executive_profile, execution_delegate_record)` are passed and prove
///       the signer is a registered DelegateExecutionV1 delegate.
///
/// Returns `Ok(())` on success. Callers should map `Err` into their own error
/// enum via `.map_err(|e| ...)`.
pub fn verify_agent_signer(
    agent_signer: &Pubkey,
    agent_asset: &AccountInfo<'_>,
    agent_identity: &AccountInfo<'_>,
    executive_profile: Option<&AccountInfo<'_>>,
    execution_delegate_record: Option<&AccountInfo<'_>>,
) -> std::result::Result<(), IdentityError> {
    // (1) agent_asset is a real MPL Core asset.
    require_keys_match(agent_asset.owner, &MPL_CORE, IdentityError::NotACoreAsset)?;
    let asset_data = agent_asset
        .try_borrow_data()
        .map_err(|_| IdentityError::NotACoreAsset)?;
    require_min_len(&asset_data, mpl_core_asset::OWNER_OFFSET + mpl_core_asset::OWNER_LEN, IdentityError::NotACoreAsset)?;
    require_eq(
        asset_data[mpl_core_asset::KEY_OFFSET],
        mpl_core_asset::KEY_VALUE_ASSET_V1,
        IdentityError::NotACoreAsset,
    )?;

    // (2) agent_identity PDA is owned by MPL Agent Registry, has the right key
    // discriminator (V1 = 1, V2 = 2), and re-derives from the asset.
    require_keys_match(
        agent_identity.owner,
        &MPL_AGENT_REGISTRY,
        IdentityError::AgentNotRegistered,
    )?;
    let id_data = agent_identity
        .try_borrow_data()
        .map_err(|_| IdentityError::AgentNotRegistered)?;
    require_min_len(&id_data, 1, IdentityError::AgentNotRegistered)?;
    let id_key = id_data[0];
    if id_key != 1u8 && id_key != 2u8 {
        return Err(IdentityError::AgentNotRegistered);
    }
    let (expected_id_pda, _) = Pubkey::find_program_address(
        &[mpl_delegate_record::AGENT_IDENTITY_SEED, agent_asset.key.as_ref()],
        &MPL_AGENT_REGISTRY,
    );
    require_keys_match(agent_identity.key, &expected_id_pda, IdentityError::AgentNotRegistered)?;

    // (3) Authorization — owner-direct OR delegate.
    let asset_owner = read_pubkey_at(&asset_data, mpl_core_asset::OWNER_OFFSET)
        .ok_or(IdentityError::NotACoreAsset)?;

    if asset_owner == *agent_signer {
        return Ok(());
    }

    let (Some(prof_ai), Some(rec_ai)) = (executive_profile, execution_delegate_record) else {
        return Err(IdentityError::NotDelegated);
    };

    // ExecutiveProfileV1: owned by Tools, key=1, authority field at offset 8..40
    // (struct: key(1) + bump(1) + padding(6) + authority(32) + ...).
    require_keys_match(prof_ai.owner, &MPL_AGENT_TOOLS, IdentityError::NotDelegated)?;
    let prof_data = prof_ai
        .try_borrow_data()
        .map_err(|_| IdentityError::NotDelegated)?;
    require_min_len(&prof_data, 40, IdentityError::NotDelegated)?;
    require_eq(prof_data[0], 1u8, IdentityError::NotDelegated)?;
    let prof_authority = read_pubkey_at(&prof_data, 8).ok_or(IdentityError::NotDelegated)?;
    require_keys_match_pk(&prof_authority, agent_signer, IdentityError::NotDelegated)?;

    let (expected_prof_pda, _) = Pubkey::find_program_address(
        &[mpl_delegate_record::PROFILE_SEED, agent_signer.as_ref()],
        &MPL_AGENT_TOOLS,
    );
    require_keys_match(prof_ai.key, &expected_prof_pda, IdentityError::NotDelegated)?;

    // ExecutionDelegateRecordV1: owned by Tools, key=2, length 104, fields:
    //   key(1) + bump(1) + padding(6) + executive_profile(32) + authority(32) + agent_asset(32)
    require_keys_match(rec_ai.owner, &MPL_AGENT_TOOLS, IdentityError::NotDelegated)?;
    let rec_data = rec_ai
        .try_borrow_data()
        .map_err(|_| IdentityError::NotDelegated)?;
    require_min_len(&rec_data, mpl_delegate_record::TOTAL_LEN, IdentityError::NotDelegated)?;
    require_eq(rec_data[0], mpl_delegate_record::KEY_VALUE, IdentityError::NotDelegated)?;
    let rec_profile = read_pubkey_at(&rec_data, mpl_delegate_record::EXECUTIVE_PROFILE_OFFSET)
        .ok_or(IdentityError::NotDelegated)?;
    let rec_asset = read_pubkey_at(&rec_data, mpl_delegate_record::AGENT_ASSET_OFFSET)
        .ok_or(IdentityError::NotDelegated)?;
    require_keys_match_pk(&rec_profile, prof_ai.key, IdentityError::NotDelegated)?;
    require_keys_match_pk(&rec_asset, agent_asset.key, IdentityError::NotDelegated)?;

    let (expected_rec_pda, _) = Pubkey::find_program_address(
        &[
            mpl_delegate_record::RECORD_SEED,
            prof_ai.key.as_ref(),
            agent_asset.key.as_ref(),
        ],
        &MPL_AGENT_TOOLS,
    );
    require_keys_match(rec_ai.key, &expected_rec_pda, IdentityError::NotDelegated)?;

    Ok(())
}

/// Slim variant of `verify_agent_signer` for callers that only need to
/// confirm "this MPL Core asset's stored owner equals the given pubkey"
/// — no agent-identity registry lookup, no delegate path. Used by
/// credmesh-reputation::register_agent for the optional MPL identity
/// proof attached at registration. Includes the same defense-in-depth
/// checks: account-owner program, length, AND the
/// `KEY_VALUE_ASSET_V1` discriminator byte. Without the discriminator
/// check, a non-asset MPL Core account whose bytes 1..33 happened to
/// equal the agent's pubkey would falsely pass.
pub fn verify_asset_owner_match(
    agent_asset: &AccountInfo<'_>,
    expected_owner: &Pubkey,
) -> std::result::Result<bool, IdentityError> {
    if agent_asset.owner != &MPL_CORE {
        return Err(IdentityError::NotACoreAsset);
    }
    let asset_data = agent_asset
        .try_borrow_data()
        .map_err(|_| IdentityError::NotACoreAsset)?;
    require_min_len(
        &asset_data,
        mpl_core_asset::OWNER_OFFSET + mpl_core_asset::OWNER_LEN,
        IdentityError::NotACoreAsset,
    )?;
    require_eq(
        asset_data[mpl_core_asset::KEY_OFFSET],
        mpl_core_asset::KEY_VALUE_ASSET_V1,
        IdentityError::NotACoreAsset,
    )?;
    let asset_owner = read_pubkey_at(&asset_data, mpl_core_asset::OWNER_OFFSET)
        .ok_or(IdentityError::NotACoreAsset)?;
    Ok(asset_owner == *expected_owner)
}

fn read_pubkey_at(data: &[u8], offset: usize) -> Option<Pubkey> {
    if data.len() < offset + 32 {
        return None;
    }
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&data[offset..offset + 32]);
    Some(Pubkey::new_from_array(buf))
}

fn require_keys_match(
    a: &Pubkey,
    b: &Pubkey,
    err: IdentityError,
) -> std::result::Result<(), IdentityError> {
    if a == b { Ok(()) } else { Err(err) }
}

fn require_keys_match_pk(
    a: &Pubkey,
    b: &Pubkey,
    err: IdentityError,
) -> std::result::Result<(), IdentityError> {
    if a == b { Ok(()) } else { Err(err) }
}

fn require_min_len(data: &[u8], min: usize, err: IdentityError) -> std::result::Result<(), IdentityError> {
    if data.len() >= min { Ok(()) } else { Err(err) }
}

fn require_eq<T: PartialEq>(a: T, b: T, err: IdentityError) -> std::result::Result<(), IdentityError> {
    if a == b { Ok(()) } else { Err(err) }
}

/// Instruction sysvar introspection helpers.
///
/// Used for:
///   1. ed25519 signature verification — confirm the prior instruction is a
///      valid ed25519_program verify, AND that its internal offsets reference
///      the verify ix itself (the asymmetric.re/Relay-class fix).
///   2. Memo nonce binding — read a memo program instruction in the same tx
///      and confirm the bytes match an expected nonce.
///
/// Anchor 0.30 has no helper for parsing the ed25519 ix data; we hand-roll it
/// against the canonical layout from `ed25519-program-instruction.h`.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::{
    load_current_index_checked, load_instruction_at_checked,
};

use crate::program_ids::{ED25519, MEMO, SQUADS_V4};

#[derive(Clone, Copy, Debug)]
pub enum IxIntrospectionError {
    SysvarLoadFailed,
    PrevIxNotFound,
    Ed25519NotFound,
    Ed25519MalformedData,
    Ed25519OffsetMismatch,
    Ed25519PubkeyMismatch,
    Ed25519MessageMismatch,
    MemoNotFound,
    MemoMismatch,
}

/// Layout of a single Ed25519SignatureOffsets entry inside the ed25519 ix data.
/// 14 bytes: u16 signature_offset, u16 signature_instruction_index,
/// u16 public_key_offset, u16 public_key_instruction_index,
/// u16 message_data_offset, u16 message_data_size, u16 message_instruction_index.
const ED25519_OFFSETS_HEADER: usize = 2; // u8 num_signatures + u8 padding
const ED25519_OFFSETS_LEN: usize = 14;

#[derive(Clone, Copy, Debug)]
pub struct Ed25519Offsets {
    pub signature_offset: u16,
    pub signature_instruction_index: u16,
    pub public_key_offset: u16,
    pub public_key_instruction_index: u16,
    pub message_data_offset: u16,
    pub message_data_size: u16,
    pub message_instruction_index: u16,
}

/// Parse the offsets struct from raw ix data.
fn parse_offsets(data: &[u8]) -> Option<Ed25519Offsets> {
    if data.len() < ED25519_OFFSETS_HEADER + ED25519_OFFSETS_LEN {
        return None;
    }
    let body = &data[ED25519_OFFSETS_HEADER..ED25519_OFFSETS_HEADER + ED25519_OFFSETS_LEN];
    Some(Ed25519Offsets {
        signature_offset: u16::from_le_bytes([body[0], body[1]]),
        signature_instruction_index: u16::from_le_bytes([body[2], body[3]]),
        public_key_offset: u16::from_le_bytes([body[4], body[5]]),
        public_key_instruction_index: u16::from_le_bytes([body[6], body[7]]),
        message_data_offset: u16::from_le_bytes([body[8], body[9]]),
        message_data_size: u16::from_le_bytes([body[10], body[11]]),
        message_instruction_index: u16::from_le_bytes([body[12], body[13]]),
    })
}

/// Verify that the immediately-prior instruction in the current tx is a valid
/// ed25519 verification AND that its offsets reference the verify ix itself
/// (not bytes elsewhere — the asymmetric.re/Relay-class bug fix).
///
/// On success, returns `(signed_pubkey, signed_message_bytes_owned)`.
pub fn verify_prev_ed25519(
    sysvar_instructions_ai: &AccountInfo<'_>,
) -> std::result::Result<(Pubkey, Vec<u8>), IxIntrospectionError> {
    let cur_idx = load_current_index_checked(sysvar_instructions_ai)
        .map_err(|_| IxIntrospectionError::SysvarLoadFailed)?;
    if cur_idx == 0 {
        return Err(IxIntrospectionError::PrevIxNotFound);
    }
    let prev_idx = cur_idx - 1;

    let prev_ix = load_instruction_at_checked(prev_idx as usize, sysvar_instructions_ai)
        .map_err(|_| IxIntrospectionError::PrevIxNotFound)?;
    if prev_ix.program_id != ED25519 {
        return Err(IxIntrospectionError::Ed25519NotFound);
    }

    // Reject multi-signature ed25519 ixs. The verify ix layout supports
    // num_signatures > 1, where each entry has its own offset table; if we
    // accepted a multi-entry ix and only parsed slot 0, an attacker could
    // pad slot 0 with a dummy 128-byte message that happens to decode
    // benignly while slot 1 carries the real attacker-favorable signature.
    // We expect exactly one signature per attestation tx — strict equality
    // closes that ambiguity at zero cost.
    if prev_ix.data.is_empty() || prev_ix.data[0] != 1 {
        return Err(IxIntrospectionError::Ed25519MalformedData);
    }

    let offsets = parse_offsets(&prev_ix.data).ok_or(IxIntrospectionError::Ed25519MalformedData)?;

    // Asymmetric.re fix: every offset's instruction-index must point at the
    // verify ix itself. Otherwise an attacker can craft offsets that reference
    // attacker-controlled bytes elsewhere in the tx.
    if offsets.signature_instruction_index != prev_idx
        || offsets.public_key_instruction_index != prev_idx
        || offsets.message_instruction_index != prev_idx
    {
        return Err(IxIntrospectionError::Ed25519OffsetMismatch);
    }

    // Read pubkey + message from the verify ix's own data buffer.
    let pk_off = offsets.public_key_offset as usize;
    let msg_off = offsets.message_data_offset as usize;
    let msg_len = offsets.message_data_size as usize;

    if prev_ix.data.len() < pk_off + 32 {
        return Err(IxIntrospectionError::Ed25519MalformedData);
    }
    if prev_ix.data.len() < msg_off + msg_len {
        return Err(IxIntrospectionError::Ed25519MalformedData);
    }

    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(&prev_ix.data[pk_off..pk_off + 32]);
    let signed_pubkey = Pubkey::new_from_array(pk_bytes);
    let signed_message = prev_ix.data[msg_off..msg_off + msg_len].to_vec();
    Ok((signed_pubkey, signed_message))
}

/// Hard upper bound on instructions scanned by `require_memo_nonce`. Solana
/// transactions are capped at ~64 top-level instructions in practice (the
/// 1232-byte tx-size limit binds before this), so 64 is permissive without
/// being unbounded. Audit-MED #4 fix: when v1.5 makes `claim_and_settle`
/// permissionless, a malicious cranker could pad the tx with no-ops to
/// exhaust compute before reaching the Memo; an explicit cap defends now.
const MAX_IX_SCAN: usize = 64;

/// Find a Memo program instruction in the current tx whose data matches the
/// expected nonce bytes. Searches up to `MAX_IX_SCAN` top-level instructions
/// (memo placement is not constrained relative to the calling ix).
pub fn require_memo_nonce(
    sysvar_instructions_ai: &AccountInfo<'_>,
    expected_nonce: &[u8],
) -> std::result::Result<(), IxIntrospectionError> {
    for idx in 0..MAX_IX_SCAN {
        match load_instruction_at_checked(idx, sysvar_instructions_ai) {
            Ok(ix) => {
                if ix.program_id == MEMO && ix.data == expected_nonce {
                    return Ok(());
                }
            }
            Err(_) => break,
        }
    }
    Err(IxIntrospectionError::MemoNotFound)
}

/// Verify the current tx contains a Squads v4 instruction that
/// authorizes-and-spends against the expected governance vault PDA.
/// This is the defensive check that gates governance instructions on
/// credmesh-escrow (propose_params, skim_protocol_fees) and on
/// credmesh-attestor-registry (add/remove_allowed_signer,
/// set_governance) — Squads vault PDAs cannot be `Signer`s in Anchor,
/// so the equivalent of "must be signed by Squads" is "must be in a
/// tx that contains a Squads ix authorizing this vault to spend."
///
/// Tightening (v1): we additionally require the vault PDA to appear
/// as **writable** in the Squads ix's account list. This narrows the
/// surface from "any Squads ix mentioning the vault" to "Squads ix
/// where the vault is the subject of an authorize-and-execute call"
/// (vault_transaction_execute and equivalents pass the vault as
/// writable; informational/config Squads ixs that merely reference
/// the vault as a read-only audit target pass it as non-writable).
///
/// Residual surface (v1.5 hardening): an attacker tx that bundles BOTH
/// (a) a legitimate Squads vault_transaction_execute against the vault
/// for some unrelated CPI AND (b) the gated CredMesh ix would still
/// pass. Defeating that requires parsing the Squads ix's inner-ix
/// payload to confirm it's specifically targeting THIS handler call.
/// Tracked as v1.5; the practical exploit requires Squads multisig
/// authorization in the same window, which is the same trust root.
pub fn require_squads_governance_cpi(
    sysvar_instructions_ai: &AccountInfo<'_>,
    expected_vault: &Pubkey,
) -> std::result::Result<(), IxIntrospectionError> {
    for idx in 0..MAX_IX_SCAN {
        match load_instruction_at_checked(idx, sysvar_instructions_ai) {
            Ok(ix) => {
                if ix.program_id != SQUADS_V4 {
                    continue;
                }
                let writable_match = ix.accounts.iter().any(|a| {
                    a.pubkey == *expected_vault && a.is_writable
                });
                if writable_match {
                    return Ok(());
                }
            }
            Err(_) => break,
        }
    }
    Err(IxIntrospectionError::PrevIxNotFound)
}

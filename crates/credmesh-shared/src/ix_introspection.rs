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

use crate::program_ids::{ED25519, MEMO};

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

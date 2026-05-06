/// Cross-program account-read helpers.
///
/// credmesh-escrow reads `AllowedSigner` (owned by
/// credmesh-attestor-registry) when verifying ed25519 credit
/// attestations. The only safe way to do a cross-program account read
/// is the four-step manual verification:
///   1. Owner pubkey matches the expected program ID.
///   2. Account address matches the expected PDA derivation.
///   3. The 8-byte Anchor account discriminator matches the type.
///   4. Bytes after the discriminator deserialize cleanly into the typed struct.
///
/// Forgetting any one of these is the Wormhole / Cashio / similar-class bug.
/// All four are wrapped together below. (Anchor 0.30 typed `Account<T>`
/// + `seeds::program` declarative constraints do this automatically;
/// this helper is the manual escape hatch when the constraints don't
/// fit.)
use anchor_lang::prelude::*;
use anchor_lang::Discriminator;

#[derive(Clone, Copy, Debug)]
pub enum CrossProgramReadError {
    OwnerMismatch,
    AddressMismatch,
    DiscriminatorMismatch,
    Deserialize,
}

/// Verify + deserialize an Anchor account that lives in another program.
///
/// `expected_owner` — the program ID that owns the account.
/// `expected_address` — the PDA address derived from your seeds.
/// `info` — the AccountInfo passed by the caller.
///
/// Returns the typed account on success. Caller is responsible for any
/// further field-level validation.
pub fn read_cross_program_account<T>(
    info: &AccountInfo<'_>,
    expected_owner: &Pubkey,
    expected_address: &Pubkey,
) -> std::result::Result<T, CrossProgramReadError>
where
    T: AccountDeserialize + Discriminator,
{
    if info.owner != expected_owner {
        return Err(CrossProgramReadError::OwnerMismatch);
    }
    if info.key != expected_address {
        return Err(CrossProgramReadError::AddressMismatch);
    }
    let data = info
        .try_borrow_data()
        .map_err(|_| CrossProgramReadError::Deserialize)?;
    if data.len() < 8 {
        return Err(CrossProgramReadError::DiscriminatorMismatch);
    }
    if data[..8] != T::DISCRIMINATOR {
        return Err(CrossProgramReadError::DiscriminatorMismatch);
    }
    let mut slice = &data[..];
    T::try_deserialize(&mut slice).map_err(|_| CrossProgramReadError::Deserialize)
}

/// Re-derive a PDA from a slice of seed parts under a program ID.
/// Convenience wrapper that returns just the address (not the bump).
pub fn derive_pda(seeds: &[&[u8]], program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program_id).0
}

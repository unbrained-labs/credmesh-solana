use anchor_lang::prelude::*;

#[event]
pub struct AttestorRegistryInitialized {
    pub governance: Pubkey,
}

#[event]
pub struct AllowedSignerAdded {
    pub signer: Pubkey,
    pub kind: u8,
}

#[event]
pub struct AllowedSignerRemoved {
    pub signer: Pubkey,
}

#[event]
pub struct GovernanceUpdated {
    pub old_governance: Pubkey,
    pub new_governance: Pubkey,
}

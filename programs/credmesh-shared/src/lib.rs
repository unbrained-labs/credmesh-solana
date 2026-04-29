use anchor_lang::prelude::*;

declare_id!("CRED1shared1111111111111111111111111111111");

pub mod seeds {
    pub const POOL_SEED: &[u8] = b"pool";
    pub const ADVANCE_SEED: &[u8] = b"advance";
    pub const CONSUMED_SEED: &[u8] = b"consumed";
    pub const TREASURY_SEED: &[u8] = b"treasury";
    pub const REPUTATION_SEED: &[u8] = b"agent_reputation";
    pub const RECEIVABLE_SEED: &[u8] = b"receivable";
    pub const ALLOWED_SIGNER_SEED: &[u8] = b"allowed_signer";
    pub const ORACLE_CONFIG_SEED: &[u8] = b"oracle_config";
}

pub mod program_ids {
    use anchor_lang::prelude::Pubkey;
    use anchor_lang::pubkey;

    pub const ESCROW: Pubkey = pubkey!("CRED1escrow1111111111111111111111111111111");
    pub const REPUTATION: Pubkey = pubkey!("CRED1rep1111111111111111111111111111111111");
    pub const RECEIVABLE_ORACLE: Pubkey = pubkey!("CRED1recv11111111111111111111111111111111");
}

pub mod ed25519_message {
    pub const VERSION: u8 = 1;
    pub const TOTAL_LEN: usize = 96;
    pub const RECEIVABLE_ID_OFFSET: usize = 0;
    pub const RECEIVABLE_ID_LEN: usize = 32;
    pub const AGENT_OFFSET: usize = 32;
    pub const AGENT_LEN: usize = 32;
    pub const AMOUNT_OFFSET: usize = 64;
    pub const AMOUNT_LEN: usize = 8;
    pub const EXPIRES_AT_OFFSET: usize = 72;
    pub const EXPIRES_AT_LEN: usize = 8;
    pub const NONCE_OFFSET: usize = 80;
    pub const NONCE_LEN: usize = 16;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceKind {
    Worker,
    Ed25519,
    X402,
}

impl SourceKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Worker),
            1 => Some(Self::Ed25519),
            2 => Some(Self::X402),
            _ => None,
        }
    }
}

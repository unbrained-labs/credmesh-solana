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

    /// MPL Agent Registry — Identity program (DECISIONS Q1).
    /// Verified against `metaplex-foundation/mpl-agent` source `declare_id!`.
    /// Mainnet + devnet share the same address.
    /// **Audit caveat**: this layer is not separately audited; MPL Core is.
    pub const MPL_AGENT_REGISTRY: Pubkey = pubkey!("1DREGFgysWYxLnRnKQnwrxnJQeSMk2HmGaC6whw2B2p");

    /// MPL Agent Registry — Tools program (DelegateExecutionV1 flows).
    /// Verified against the same repo. Used to verify the `agent` signer is
    /// either the asset owner OR a registered ExecutionDelegateRecordV1 delegate.
    pub const MPL_AGENT_TOOLS: Pubkey = pubkey!("TLREGni9ZEyGC3vnPZtqUh95xQ8oPqJSvNjvB7FGK8S");

    /// MPL Core program. The Solana account-level owner of every Agent Registry
    /// asset is always this program; the agent's owner-wallet is the `owner`
    /// field at byte offset 1..33 of the asset's data (BaseAssetV1).
    pub const MPL_CORE: Pubkey = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");

    /// Squads v4 multisig program (DECISIONS Q3).
    /// Verified pin: commit `64af7330413d5c85cbbccfd8c27a05d45b6e666f` /
    /// `squads-multisig-program = "=2.0.0"`. Mainnet + devnet same address.
    pub const SQUADS_V4: Pubkey = pubkey!("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf");

    /// Memo program v2 (used for replay-nonce binding).
    pub const MEMO: Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");

    /// Native ed25519 signature-verification program.
    pub const ED25519: Pubkey = pubkey!("Ed25519SigVerify111111111111111111111111111");
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

/// Field offsets inside an MPL Core `BaseAssetV1` account's data buffer.
/// Verified against `mpl_core::accounts::BaseAssetV1`. Used to read the
/// asset's owner-wallet without a CPI.
pub mod mpl_core_asset {
    pub const KEY_OFFSET: usize = 0;
    pub const KEY_VALUE_ASSET_V1: u8 = 1;
    pub const OWNER_OFFSET: usize = 1;
    pub const OWNER_LEN: usize = 32;
}

/// Field offsets inside MPL Agent Tools' `ExecutionDelegateRecordV1`.
/// Verified against repo source. Layout: key(1) + bump(1) + padding(6) +
/// executive_profile(32) + authority(32) + agent_asset(32) = 104 bytes.
pub mod mpl_delegate_record {
    pub const KEY_VALUE: u8 = 2;
    pub const TOTAL_LEN: usize = 104;
    pub const EXECUTIVE_PROFILE_OFFSET: usize = 8;
    pub const AUTHORITY_OFFSET: usize = 40;
    pub const AGENT_ASSET_OFFSET: usize = 72;
    pub const RECORD_SEED: &[u8] = b"execution_delegate_record";
    pub const PROFILE_SEED: &[u8] = b"executive_profile";
    pub const AGENT_IDENTITY_SEED: &[u8] = b"agent_identity";
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

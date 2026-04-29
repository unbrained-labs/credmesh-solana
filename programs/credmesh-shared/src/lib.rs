use anchor_lang::prelude::*;

pub mod cross_program;
pub mod ix_introspection;
pub mod mpl_identity;

// PLACEHOLDER program IDs. They decode to valid 32-byte pubkeys but are NOT
// real keypair-derived addresses. Before any devnet deploy, run
// `solana-keygen new -o target/deploy/<program>-keypair.json` for each program
// and `anchor keys sync` to replace these. See DEPLOYMENT.md §"Phase 0".
declare_id!("11111111111111111111111111111115");

pub mod seeds {
    pub const POOL_SEED: &[u8] = b"pool";
    pub const ADVANCE_SEED: &[u8] = b"advance";
    pub const CONSUMED_SEED: &[u8] = b"consumed";
    pub const REPUTATION_SEED: &[u8] = b"agent_reputation";
    pub const RECEIVABLE_SEED: &[u8] = b"receivable";
    pub const ALLOWED_SIGNER_SEED: &[u8] = b"allowed_signer";
    pub const ORACLE_CONFIG_SEED: &[u8] = b"oracle_config";
}

/// 18-decimal scalar for `score_ema` (mirrors EVM 18-decimal arithmetic).
pub const SCORE_SCALE: u128 = 1_000_000_000_000_000_000;

pub const SECONDS_PER_DAY: u64 = 86_400;

pub mod program_ids {
    use anchor_lang::prelude::Pubkey;
    use anchor_lang::pubkey;

    // PLACEHOLDERS — replace before deploy. See DEPLOYMENT.md §"Phase 0".
    pub const ESCROW: Pubkey = pubkey!("11111111111111111111111111111112");
    pub const REPUTATION: Pubkey = pubkey!("11111111111111111111111111111113");
    pub const RECEIVABLE_ORACLE: Pubkey = pubkey!("11111111111111111111111111111114");

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

    /// Typed view of a 96-byte ed25519-signed receivable. Single source of truth
    /// for the layout — both escrow and receivable-oracle decode via this.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SignedReceivable {
        pub receivable_id: [u8; 32],
        pub agent: [u8; 32],
        pub amount: u64,
        pub expires_at: i64,
        pub nonce: [u8; 16],
    }

    pub fn decode(msg: &[u8]) -> Option<SignedReceivable> {
        if msg.len() != TOTAL_LEN {
            return None;
        }
        let mut receivable_id = [0u8; 32];
        receivable_id.copy_from_slice(&msg[RECEIVABLE_ID_OFFSET..RECEIVABLE_ID_OFFSET + RECEIVABLE_ID_LEN]);
        let mut agent = [0u8; 32];
        agent.copy_from_slice(&msg[AGENT_OFFSET..AGENT_OFFSET + AGENT_LEN]);
        let mut amount_buf = [0u8; 8];
        amount_buf.copy_from_slice(&msg[AMOUNT_OFFSET..AMOUNT_OFFSET + AMOUNT_LEN]);
        let mut expires_buf = [0u8; 8];
        expires_buf.copy_from_slice(&msg[EXPIRES_AT_OFFSET..EXPIRES_AT_OFFSET + EXPIRES_AT_LEN]);
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&msg[NONCE_OFFSET..NONCE_OFFSET + NONCE_LEN]);
        Some(SignedReceivable {
            receivable_id,
            agent,
            amount: u64::from_le_bytes(amount_buf),
            expires_at: i64::from_le_bytes(expires_buf),
            nonce,
        })
    }
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

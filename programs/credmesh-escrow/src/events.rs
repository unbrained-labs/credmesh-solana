use anchor_lang::prelude::*;

#[event]
pub struct PoolInitialized {
    pub pool: Pubkey,
    pub asset_mint: Pubkey,
    pub share_mint: Pubkey,
    pub governance: Pubkey,
}

#[event]
pub struct Deposited {
    pub pool: Pubkey,
    pub lp: Pubkey,
    pub amount: u64,
    pub shares_minted: u64,
}

#[event]
pub struct Withdrew {
    pub pool: Pubkey,
    pub lp: Pubkey,
    pub shares_burned: u64,
    pub assets_returned: u64,
}

#[event]
pub struct AdvanceIssued {
    pub pool: Pubkey,
    pub agent: Pubkey,
    pub advance: Pubkey,
    pub principal: u64,
    pub fee_owed: u64,
    pub expires_at: i64,
    pub source_kind: u8,
}

#[event]
pub struct AdvanceSettled {
    pub pool: Pubkey,
    pub agent: Pubkey,
    pub advance: Pubkey,
    pub principal: u64,
    pub lp_cut: u64,
    pub protocol_cut: u64,
    pub agent_net: u64,
    pub late_days: u32,
}

#[event]
pub struct AdvanceLiquidated {
    pub pool: Pubkey,
    pub agent: Pubkey,
    pub advance: Pubkey,
    pub loss: u64,
}

#[event]
pub struct ParamsProposed {
    pub pool: Pubkey,
    pub execute_after: i64,
}

#[event]
pub struct ParamsExecuted {
    pub pool: Pubkey,
}

#[event]
pub struct ProtocolFeesSkimmed {
    pub pool: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
}

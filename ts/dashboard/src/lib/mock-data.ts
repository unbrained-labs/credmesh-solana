// TODO: real client (issue #15) — replace mock data with Codama-generated client + RPC calls
// once the Worker IDL track lands.

export interface FeeCurve {
  utilization_kink_bps: number;
  base_rate_bps: number;
  kink_rate_bps: number;
  max_rate_bps: number;
  duration_per_day_bps: number;
  risk_premium_bps: number;
  pool_loss_surcharge_bps: number;
}

export interface PendingParams {
  fee_curve: FeeCurve;
  max_advance_pct_bps: number;
  max_advance_abs: bigint;
  execute_after: number; // unix seconds
}

export interface PoolState {
  asset_mint: string;
  usdc_vault: string;
  share_mint: string;
  governance: string;
  total_assets: bigint;
  total_shares: bigint;
  deployed_amount: bigint;
  accrued_protocol_fees: bigint;
  virtual_assets_offset: bigint;
  virtual_shares_offset: bigint;
  fee_curve: FeeCurve;
  max_advance_pct_bps: number;
  max_advance_abs: bigint;
  timelock_seconds: number;
  pending_params: PendingParams | null;
  paused: boolean;
}

export interface AdvanceState {
  pubkey: string;
  agent: string;
  receivable_id: string;
  principal: bigint;
  fee_owed: bigint;
  late_penalty_per_day: bigint;
  issued_at: number;
  expires_at: number;
  source_kind: 0 | 1 | 2;
  state: 'Issued' | 'Settled' | 'Liquidated';
}

export const MOCK_POOL: PoolState = {
  asset_mint: '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU', // devnet USDC
  usdc_vault: 'Bc8d9pLmu1AqYvF3uRzKxK7Hk3jyKKqRkzwcXUcJ8sLQ',
  share_mint: 'PoolShareMintPDA111111111111111111111111111',
  governance: 'SquadsVau1tAddr1111111111111111111111111111',
  total_assets: 12_487_500_000n, // 12,487.50 USDC
  total_shares: 12_350_000_000_000n,
  deployed_amount: 4_215_400_000n, // 4,215.40 USDC
  accrued_protocol_fees: 187_320_000n, // 187.32 USDC
  virtual_assets_offset: 1_000_000n,
  virtual_shares_offset: 1_000_000_000n,
  fee_curve: {
    utilization_kink_bps: 8000,
    base_rate_bps: 200,
    kink_rate_bps: 800,
    max_rate_bps: 2500,
    duration_per_day_bps: 50,
    risk_premium_bps: 150,
    pool_loss_surcharge_bps: 0,
  },
  max_advance_pct_bps: 3000,
  max_advance_abs: 100_000_000n, // $100
  timelock_seconds: 86400,
  pending_params: null,
  paused: false,
};

export const MOCK_ADVANCES: AdvanceState[] = [
  {
    pubkey: 'AdvNCePDA111111111111111111111111111',
    agent: 'AgEnTPubKey11111111111111111111111111',
    receivable_id: '0x7f2e9a3c4d5b6a7e8f9012345678abcd9b1c2d3e4f56789abcdef0123456789a',
    principal: 75_000_000n,
    fee_owed: 1_320_000n,
    late_penalty_per_day: 0n,
    issued_at: Math.floor(Date.now() / 1000) - 3 * 86400,
    expires_at: Math.floor(Date.now() / 1000) + 11 * 86400,
    source_kind: 0,
    state: 'Issued',
  },
  {
    pubkey: 'AdvNCePDA222222222222222222222222222',
    agent: 'AgEnTPubKey11111111111111111111111111',
    receivable_id: '0xfeedface1122334455667788aabbccddeeff00112233445566778899aabbccdd',
    principal: 42_000_000n,
    fee_owed: 720_000n,
    late_penalty_per_day: 0n,
    issued_at: Math.floor(Date.now() / 1000) - 8 * 86400,
    expires_at: Math.floor(Date.now() / 1000) + 6 * 86400,
    source_kind: 1,
    state: 'Issued',
  },
];

// Share price over time, last 30 days. Slowly accreting from 1.0000 → 1.0247.
export const MOCK_SHARE_PRICE_HISTORY: { t: number; price: number }[] = (() => {
  const now = Math.floor(Date.now() / 1000);
  const points: { t: number; price: number }[] = [];
  let p = 1.0;
  for (let i = 30; i >= 0; i--) {
    // small accretion + jitter
    p += 0.0008 + (Math.sin(i * 0.7) * 0.0003);
    points.push({ t: now - i * 86400, price: p });
  }
  return points;
})();

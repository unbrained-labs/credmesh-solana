/**
 * Dynamic fee pricing engine — Solana port of credmesh/packages/credit-worker/src/pricing.ts
 *
 * Identical math; only the integration surface (treasury source) changes.
 *
 * Fees are computed from four independent components:
 *   1. Utilization premium  — Aave-style kink model; fees rise when capital is scarce
 *   2. Duration premium     — longer advances carry more risk
 *   3. Agent risk premium   — based on repayment/completion history
 *   4. Pool loss surcharge  — pool absorbed defaults → fees rise to rebuild reserves
 *
 * Total fee is split:
 *   - Underwriter share (e.g. 85%) → Pool LPs (manifests as share-price increase)
 *   - Protocol share   (e.g. 15%) → ProtocolTreasury PDA
 *
 * The same parameters MUST be stored on-chain in `Pool.fee_curve` so the program
 * enforces them; the server quote and the on-chain quote MUST match exactly
 * (asserted in tests).
 */

const MIN_FEE_RATE = 0.02;
const MAX_FEE_RATE = 0.25;

export const PROTOCOL_FEE_BPS = 1500;

const OPTIMAL_UTILIZATION = 0.80;
const BASE_RATE = 0.02;
const SLOPE_1 = 0.04;
const SLOPE_2 = 0.60;

const DURATION_BRACKETS: ReadonlyArray<{ maxHours: number; premium: number }> = [
  { maxHours: 4, premium: 0.0 },
  { maxHours: 24, premium: 0.01 },
  { maxHours: 72, premium: 0.025 },
  { maxHours: 168, premium: 0.04 },
  { maxHours: Infinity, premium: 0.06 },
];

export interface PoolStateForPricing {
  totalAssets: number;
  deployedAmount: number;
  accruedDefaultLoss: number;
}

export interface FeeBreakdown {
  totalFee: number;
  effectiveRate: number;
  underwriterFee: number;
  protocolFee: number;
  components: {
    utilizationRate: number;
    utilizationPremium: number;
    durationPremium: number;
    riskPremium: number;
    poolLossSurcharge: number;
    totalRate: number;
  };
}

const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));
const roundTo = (n: number, places: number) => Math.round(n * 10 ** places) / 10 ** places;
const rc = (n: number) => roundTo(n, 2);

export function computeFee(
  principal: number,
  durationHours: number,
  repaymentRate: number,
  completionRate: number,
  pool: PoolStateForPricing,
): FeeBreakdown {
  const utilization = computeUtilization(pool);
  const utilizationPremium = computeUtilizationPremium(utilization);
  const durationPremium = computeDurationPremium(durationHours);
  const riskPremium = computeRiskPremium(repaymentRate, completionRate);
  const poolLossSurcharge = computePoolLossSurcharge(pool);

  const rawRate = utilizationPremium + durationPremium + riskPremium + poolLossSurcharge;
  const totalRate = clamp(roundTo(rawRate, 4), MIN_FEE_RATE, MAX_FEE_RATE);

  const totalFee = rc(principal * totalRate);
  const { underwriterFee, protocolFee } = splitFee(totalFee);

  return {
    totalFee,
    effectiveRate: roundTo(totalRate, 4),
    underwriterFee,
    protocolFee,
    components: {
      utilizationRate: roundTo(utilization, 4),
      utilizationPremium: roundTo(utilizationPremium, 4),
      durationPremium: roundTo(durationPremium, 4),
      riskPremium: roundTo(riskPremium, 4),
      poolLossSurcharge: roundTo(poolLossSurcharge, 4),
      totalRate,
    },
  };
}

export function splitFee(totalFee: number): { underwriterFee: number; protocolFee: number } {
  const protocolFee = Math.floor((totalFee * PROTOCOL_FEE_BPS) / 100) / 100;
  const underwriterFee = rc(totalFee - protocolFee);
  return { underwriterFee, protocolFee };
}

function computeUtilization(pool: PoolStateForPricing): number {
  const totalCapital = pool.totalAssets - pool.accruedDefaultLoss;
  if (totalCapital <= 0) return 1;
  const idle = pool.totalAssets - pool.deployedAmount;
  return clamp(1 - idle / totalCapital, 0, 1);
}

function computeUtilizationPremium(utilization: number): number {
  if (utilization <= OPTIMAL_UTILIZATION) {
    return BASE_RATE + (utilization / OPTIMAL_UTILIZATION) * SLOPE_1;
  }
  const excessUtilization = (utilization - OPTIMAL_UTILIZATION) / (1 - OPTIMAL_UTILIZATION);
  return BASE_RATE + SLOPE_1 + excessUtilization * SLOPE_2;
}

function computeDurationPremium(durationHours: number): number {
  for (const bracket of DURATION_BRACKETS) {
    if (durationHours <= bracket.maxHours) return bracket.premium;
  }
  return 0;
}

function computeRiskPremium(repaymentRate: number, completionRate: number): number {
  const repayPenalty = (1 - repaymentRate) * 0.05;
  const completionPenalty = (1 - completionRate) * 0.03;
  return repayPenalty + completionPenalty;
}

function computePoolLossSurcharge(pool: PoolStateForPricing): number {
  if (pool.totalAssets <= 0) return 0;
  const lossRatio = pool.accruedDefaultLoss / pool.totalAssets;
  return clamp(lossRatio * 0.15, 0, 0.03);
}

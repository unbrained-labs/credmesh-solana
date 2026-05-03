// USDC has 6 decimals everywhere in the protocol (Pool.total_assets etc.)
export const USDC_DECIMALS = 6;
const USDC_UNIT = 1_000_000n;

export function formatUsdc(microUsdc: bigint | number, opts?: { precision?: number; showSymbol?: boolean }): string {
  const precision = opts?.precision ?? 2;
  const showSymbol = opts?.showSymbol ?? true;
  const v = typeof microUsdc === 'bigint' ? microUsdc : BigInt(Math.trunc(microUsdc));
  const whole = v / USDC_UNIT;
  const frac = v % USDC_UNIT;
  const fracStr = frac.toString().padStart(6, '0').slice(0, precision);
  const wholeStr = whole.toLocaleString('en-US');
  const num = precision > 0 ? `${wholeStr}.${fracStr}` : wholeStr;
  return showSymbol ? `${num} USDC` : num;
}

export function formatBps(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`;
}

export function shortAddr(addr: string, chars = 4): string {
  if (!addr) return '—';
  if (addr.length <= chars * 2 + 1) return addr;
  return `${addr.slice(0, chars)}…${addr.slice(-chars)}`;
}

export function sharePrice(totalAssets: bigint, totalShares: bigint, virtualAssets: bigint, virtualShares: bigint): number {
  // assets-per-share, scaled to a regular number for display
  const num = Number(totalAssets + virtualAssets);
  const den = Number(totalShares + virtualShares);
  if (den === 0) return 0;
  return num / den;
}

export function formatSharePrice(p: number): string {
  // shares are 9-dec; assets are 6-dec — ratio in raw units is ~10^-3.
  // We display normalized so a deposit at parity reads 1.0000.
  return (p * 1000).toFixed(4);
}

export function utilizationBps(deployed: bigint, totalAssets: bigint): number {
  if (totalAssets === 0n) return 0;
  return Number((deployed * 10000n) / totalAssets);
}

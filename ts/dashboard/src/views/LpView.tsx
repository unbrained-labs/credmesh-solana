import { useMemo, useState, type FC, type FormEvent } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { Card } from '../components/Card';
import { Stat } from '../components/Stat';
import { SharePriceChart } from '../components/SharePriceChart';
import { MOCK_POOL, MOCK_SHARE_PRICE_HISTORY } from '../lib/mock-data';
import {
  formatBps,
  formatSharePrice,
  formatUsdc,
  sharePrice,
  utilizationBps,
} from '../lib/format';

export const LpView: FC = () => {
  const { publicKey } = useWallet();
  const pool = MOCK_POOL;

  const price = useMemo(
    () =>
      sharePrice(
        pool.total_assets,
        pool.total_shares,
        pool.virtual_assets_offset,
        pool.virtual_shares_offset,
      ),
    [pool],
  );
  const utilBps = utilizationBps(pool.deployed_amount, pool.total_assets);

  return (
    <div className="space-y-6">
      <header className="flex items-end justify-between">
        <div>
          <p className="text-xs uppercase tracking-widest text-zinc-500">
            Liquidity Provider
          </p>
          <h1 className="mt-1 text-2xl font-medium text-zinc-100">
            USDC Pool
          </h1>
          <p className="mt-1 text-sm text-zinc-500">
            ERC-4626-style virtual-shares accounting · {pool.paused ? 'paused' : 'open'}
          </p>
        </div>
        <div className="text-right">
          <p className="text-xs uppercase tracking-widest text-zinc-500">Share price</p>
          <p className="tnum font-mono text-2xl text-indigo-300">
            {formatSharePrice(price)}
            <span className="text-zinc-500 text-base ml-1">USDC/share</span>
          </p>
        </div>
      </header>

      <Card title="Pool stats">
        <div className="grid grid-cols-2 md:grid-cols-5 gap-6">
          <Stat
            label="Total assets"
            value={formatUsdc(pool.total_assets, { showSymbol: false })}
            hint="USDC"
          />
          <Stat
            label="Total shares"
            value={(Number(pool.total_shares) / 1e9).toLocaleString('en-US', {
              maximumFractionDigits: 2,
            })}
            hint="9-dec mint"
          />
          <Stat
            label="Deployed"
            value={formatUsdc(pool.deployed_amount, { showSymbol: false })}
            hint={`${formatBps(utilBps)} utilization`}
          />
          <Stat
            label="Idle"
            value={formatUsdc(pool.total_assets - pool.deployed_amount, {
              showSymbol: false,
            })}
            hint="Withdrawable"
          />
          <Stat
            label="Protocol fees"
            value={formatUsdc(pool.accrued_protocol_fees, { showSymbol: false })}
            hint="Skim via gov"
            accent
          />
        </div>
      </Card>

      <Card title="Share price · last 30 days" description="Mock data — TODO: real client (issue #15)">
        <SharePriceChart points={MOCK_SHARE_PRICE_HISTORY} />
        <div className="mt-3 flex justify-between text-xs text-zinc-500 tnum font-mono">
          <span>{formatSharePrice(MOCK_SHARE_PRICE_HISTORY[0].price)}</span>
          <span>{formatSharePrice(MOCK_SHARE_PRICE_HISTORY.at(-1)!.price)}</span>
        </div>
      </Card>

      <div className="grid md:grid-cols-2 gap-6">
        <DepositForm walletConnected={!!publicKey} sharePrice={price} />
        <WithdrawForm walletConnected={!!publicKey} sharePrice={price} idle={pool.total_assets - pool.deployed_amount} />
      </div>
    </div>
  );
};

const DepositForm: FC<{ walletConnected: boolean; sharePrice: number }> = ({
  walletConnected,
  sharePrice: price,
}) => {
  const [amount, setAmount] = useState('');
  const [pending, setPending] = useState(false);

  const sharesOut = useMemo(() => {
    const v = Number(amount);
    if (!v || v <= 0 || price === 0) return 0;
    // raw amount in micro-USDC, shares are 9-dec → preview formula in display units:
    return v / (price * 1000);
  }, [amount, price]);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setPending(true);
    // TODO: real client (issue #15) — build & sign deposit() ix via Codama client
    console.info('[stub] deposit()', { amount });
    await new Promise((r) => setTimeout(r, 600));
    setPending(false);
    alert(`Stub: would call escrow::deposit(${amount} USDC)`);
  };

  return (
    <Card title="Deposit" description="Mint pool shares against USDC">
      <form onSubmit={onSubmit} className="space-y-4">
        <label className="block">
          <span className="text-xs text-zinc-400">Amount (USDC)</span>
          <input
            inputMode="decimal"
            value={amount}
            onChange={(e) => setAmount(e.target.value.replace(/[^0-9.]/g, ''))}
            placeholder="0.00"
            className="mt-1 w-full rounded-md bg-zinc-950 border border-zinc-800 px-3 py-2 text-sm tnum font-mono focus:outline-none focus:ring-1 focus:ring-indigo-500/60 focus:border-indigo-500/60"
          />
        </label>
        <div className="flex justify-between text-xs text-zinc-500">
          <span>You receive</span>
          <span className="tnum font-mono text-zinc-300">
            {sharesOut.toLocaleString('en-US', { maximumFractionDigits: 4 })} shares
          </span>
        </div>
        <button
          type="submit"
          disabled={!walletConnected || !amount || pending}
          className="w-full h-9 rounded-md bg-indigo-500 text-white text-sm font-medium hover:bg-indigo-400 disabled:bg-zinc-800 disabled:text-zinc-600 transition-colors"
        >
          {pending ? 'Submitting…' : walletConnected ? 'Deposit' : 'Connect wallet to deposit'}
        </button>
      </form>
    </Card>
  );
};

const WithdrawForm: FC<{ walletConnected: boolean; sharePrice: number; idle: bigint }> = ({
  walletConnected,
  sharePrice: price,
  idle,
}) => {
  const [shares, setShares] = useState('');
  const [pending, setPending] = useState(false);

  const assetsOut = useMemo(() => {
    const v = Number(shares);
    if (!v || v <= 0) return 0;
    return v * price * 1000;
  }, [shares, price]);

  const overIdle = BigInt(Math.floor(assetsOut * 1e6)) > idle;

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setPending(true);
    // TODO: real client (issue #15) — build & sign withdraw() ix via Codama client
    console.info('[stub] withdraw()', { shares });
    await new Promise((r) => setTimeout(r, 600));
    setPending(false);
    alert(`Stub: would call escrow::withdraw(${shares} shares)`);
  };

  return (
    <Card title="Withdraw" description="Burn shares to redeem USDC (idle only)">
      <form onSubmit={onSubmit} className="space-y-4">
        <label className="block">
          <span className="text-xs text-zinc-400">Shares</span>
          <input
            inputMode="decimal"
            value={shares}
            onChange={(e) => setShares(e.target.value.replace(/[^0-9.]/g, ''))}
            placeholder="0.0000"
            className="mt-1 w-full rounded-md bg-zinc-950 border border-zinc-800 px-3 py-2 text-sm tnum font-mono focus:outline-none focus:ring-1 focus:ring-indigo-500/60 focus:border-indigo-500/60"
          />
        </label>
        <div className="flex justify-between text-xs text-zinc-500">
          <span>You receive</span>
          <span className={`tnum font-mono ${overIdle ? 'text-amber-400' : 'text-zinc-300'}`}>
            {assetsOut.toLocaleString('en-US', { maximumFractionDigits: 2 })} USDC
            {overIdle && ' · exceeds idle'}
          </span>
        </div>
        <button
          type="submit"
          disabled={!walletConnected || !shares || pending || overIdle}
          className="w-full h-9 rounded-md bg-indigo-500 text-white text-sm font-medium hover:bg-indigo-400 disabled:bg-zinc-800 disabled:text-zinc-600 transition-colors"
        >
          {pending ? 'Submitting…' : walletConnected ? 'Withdraw' : 'Connect wallet to withdraw'}
        </button>
      </form>
    </Card>
  );
};

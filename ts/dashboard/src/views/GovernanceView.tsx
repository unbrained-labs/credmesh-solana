import type { FC } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { Card } from '../components/Card';
import { Stat } from '../components/Stat';
import { MOCK_POOL } from '../lib/mock-data';
import { formatBps, formatUsdc, shortAddr } from '../lib/format';

const FEE_CURVE_PARAMS: { key: keyof typeof MOCK_POOL.fee_curve; label: string; hint?: string }[] = [
  { key: 'utilization_kink_bps', label: 'Utilization kink', hint: 'Curve elbow' },
  { key: 'base_rate_bps', label: 'Base rate', hint: 'Floor at 0% util' },
  { key: 'kink_rate_bps', label: 'Kink rate', hint: 'Rate at kink' },
  { key: 'max_rate_bps', label: 'Max rate', hint: 'Cap at 100% util' },
  { key: 'duration_per_day_bps', label: 'Duration / day', hint: 'Additive' },
  { key: 'risk_premium_bps', label: 'Risk premium', hint: 'Multiplier on score_ema' },
  { key: 'pool_loss_surcharge_bps', label: 'Loss surcharge', hint: 'Post-default' },
];

export const GovernanceView: FC = () => {
  const { publicKey } = useWallet();
  const pool = MOCK_POOL;
  const isGovernance =
    publicKey?.toBase58() === pool.governance; // mock — never matches in v1 demo

  const onSkim = () => {
    console.info('[stub] skim_protocol_fees()', { amount: pool.accrued_protocol_fees });
    // TODO: real client (issue #15)
    alert(`Stub: would call escrow::skim_protocol_fees(${pool.accrued_protocol_fees})`);
  };

  return (
    <div className="space-y-6">
      <header>
        <p className="text-xs uppercase tracking-widest text-zinc-500">Governance</p>
        <h1 className="mt-1 text-2xl font-medium text-zinc-100">Pool parameters</h1>
        <p className="mt-1 text-sm text-zinc-500">
          Authority ·{' '}
          <span className="font-mono">{shortAddr(pool.governance, 6)}</span>
          <span className="ml-2 text-[11px] uppercase tracking-widest text-zinc-600">
            (Squads vault)
          </span>
        </p>
      </header>

      <div className="grid grid-cols-3 gap-6">
        <Card>
          <Stat
            label="Timelock"
            value={`${pool.timelock_seconds / 3600}h`}
            hint="Param-change delay"
          />
        </Card>
        <Card>
          <Stat
            label="Pause state"
            value={pool.paused ? 'Paused' : 'Open'}
            hint="Init/withdraw only — issuance never gated"
            accent={pool.paused}
          />
        </Card>
        <Card>
          <Stat
            label="Accrued fees"
            value={formatUsdc(pool.accrued_protocol_fees, { showSymbol: false })}
            hint="Skimmable"
            accent
          />
        </Card>
      </div>

      <Card title="Fee curve" description="Read-only in v1 demo · propose/execute requires Squads vault signer">
        <div className="grid grid-cols-2 md:grid-cols-4 gap-x-6 gap-y-5">
          {FEE_CURVE_PARAMS.map((p) => (
            <div key={p.key} className="flex flex-col gap-0.5">
              <span className="text-[11px] uppercase tracking-wider text-zinc-500">
                {p.label}
              </span>
              <span className="tnum font-mono text-base text-zinc-100">
                {formatBps(pool.fee_curve[p.key])}
              </span>
              {p.hint && <span className="text-[11px] text-zinc-500">{p.hint}</span>}
            </div>
          ))}
          <div className="flex flex-col gap-0.5">
            <span className="text-[11px] uppercase tracking-wider text-zinc-500">
              Max advance %
            </span>
            <span className="tnum font-mono text-base text-zinc-100">
              {formatBps(pool.max_advance_pct_bps)}
            </span>
            <span className="text-[11px] text-zinc-500">Of receivable amount</span>
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-[11px] uppercase tracking-wider text-zinc-500">
              Max advance abs
            </span>
            <span className="tnum font-mono text-base text-zinc-100">
              {formatUsdc(pool.max_advance_abs, { showSymbol: false })}
            </span>
            <span className="text-[11px] text-zinc-500">Hard cap, USDC</span>
          </div>
        </div>
      </Card>

      <Card title="Pending parameters" description="Two-step timelocked proposals">
        {pool.pending_params ? (
          <div className="space-y-2 text-sm">
            <p className="text-zinc-300">
              Executes after{' '}
              <span className="font-mono text-zinc-100">
                {new Date(pool.pending_params.execute_after * 1000).toLocaleString()}
              </span>
            </p>
          </div>
        ) : (
          <p className="text-sm text-zinc-500">No pending proposal.</p>
        )}
        <div className="mt-4 flex gap-2">
          <button
            disabled={!isGovernance}
            className="h-9 px-4 rounded-md bg-zinc-800 text-zinc-300 text-sm hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed"
            title="Requires Squads vault signer"
          >
            Propose params
          </button>
          <button
            disabled={!isGovernance || !pool.pending_params}
            className="h-9 px-4 rounded-md bg-indigo-500 text-white text-sm hover:bg-indigo-400 disabled:bg-zinc-800 disabled:text-zinc-600 disabled:cursor-not-allowed"
          >
            Execute params
          </button>
        </div>
      </Card>

      <Card title="Skim protocol fees">
        <div className="flex items-center justify-between gap-4">
          <p className="text-sm text-zinc-400">
            Withdraw{' '}
            <span className="font-mono text-zinc-200 tnum">
              {formatUsdc(pool.accrued_protocol_fees)}
            </span>{' '}
            to a governance-specified ATA.
          </p>
          <button
            onClick={onSkim}
            disabled={pool.accrued_protocol_fees === 0n}
            className="h-9 px-4 rounded-md bg-indigo-500 text-white text-sm font-medium hover:bg-indigo-400 disabled:bg-zinc-800 disabled:text-zinc-600"
          >
            Skim
          </button>
        </div>
      </Card>
    </div>
  );
};

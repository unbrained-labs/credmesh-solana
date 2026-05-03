import { useMemo, useState, type FC, type FormEvent } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { Card } from '../components/Card';
import { Stat } from '../components/Stat';
import { MOCK_ADVANCES, MOCK_POOL } from '../lib/mock-data';
import { formatUsdc, shortAddr } from '../lib/format';

const SOURCE_LABEL: Record<number, string> = {
  0: 'Worker',
  1: 'Ed25519',
  2: 'X402',
};

export const AgentView: FC = () => {
  const { publicKey } = useWallet();
  const advances = MOCK_ADVANCES;

  const totalOwed = advances.reduce((s, a) => s + a.principal + a.fee_owed, 0n);
  const outstandingCount = advances.filter((a) => a.state === 'Issued').length;

  return (
    <div className="space-y-6">
      <header>
        <p className="text-xs uppercase tracking-widest text-zinc-500">Agent</p>
        <h1 className="mt-1 text-2xl font-medium text-zinc-100">Receivable advances</h1>
        <p className="mt-1 text-sm text-zinc-500">
          Connected agent ·{' '}
          <span className="font-mono">{publicKey ? shortAddr(publicKey.toBase58(), 6) : '—'}</span>
        </p>
      </header>

      <div className="grid grid-cols-3 gap-6">
        <Card>
          <Stat
            label="Outstanding"
            value={outstandingCount.toString()}
            hint="Active advances"
          />
        </Card>
        <Card>
          <Stat
            label="Owed"
            value={formatUsdc(totalOwed, { showSymbol: false })}
            hint="Principal + fees"
          />
        </Card>
        <Card>
          <Stat
            label="Cap per advance"
            value={`${formatUsdc(MOCK_POOL.max_advance_abs, { showSymbol: false })}`}
            hint={`${(MOCK_POOL.max_advance_pct_bps / 100).toFixed(0)}% of receivable`}
          />
        </Card>
      </div>

      <RequestAdvanceForm walletConnected={!!publicKey} />

      <Card title="Outstanding advances" description="Mock data — TODO: real client (issue #15)">
        {advances.length === 0 ? (
          <p className="text-sm text-zinc-500">No advances yet.</p>
        ) : (
          <ul className="divide-y divide-zinc-800/80 -mx-1">
            {advances.map((a) => (
              <AdvanceRow key={a.pubkey} a={a} />
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
};

const AdvanceRow: FC<{ a: (typeof MOCK_ADVANCES)[number] }> = ({ a }) => {
  const now = Math.floor(Date.now() / 1000);
  const daysToExpiry = Math.round((a.expires_at - now) / 86400);
  const overdue = daysToExpiry < 0;
  const settleable = a.state === 'Issued';
  const liquidatable = a.state === 'Issued' && daysToExpiry < -14;

  const onSettle = () => {
    console.info('[stub] claim_and_settle()', { advance: a.pubkey });
    // TODO: real client (issue #15)
    alert(`Stub: would call escrow::claim_and_settle(${a.pubkey})`);
  };
  const onLiquidate = () => {
    console.info('[stub] liquidate()', { advance: a.pubkey });
    // TODO: real client (issue #15)
    alert(`Stub: would call escrow::liquidate(${a.pubkey})`);
  };

  return (
    <li className="px-1 py-3 flex items-center gap-4">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm text-zinc-200">{shortAddr(a.pubkey, 6)}</span>
          <span className="text-[10px] uppercase tracking-widest px-1.5 py-0.5 rounded border border-zinc-800 text-zinc-400">
            {SOURCE_LABEL[a.source_kind]}
          </span>
          <span
            className={[
              'text-[10px] uppercase tracking-widest px-1.5 py-0.5 rounded',
              a.state === 'Issued'
                ? 'bg-indigo-500/10 text-indigo-300 border border-indigo-500/30'
                : a.state === 'Settled'
                ? 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/30'
                : 'bg-rose-500/10 text-rose-300 border border-rose-500/30',
            ].join(' ')}
          >
            {a.state}
          </span>
        </div>
        <div className="mt-0.5 text-xs text-zinc-500 font-mono truncate">
          receivable {shortAddr(a.receivable_id, 8)}
        </div>
      </div>
      <div className="text-right">
        <div className="tnum font-mono text-sm text-zinc-200">
          {formatUsdc(a.principal, { showSymbol: false })}
          <span className="text-zinc-500"> + {formatUsdc(a.fee_owed, { showSymbol: false })} fee</span>
        </div>
        <div className={`text-xs ${overdue ? 'text-amber-400' : 'text-zinc-500'}`}>
          {overdue ? `${-daysToExpiry}d overdue` : `${daysToExpiry}d to expiry`}
        </div>
      </div>
      <div className="flex gap-1.5">
        <button
          onClick={onSettle}
          disabled={!settleable}
          className="h-8 px-3 rounded-md bg-zinc-800 text-zinc-100 text-xs hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          Settle
        </button>
        <button
          onClick={onLiquidate}
          disabled={!liquidatable}
          className="h-8 px-3 rounded-md bg-zinc-900 border border-rose-900/60 text-rose-300 text-xs hover:bg-rose-950/40 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          Liquidate
        </button>
      </div>
    </li>
  );
};

const RequestAdvanceForm: FC<{ walletConnected: boolean }> = ({ walletConnected }) => {
  const [receivableId, setReceivableId] = useState('');
  const [amount, setAmount] = useState('');
  const [sourceKind, setSourceKind] = useState<0 | 1 | 2>(0);
  const [pending, setPending] = useState(false);

  const validReceivable = useMemo(
    () => /^0x[0-9a-fA-F]{64}$/.test(receivableId),
    [receivableId],
  );

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!validReceivable) return;
    setPending(true);
    console.info('[stub] request_advance()', {
      receivable_id: receivableId,
      amount,
      source_kind: sourceKind,
    });
    // TODO: real client (issue #15) — build request_advance ix via Codama client.
    // For source_kind 1/2 will need to insert ed25519 verify ix above this one.
    await new Promise((r) => setTimeout(r, 600));
    setPending(false);
    alert(
      `Stub: escrow::request_advance(${amount} USDC, src=${SOURCE_LABEL[sourceKind]})`,
    );
  };

  return (
    <Card title="Request advance" description="Issues an Advance PDA against a verified receivable">
      <form onSubmit={onSubmit} className="grid md:grid-cols-2 gap-4">
        <label className="md:col-span-2">
          <span className="text-xs text-zinc-400">Receivable ID (32-byte hex)</span>
          <input
            value={receivableId}
            onChange={(e) => setReceivableId(e.target.value)}
            placeholder="0x…"
            className="mt-1 w-full rounded-md bg-zinc-950 border border-zinc-800 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-1 focus:ring-indigo-500/60 focus:border-indigo-500/60"
          />
          {receivableId && !validReceivable && (
            <span className="text-[11px] text-amber-400 mt-1 block">
              Must be 0x + 64 hex chars
            </span>
          )}
        </label>
        <label>
          <span className="text-xs text-zinc-400">Amount (USDC)</span>
          <input
            inputMode="decimal"
            value={amount}
            onChange={(e) => setAmount(e.target.value.replace(/[^0-9.]/g, ''))}
            placeholder="0.00"
            className="mt-1 w-full rounded-md bg-zinc-950 border border-zinc-800 px-3 py-2 text-sm tnum font-mono focus:outline-none focus:ring-1 focus:ring-indigo-500/60 focus:border-indigo-500/60"
          />
        </label>
        <label>
          <span className="text-xs text-zinc-400">Source</span>
          <select
            value={sourceKind}
            onChange={(e) => setSourceKind(Number(e.target.value) as 0 | 1 | 2)}
            className="mt-1 w-full rounded-md bg-zinc-950 border border-zinc-800 px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-500/60 focus:border-indigo-500/60"
          >
            <option value={0}>Worker (oracle)</option>
            <option value={1}>Ed25519 (signed receivable)</option>
            <option value={2}>X402 (allowed signer)</option>
          </select>
        </label>
        <div className="md:col-span-2">
          <button
            type="submit"
            disabled={!walletConnected || !validReceivable || !amount || pending}
            className="w-full md:w-auto h-9 px-6 rounded-md bg-indigo-500 text-white text-sm font-medium hover:bg-indigo-400 disabled:bg-zinc-800 disabled:text-zinc-600 transition-colors"
          >
            {pending ? 'Submitting…' : 'Request advance'}
          </button>
        </div>
      </form>
    </Card>
  );
};

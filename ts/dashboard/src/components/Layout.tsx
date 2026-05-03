import type { FC } from 'react';
import { NavLink, Outlet } from 'react-router-dom';
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui';

const navItems = [
  { to: '/', label: 'LP', end: true },
  { to: '/agent', label: 'Agent' },
  { to: '/governance', label: 'Governance' },
];

export const Layout: FC = () => {
  return (
    <div className="min-h-screen flex flex-col">
      <header className="border-b border-zinc-800/80 bg-zinc-950/90 backdrop-blur sticky top-0 z-10">
        <div className="max-w-6xl mx-auto px-6 h-14 flex items-center gap-8">
          <div className="flex items-center gap-2">
            <div className="size-6 rounded-md bg-indigo-500/15 ring-1 ring-indigo-500/40 grid place-items-center">
              <div className="size-2 rounded-sm bg-indigo-400" />
            </div>
            <span className="font-mono text-sm tracking-tight text-zinc-200">
              credmesh<span className="text-zinc-500">.sol</span>
            </span>
            <span className="ml-2 text-[10px] uppercase tracking-widest text-zinc-500 px-1.5 py-0.5 rounded border border-zinc-800">
              devnet
            </span>
          </div>
          <nav className="flex items-center gap-1">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  [
                    'px-3 py-1.5 rounded-md text-sm transition-colors',
                    isActive
                      ? 'bg-zinc-900 text-zinc-100'
                      : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900/60',
                  ].join(' ')
                }
              >
                {item.label}
              </NavLink>
            ))}
          </nav>
          <div className="ml-auto">
            <WalletMultiButton />
          </div>
        </div>
      </header>
      <main className="flex-1">
        <div className="max-w-6xl mx-auto px-6 py-8">
          <Outlet />
        </div>
      </main>
      <footer className="border-t border-zinc-900 text-xs text-zinc-600">
        <div className="max-w-6xl mx-auto px-6 h-10 flex items-center justify-between">
          <span>CredMesh — programmable credit for autonomous agents</span>
          <span className="font-mono">v0.1 · scaffold</span>
        </div>
      </footer>
    </div>
  );
};

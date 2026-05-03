import type { FC, ReactNode } from 'react';

interface StatProps {
  label: string;
  value: ReactNode;
  hint?: string;
  accent?: boolean;
}

export const Stat: FC<StatProps> = ({ label, value, hint, accent }) => {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[11px] uppercase tracking-wider text-zinc-500">{label}</span>
      <span
        className={[
          'tnum font-mono text-xl leading-tight',
          accent ? 'text-indigo-300' : 'text-zinc-100',
        ].join(' ')}
      >
        {value}
      </span>
      {hint && <span className="text-xs text-zinc-500">{hint}</span>}
    </div>
  );
};

import type { FC, ReactNode } from 'react';

interface CardProps {
  title?: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}

export const Card: FC<CardProps> = ({ title, description, action, children, className }) => {
  return (
    <section
      className={[
        'rounded-xl border border-zinc-800/80 bg-zinc-900/40 backdrop-blur-sm',
        'shadow-[inset_0_1px_0_0_rgba(255,255,255,0.04)]',
        className ?? '',
      ].join(' ')}
    >
      {(title || action) && (
        <header className="flex items-start justify-between gap-4 px-5 pt-5 pb-3">
          <div>
            {title && <h2 className="text-sm font-medium text-zinc-200">{title}</h2>}
            {description && (
              <p className="mt-0.5 text-xs text-zinc-500">{description}</p>
            )}
          </div>
          {action}
        </header>
      )}
      <div className="px-5 pb-5">{children}</div>
    </section>
  );
};

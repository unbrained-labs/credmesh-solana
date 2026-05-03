import type { FC } from 'react';

interface Point {
  t: number; // unix seconds
  price: number;
}

interface Props {
  points: Point[];
  height?: number;
}

export const SharePriceChart: FC<Props> = ({ points, height = 140 }) => {
  if (points.length < 2) return null;
  const width = 720;
  const padX = 8;
  const padY = 12;

  const xs = points.map((p) => p.t);
  const ys = points.map((p) => p.price);
  const xMin = Math.min(...xs);
  const xMax = Math.max(...xs);
  const yMin = Math.min(...ys);
  const yMax = Math.max(...ys);
  const yRange = yMax - yMin || 1;

  const sx = (t: number) => padX + ((t - xMin) / (xMax - xMin)) * (width - padX * 2);
  const sy = (p: number) =>
    padY + (1 - (p - yMin) / yRange) * (height - padY * 2);

  const linePath = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'}${sx(p.t).toFixed(1)},${sy(p.price).toFixed(1)}`)
    .join(' ');

  const areaPath =
    `M${sx(points[0].t).toFixed(1)},${(height - padY).toFixed(1)} ` +
    points.map((p) => `L${sx(p.t).toFixed(1)},${sy(p.price).toFixed(1)}`).join(' ') +
    ` L${sx(points[points.length - 1].t).toFixed(1)},${(height - padY).toFixed(1)} Z`;

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-auto" preserveAspectRatio="none">
      <defs>
        <linearGradient id="spc-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#818cf8" stopOpacity="0.35" />
          <stop offset="100%" stopColor="#818cf8" stopOpacity="0" />
        </linearGradient>
      </defs>
      {/* horizontal gridlines */}
      {[0.25, 0.5, 0.75].map((f) => (
        <line
          key={f}
          x1={padX}
          x2={width - padX}
          y1={padY + f * (height - padY * 2)}
          y2={padY + f * (height - padY * 2)}
          stroke="rgb(39 39 42)"
          strokeDasharray="2 4"
        />
      ))}
      <path d={areaPath} fill="url(#spc-fill)" />
      <path
        d={linePath}
        fill="none"
        stroke="#a5b4fc"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
};

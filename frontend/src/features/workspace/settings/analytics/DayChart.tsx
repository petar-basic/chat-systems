import { dayLabel } from './days';

export interface ChartSeries {
  label: string;
  fill: string;
  swatch: string;
  values: number[];
}

interface Props {
  title: string;
  days: string[];
  series: ChartSeries[];
  ceiling?: number;
  qa: string;
}

const HEIGHT = 120;
const BAR = 10;
const GAP = 4;

export function DayChart({ title, days, series, ceiling, qa }: Props) {
  const totals = days.map((_, i) => series.reduce((sum, s) => sum + (s.values[i] ?? 0), 0));
  const top = Math.max(ceiling ?? 0, ...totals, 1);
  const width = days.length * (BAR + GAP) - GAP;
  const busiest = Math.max(...totals);

  return (
    <figure className="space-y-2" data-qa={qa}>
      <figcaption className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-sm font-medium text-fg">{title}</span>
        {series.length > 1 && (
          <span className="flex flex-wrap gap-3 text-xs text-muted">
            {series.map((s) => (
              <span key={s.label} className="inline-flex items-center gap-1.5">
                <span className={`h-2.5 w-2.5 rounded-sm ${s.swatch}`} aria-hidden="true" />
                {s.label}
              </span>
            ))}
          </span>
        )}
      </figcaption>
      {busiest === 0 ? (
        <p className="rounded-lg border border-dashed border-line py-8 text-center text-sm text-muted">
          Nothing in these days yet.
        </p>
      ) : (
        <svg
          viewBox={`0 0 ${width} ${HEIGHT}`}
          preserveAspectRatio="none"
          className="h-32 w-full"
          role="img"
          aria-label={`${title}: at most ${busiest.toLocaleString()} on a day`}
        >
          {days.map((day, i) => {
            let y = HEIGHT;
            const parts = series.map((s) => s.label + ' ' + (s.values[i] ?? 0).toLocaleString()).join(', ');
            return (
              <g key={day}>
                <title>{`${dayLabel(day)}: ${parts}`}</title>
                <rect x={i * (BAR + GAP)} y={0} width={BAR} height={HEIGHT} className="fill-transparent" />
                {series.map((s) => {
                  const h = ((s.values[i] ?? 0) / top) * HEIGHT;
                  y -= h;
                  return h > 0 ? (
                    <rect key={s.label} x={i * (BAR + GAP)} y={y} width={BAR} height={h} className={s.fill} />
                  ) : null;
                })}
              </g>
            );
          })}
        </svg>
      )}
      {busiest > 0 && (
        <div className="flex justify-between text-xs text-muted" aria-hidden="true">
          <span>{dayLabel(days[0])}</span>
          <span>{dayLabel(days[days.length - 1])}</span>
        </div>
      )}
    </figure>
  );
}

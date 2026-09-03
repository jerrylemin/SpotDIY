import { useMemo } from "react";

import type { ListeningHeatmapCell } from "../../types/domain";

function formatDuration(milliseconds: number): string {
  const totalMinutes = Math.floor(milliseconds / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

const shortWeekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const longWeekdays = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

export function AnalyticsHeatmap({ cells }: { cells: ListeningHeatmapCell[] }) {
  const byKey = useMemo(
    () => new Map(cells.map((cell) => [`${cell.weekday}:${cell.hour}`, cell.listenedMs])),
    [cells],
  );
  const max = Math.max(...cells.map((cell) => cell.listenedMs), 1);

  return (
    <div aria-label="Listening heatmap" className="analytics-heatmap">
      <div className="analytics-heatmap-corner" />
      {Array.from({ length: 24 }, (_, hour) => (
        <span className="analytics-heatmap-hour" key={hour}>{hour}</span>
      ))}
      {Array.from({ length: 7 }, (_, weekday) => (
        <div className="analytics-heatmap-row" key={weekday}>
          <span className="analytics-heatmap-day">{shortWeekdays[weekday]}</span>
          {Array.from({ length: 24 }, (_, hour) => {
            const listenedMs = byKey.get(`${weekday}:${hour}`) ?? 0;
            const intensity = listenedMs === 0 ? 0 : Math.max(0.18, listenedMs / max);
            return (
              <span
                aria-label={`${longWeekdays[weekday]} ${hour}:00, ${formatDuration(listenedMs)}`}
                className="analytics-heatmap-cell"
                key={hour}
                style={{ opacity: intensity }}
                title={`${formatDuration(listenedMs)} listened`}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}

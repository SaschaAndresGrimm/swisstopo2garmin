import { useEffect, useRef, useState } from "react";
import { formatBytes } from "../state/api";
import type { TaskProgress } from "../state/api";
import type { T } from "../i18n";

/** Smoothed transfer rate, in bytes/second, from successive progress samples. */
function useRate(progress: TaskProgress | null) {
  const [rate, setRate] = useState<number | null>(null);
  const last = useRef<{ t: number; read: number } | null>(null);

  useEffect(() => {
    if (!progress) {
      last.current = null;
      setRate(null);
      return;
    }
    const now = performance.now();
    const prev = last.current;
    // Sample at most twice a second: raw chunk events are far too frequent to
    // produce a stable number.
    if (prev && now - prev.t >= 500) {
      const instant = ((progress.read - prev.read) * 1000) / (now - prev.t);
      setRate((r) => (r === null ? instant : r * 0.7 + instant * 0.3));
      last.current = { t: now, read: progress.read };
    } else if (!prev) {
      last.current = { t: now, read: progress.read };
    }
  }, [progress]);

  return rate;
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function DownloadProgress({ t, progress }: { t: T; progress: TaskProgress }) {
  const rate = useRate(progress);
  const pct = progress.total ? (progress.read / progress.total) * 100 : null;
  const remaining =
    progress.total && rate && rate > 0 ? (progress.total - progress.read) / rate : null;

  return (
    <div className="progress" role="group" aria-label={t("data.progress.label")}>
      <div
        className="track"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct === null ? undefined : Math.round(pct)}
      >
        <div
          className={pct === null ? "fill indeterminate" : "fill"}
          style={pct === null ? undefined : { width: `${pct}%` }}
        />
      </div>

      <div className="progress-lines">
        <div className="progress-main">
          <strong>{pct === null ? "…" : `${pct.toFixed(1)}%`}</strong>
          <span className="muted">
            {t("data.progress.transferred", {
              read: formatBytes(progress.read),
              total: formatBytes(progress.total),
            })}
          </span>
        </div>
        <div className="progress-meta muted small">
          <span>
            {t("data.progress.written", { written: formatBytes(progress.written) })}
          </span>
          <span>{rate ? `${formatBytes(rate)}/s` : "—"}</span>
          <span>
            {remaining === null
              ? "—"
              : t("data.progress.eta", { eta: formatDuration(remaining) })}
          </span>
          {progress.retries > 0 && (
            <span className="warn">
              {t("data.progress.retries", { n: progress.retries })}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

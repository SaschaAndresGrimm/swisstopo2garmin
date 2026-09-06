import { useCallback, useEffect, useState } from "react";
import type React from "react";
import { api, formatBytes, formatDuration, onBuildEvents } from "../state/api";
import type {
  BuildFailure,
  BuildFinished,
  BuildProgress,
  PartitionPlan,
  Recipe,
} from "../state/api";
import type { T } from "../i18n";

/** Build execution with per-stage progress and working cancellation (FR-70..FR-74). */
export function BuildStep({
  t,
  recipe,
  onDone,
  onBack,
  before,
}: {
  t: T;
  recipe: Recipe;
  onDone: (r: BuildFinished) => void;
  onBack: () => void;
  /** Slot for the recipe library, which needs the assembled recipe. */
  before?: React.ReactNode;
}) {
  const [progress, setProgress] = useState<BuildProgress | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [result, setResult] = useState<BuildFinished | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [failure, setFailure] = useState<BuildFailure | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);
  const [split, setSplit] = useState<PartitionPlan | null>(null);

  const append = useCallback((line: string) => {
    // Bounded: a long build would otherwise grow the DOM without limit.
    setLog((l) => [...l.slice(-200), line]);
  }, []);

  useEffect(() => {
    const un = onBuildEvents({
      progress: (p) => {
        setProgress(p);
        append(`${p.stageLabel}${p.detail ? ` — ${p.detail}` : ""}`);
      },
      done: (d) => {
        setProgress(null);
        setTaskId(null);
        setResult(d);
        onDone(d);
      },
      error: (e) => {
        setError(e.message);
        setProgress(null);
        setTaskId(null);
      },
      failed: setFailure,
      onFailure: setError,
    });
    return () => {
      void un.then((fns) => fns.forEach((f) => f()));
    };
  }, [append, onDone]);

  // Checked before offering the button, because a user who learns the area is too big
  // only when the build fails has waited minutes for nothing.
  useEffect(() => {
    let live = true;
    api
      .partitionPlan(recipe)
      .then((p) => live && setSplit(p.parts > 1 ? p : null))
      .catch(() => live && setSplit(null));
    return () => {
      live = false;
    };
  }, [recipe]);

  const start = async () => {
    setError(null);
    setFailure(null);
    setResult(null);
    setLog([]);
    try {
      setTaskId(await api.startBuild(recipe));
    } catch (e) {
      setError(String(e));
    }
  };

  // Weighted by how long each stage usually takes, not by stage count: the contour
  // stage alone is nearly half a build, so counting stages made the bar lurch.
  const pct = progress ? progress.overall * 100 : 0;

  return (
    <section className="screen">
      <h2>{t("step.build")}</h2>

      {before}

      <dl className="facts">
        <div>
          <dt>{t("build.device")}</dt>
          <dd className="mono">{recipe.deviceId}</dd>
        </div>
        <div>
          <dt>{t("build.preset")}</dt>
          <dd>{t(`preset.${recipe.preset}`)}</dd>
        </div>
        <div>
          <dt>{t("build.contours")}</dt>
          <dd>{recipe.contours.intervalM > 0 ? `${recipe.contours.intervalM} m` : t("common.off")}</dd>
        </div>
        <div>
          <dt>{t("content.relief")}</dt>
          <dd>{t(`content.relief.${recipe.relief}`)}</dd>
        </div>
        <div>
          <dt>{t("build.palette")}</dt>
          <dd>{t(`content.palette.${recipe.palette}`)}</dd>
        </div>
      </dl>

      {split && (
        <div className="notice">
          <strong>{t("build.splitTitle", { n: split.parts })}</strong>
          <p className="small">{split.reason}</p>
          {!split.fits && <p className="error small">{t("build.splitCannotFit")}</p>}
          <ul className="small">
            {split.partNames.map((n, i) => (
              <li key={n}>
                {n} — {formatBytes(split.partBytes[i] ?? 0)}
              </li>
            ))}
          </ul>
          <p className="muted small">{t("build.splitHint")}</p>
        </div>
      )}

      <div className="row">
        {!taskId && !result && (
          <button type="button" className="primary" onClick={() => void start()}>
            {t("build.start")}
          </button>
        )}
        {taskId && (
          <button type="button" onClick={() => void api.cancel(taskId)}>
            {t("data.cancel")}
          </button>
        )}
        {result && (
          <button type="button" onClick={() => void start()}>{t("build.rebuild")}</button>
        )}
        <button type="button" onClick={onBack} disabled={!!taskId}>{t("common.back")}</button>
      </div>

      {progress && (
        <div className="progress">
          <div className="track" role="progressbar" aria-valuemin={0} aria-valuemax={100}
               aria-valuenow={Math.round(pct)}>
            <div className="fill" style={{ width: `${pct}%` }} />
          </div>
          <div className="progress-lines">
            <div className="progress-main">
              <strong>
                {t("build.stageOf", {
                  i: progress.stageIndex + 1,
                  n: progress.stageCount,
                })}
              </strong>
              <span>{progress.stageLabel}</span>
            </div>
            <div className="progress-meta muted small" aria-live="polite">
              <span>{progress.detail}</span>
              <span className="spacer" />
              <span>{t("build.elapsed", { time: formatDuration(progress.elapsedSeconds) })}</span>
              <span>
                {progress.etaSeconds !== null
                  ? t("build.remaining", { time: formatDuration(progress.etaSeconds) })
                  : t("build.estimating")}
              </span>
            </div>
          </div>
        </div>
      )}

      {result && (
        <div className="notice" role="status">
          <strong>{t("build.finished")}</strong>
          <dl className="facts">
            <div>
              <dt>{t("build.took")}</dt>
              <dd>{formatDuration(result.seconds)}</dd>
            </div>
            <div>
              <dt>{t("build.size")}</dt>
              <dd>{formatBytes(result.bytes)}</dd>
            </div>
            <div>
              <dt>{t("build.tiles")}</dt>
              <dd>{result.tileCount}</dd>
            </div>
            <div>
              <dt>{t("build.features")}</dt>
              <dd>{result.features.toLocaleString()}</dd>
            </div>
            <div>
              <dt>{t("build.contourLines")}</dt>
              <dd>{result.contourLines.toLocaleString()}</dd>
            </div>
            <div>
              <dt>{t("content.relief")}</dt>
              <dd>{result.hasDem ? t("common.yes") : t("common.no")}</dd>
            </div>
          </dl>
          <p className="mono small">{result.gmapsupp}</p>
          {/* Named, because it is what lets a map on a device be traced back to the
              releases and tools that made it. */}
          {result.manifest && (
            <p className="muted small">
              {t("build.manifest")} <span className="mono">{result.manifest}</span>
            </p>
          )}
          {result.warnings.length > 0 && (
            <ul className="warnings">
              {result.warnings.map((w) => (
                <li key={w} className="small">{w}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      {error && (
        <div className="notice error-box" role="alert">
          <strong>{failure ? failure.summary : t("build.failed")}</strong>
          {failure?.suggestion && <p className="small">{failure.suggestion}</p>}
          {/* Said plainly rather than dressed up as an explanation. */}
          {failure && !failure.recognised && (
            <p className="muted small">{t("build.unrecognised")}</p>
          )}
          <details className="notes">
            <summary>{t("build.details")}</summary>
            <pre className="log">{failure?.detail ?? error}</pre>
          </details>
          <div className="row tight">
            <button
              type="button"
              onClick={() => {
                // Everything a bug report needs, in one paste.
                const report = [
                  `swisstopo2garmin build failure`,
                  `device: ${recipe.deviceId}`,
                  `preset: ${recipe.preset}  contours: ${recipe.contours.intervalM} m  relief: ${recipe.relief}`,
                  `palette: ${recipe.palette}  slope: ${recipe.slopeClasses}`,
                  `area: ${JSON.stringify(recipe.area)}`,
                  ``,
                  failure?.detail ?? error,
                  ``,
                  ...log.slice(-40),
                ].join("\n");
                void navigator.clipboard.writeText(report).then(
                  () => setCopied(true),
                  () => setCopied(false),
                );
              }}
            >
              {copied ? t("build.copied") : t("build.copyDiagnostics")}
            </button>
          </div>
        </div>
      )}

      {log.length > 0 && (
        <details className="notes" open={!!taskId}>
          <summary>{t("build.log")}</summary>
          <pre className="log">{log.join("\n")}</pre>
        </details>
      )}
    </section>
  );
}

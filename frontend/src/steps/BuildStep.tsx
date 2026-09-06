import { useCallback, useEffect, useState } from "react";
import type React from "react";
import { api, formatBytes, onBuildEvents } from "../state/api";
import type { BuildFinished, BuildProgress, Recipe } from "../state/api";
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
  const [log, setLog] = useState<string[]>([]);

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
      onFailure: setError,
    });
    return () => {
      void un.then((fns) => fns.forEach((f) => f()));
    };
  }, [append, onDone]);

  const start = async () => {
    setError(null);
    setResult(null);
    setLog([]);
    try {
      setTaskId(await api.startBuild(recipe));
    } catch (e) {
      setError(String(e));
    }
  };

  const pct = progress
    ? ((progress.stageIndex + (progress.fraction ?? 0)) / progress.stageCount) * 100
    : 0;

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
      </dl>

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
            <div className="progress-meta muted small">
              <span>{progress.detail}</span>
            </div>
          </div>
        </div>
      )}

      {result && (
        <div className="notice">
          <strong>{t("build.finished")}</strong>
          <dl className="facts">
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
        <div className="notice error-box">
          <strong>{t("build.failed")}</strong>
          <p className="small mono">{error}</p>
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

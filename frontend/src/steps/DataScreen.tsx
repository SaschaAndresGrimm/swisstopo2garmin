import { useCallback, useEffect, useState } from "react";
import { api, formatBytes, onTaskEvents, SOURCES } from "../state/api";
import type { CacheStatus, TaskProgress } from "../state/api";
import { SourceRow } from "./SourceRow";
import { DataLocation } from "../components/DataLocation";
import type { T } from "../i18n";

export function DataScreen({ t }: { t: T }) {
  const [status, setStatus] = useState<CacheStatus | null>(null);
  const [progress, setProgress] = useState<TaskProgress | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Bumped whenever the cache changes, to re-check every source's `cached` flag.
  const [refreshToken, setRefreshToken] = useState(0);

  const refreshCache = useCallback(async () => {
    try {
      setStatus(await api.cacheStatus());
      setRefreshToken((n) => n + 1);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const finish = useCallback(() => {
    setProgress(null);
    setTaskId(null);
    void refreshCache();
  }, [refreshCache]);

  // refreshCache is stable (useCallback with no deps), so this runs once on mount.
  useEffect(() => {
    void refreshCache();
  }, [refreshCache]);

  useEffect(() => {
    const un = onTaskEvents({
      progress: setProgress,
      done: finish,
      error: (e) => {
        setError(e.message);
        setProgress(null);
        setTaskId(null);
        void refreshCache();
      },
      onFailure: setError,
    });
    return () => {
      void un.then((fns) => fns.forEach((f) => f()));
    };
  }, [finish, refreshCache]);

  // Safety net: events are the fast path, but a dropped event must not leave the
  // screen stuck on a stale progress bar.
  useEffect(() => {
    if (!taskId) return;
    const h = window.setInterval(() => {
      void api
        .cacheStatus()
        .then((s) => {
          setStatus(s);
          const collection = taskId.split(":")[0];
          if (s.entries.some((e) => e.collection === collection && e.bytes > 0)) {
            finish();
          }
        })
        .catch(() => undefined);
    }, 3000);
    return () => window.clearInterval(h);
  }, [taskId, finish]);

  return (
    <section className="screen">
      <h2>{t("data.title")}</h2>
      <p className="muted">{t("data.intro")}</p>

      {status && (
        <dl className="facts">
          <div>
            <dt>{t("data.cacheLocation")}</dt>
            <dd className="mono">{status.root}</dd>
          </div>
          <div>
            <dt>{t("data.totalSize")}</dt>
            <dd>{formatBytes(status.totalBytes)}</dd>
          </div>
          <div>
            <dt>{t("data.free")}</dt>
            <dd>{formatBytes(status.freeBytes)}</dd>
          </div>
        </dl>
      )}

      <DataLocation t={t} onChanged={() => void refreshCache()} />

      <p className="muted small">{t("data.inflateNote")}</p>

      {(["base", "winter", "cycling"] as const).map((group) => (
        <div key={group}>
          <h3 className="grouphead">{t(`sources.group.${group}`)}</h3>
          <div className="sources">
            {SOURCES.filter((s) => s.group === group).map((s) => (
              <SourceRow
                key={s.id}
                t={t}
                collection={s.id}
                label={t(s.key)}
                progress={progress}
                activeTask={taskId}
                onStarted={setTaskId}
                onCacheChanged={() => void refreshCache()}
                refreshToken={refreshToken}
              />
            ))}
          </div>
        </div>
      ))}

      {error && <p className="error">{t("data.error", { message: error })}</p>}
    </section>
  );
}

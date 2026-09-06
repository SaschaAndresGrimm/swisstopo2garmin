import { useCallback, useEffect, useState } from "react";
import { api, formatBytes } from "../state/api";
import type { ReleaseInfo, TaskProgress } from "../state/api";
import { DownloadProgress } from "../components/Progress";
import type { T } from "../i18n";

export function SourceRow({
  t,
  collection,
  label,
  progress,
  activeTask,
  onStarted,
  onCacheChanged,
  refreshToken,
}: {
  t: T;
  collection: string;
  label: string;
  progress: TaskProgress | null;
  activeTask: string | null;
  onStarted: (taskId: string) => void;
  onCacheChanged: () => void;
  refreshToken: number;
}) {
  const [release, setRelease] = useState<ReleaseInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setRelease(await api.latestRelease(collection));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [collection]);

  // Re-check whenever the cache changes, so `cached` reflects reality.
  useEffect(() => {
    void check();
  }, [check, refreshToken]);

  const mine = activeTask?.startsWith(`${collection}:`) ?? false;

  return (
    <article className="source">
      <header className="source-head">
        <div>
          <h3>{label}</h3>
          <p className="muted small mono">{collection}</p>
        </div>
        <div className="row tight">
          {release && !release.cached && !mine && (
            <button type="button" className="primary" disabled={busy} onClick={() => {
              setError(null);
              api.acquire(collection).then(onStarted).catch((e) => setError(String(e)));
            }}>
              {t("data.download")}
            </button>
          )}
          {mine && activeTask && (
            <button type="button" onClick={() => void api.cancel(activeTask)}>
              {t("data.cancel")}
            </button>
          )}
          {release?.cached && !mine && (
            <button type="button" onClick={() => {
              void api.remove(collection, release.item).then(onCacheChanged);
            }}>
              {t("data.remove")}
            </button>
          )}
        </div>
      </header>

      {busy && !release && <p className="muted small">{t("data.checking")}</p>}

      {release && (
        <p className="small">
          {t("data.available", { release: release.item })}
          {release.archiveBytes ? ` · ${formatBytes(release.archiveBytes)}` : ""}
          {release.memberBytes ? ` → ${formatBytes(release.memberBytes)}` : ""}
          {release.cached ? <span className="badge">{t("data.cached")}</span> : null}
        </p>
      )}

      {mine && progress && <DownloadProgress t={t} progress={progress} />}
      {error && <p className="error small">{t("data.error", { message: error })}</p>}
    </article>
  );
}

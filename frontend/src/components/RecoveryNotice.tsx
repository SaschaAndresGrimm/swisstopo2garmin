import { useCallback, useEffect, useState } from "react";
import { api, type InterruptedBuild, type Recipe } from "../state/api";
import type { T } from "../i18n";

/** Bytes as the user's file manager would show them. */
function gb(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
}

/**
 * Builds the app was running when it last stopped (SPEC.md §12, "App killed mid-build").
 *
 * Two things are left behind by a crash, a forced quit or a flat battery: a work
 * directory holding hundreds of megabytes, and Java child processes that outlive their
 * parent and keep a core busy indefinitely. Neither is visible to the user — the disk
 * quietly fills and the laptop quietly runs hot — so this says what happened and offers
 * the two things worth doing about it.
 *
 * Resuming is offered only as what it is: the cached region survived, so the expensive
 * stages are skipped. It is not a continuation from the exact byte the crash reached,
 * and the wording does not claim to be.
 */
export function RecoveryNotice({
  t,
  onResume,
}: {
  t: T;
  onResume: (recipe: Recipe) => void;
}) {
  const [found, setFound] = useState<InterruptedBuild[]>([]);
  const [dismissed, setDismissed] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    // Once, at startup. A build started in this session cannot be interrupted while
    // this session is the one running it.
    void api.interruptedBuilds().then(setFound).catch(() => setFound([]));
  }, []);

  const discard = useCallback(async (workDir: string) => {
    setBusy(workDir);
    try {
      await api.discardInterrupted(workDir);
      setFound((f) => f.filter((b) => b.workDir !== workDir));
    } finally {
      setBusy(null);
    }
  }, []);

  if (dismissed || found.length === 0) return null;
  const strays = found.reduce((n, b) => n + b.strayProcesses, 0);

  return (
    <div className="notice" role="status">
      <strong>{t("recovery.title", { count: found.length })}</strong>
      {strays > 0 && <p className="small">{t("recovery.strays", { count: strays })}</p>}
      <ul>
        {found.map((b) => (
          <li key={b.workDir}>
            {b.recipeName || t("recovery.unnamed")}{" "}
            <span className="muted small">{gb(b.bytes)}</span>
            <div className="row">
              {b.resumable && (
                <button type="button" onClick={() => onResume(b.recipe)}>
                  {t("recovery.resume")}
                </button>
              )}
              <button
                type="button"
                className="link"
                disabled={busy === b.workDir}
                onClick={() => void discard(b.workDir)}
              >
                {t("recovery.discard")}
              </button>
            </div>
            {b.resumable && <p className="muted small">{t("recovery.resumeHint")}</p>}
          </li>
        ))}
      </ul>
      <button type="button" className="link" onClick={() => setDismissed(true)}>
        {t("recovery.later")}
      </button>
    </div>
  );
}

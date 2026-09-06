import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, formatBytes } from "../state/api";
import type { DataLocation as Location } from "../state/api";
import type { T } from "../i18n";

/**
 * Where datasets and build working files live (SPEC.md FR-C2).
 *
 * Worth a first-class control rather than a buried preference: swissTLM3D alone is
 * 10.0 GB inflated, and a laptop with a small internal disk needs this pointed at an
 * external volume *before* the first download, not after four gigabytes have landed.
 */
export function DataLocation({ t, onChanged }: { t: T; onChanged: () => void }) {
  const [loc, setLoc] = useState<Location | null>(null);
  const [candidate, setCandidate] = useState<Location | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoc(await api.dataLocation());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const choose = async () => {
    setError(null);
    try {
      const picked = await open({ directory: true, multiple: false, title: t("data.chooseTitle") });
      if (typeof picked !== "string") return;
      // Report on the directory before committing, so free space is visible first.
      setCandidate(await api.inspectDataLocation(picked));
    } catch (e) {
      setError(String(e));
    }
  };

  // Both are re-derivable: elevation tiles re-download, build files rebuild. Neither
  // touches an acquired dataset or a saved recipe.
  const clear = async (what: "elevation" | "builds") => {
    setError(null);
    try {
      setLoc(what === "elevation" ? await api.clearElevationCache() : await api.clearBuildFiles());
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  const apply = async (path: string | null) => {
    setError(null);
    try {
      setLoc(await api.setDataLocation(path));
      setCandidate(null);
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!loc) return null;

  return (
    <div className="notice">
      <strong>{t("data.location")}</strong>
      <dl className="facts">
        <div>
          <dt>{t("data.locationPath")}</dt>
          <dd className="mono small">{loc.path}</dd>
        </div>
        <div>
          <dt>{t("data.used")}</dt>
          <dd>{formatBytes(loc.usedBytes)}</dd>
        </div>
        <div>
          <dt>{t("data.freeHere")}</dt>
          <dd className={loc.freeBytes !== null && loc.freeBytes < 12e9 ? "error" : ""}>
            {loc.freeBytes !== null ? formatBytes(loc.freeBytes) : "—"}
          </dd>
        </div>
      </dl>

      {/* Broken down because the elevation cache grows without bound as areas are
          built, and a single total hides that until the disk is full. */}
      {loc.usedBytes > 0 && (
        <dl className="facts small">
          <div>
            <dt>{t("data.usedDatasets")}</dt>
            <dd>{formatBytes(loc.datasetBytes)}</dd>
          </div>
          <div>
            <dt>{t("data.usedElevation")}</dt>
            <dd>{formatBytes(loc.elevationBytes)}</dd>
          </div>
          <div>
            <dt>{t("data.usedBuilds")}</dt>
            <dd>{formatBytes(loc.buildBytes)}</dd>
          </div>
          <div>
            <dt>{t("data.usedOther")}</dt>
            <dd>{formatBytes(loc.otherBytes)}</dd>
          </div>
        </dl>
      )}

      {(loc.elevationBytes > 0 || loc.buildBytes > 0) && (
        <div className="row tight">
          {loc.elevationBytes > 0 && (
            <button type="button" onClick={() => void clear("elevation")}>
              {t("data.clearElevation", { size: formatBytes(loc.elevationBytes) })}
            </button>
          )}
          {loc.buildBytes > 0 && (
            <button type="button" onClick={() => void clear("builds")}>
              {t("data.clearBuilds", { size: formatBytes(loc.buildBytes) })}
            </button>
          )}
        </div>
      )}
      {(loc.elevationBytes > 0 || loc.buildBytes > 0) && (
        <p className="muted small">{t("data.clearHint")}</p>
      )}

      <p className="muted small">{t("data.spaceHint")}</p>
      {loc.fromEnvironment && <p className="error small">{t("data.envOverride")}</p>}

      <div className="row tight">
        <button type="button" onClick={() => void choose()} disabled={loc.fromEnvironment}>
          {t("data.choose")}
        </button>
        {!loc.isDefault && (
          <button type="button" onClick={() => void apply(null)} disabled={loc.fromEnvironment}>
            {t("data.useDefault")}
          </button>
        )}
      </div>

      {candidate && (
        <div className="notice">
          <strong>{t("data.confirmMove")}</strong>
          <dl className="facts">
            <div>
              <dt>{t("data.locationPath")}</dt>
              <dd className="mono small">{candidate.path}</dd>
            </div>
            <div>
              <dt>{t("data.freeHere")}</dt>
              <dd className={candidate.freeBytes !== null && candidate.freeBytes < 12e9 ? "error" : ""}>
                {candidate.freeBytes !== null ? formatBytes(candidate.freeBytes) : "—"}
              </dd>
            </div>
          </dl>
          {/* Nothing is copied: saying so beats a settings change that silently
              appears to lose every dataset already downloaded. */}
          {loc.usedBytes > 0 && <p className="small">{t("data.noMove", { path: loc.path })}</p>}
          <div className="row tight">
            <button type="button" className="primary" onClick={() => void apply(candidate.path)}>
              {t("data.useThis")}
            </button>
            <button type="button" onClick={() => setCandidate(null)}>{t("common.cancel")}</button>
          </div>
        </div>
      )}

      {error && <p className="error small">{error}</p>}
    </div>
  );
}

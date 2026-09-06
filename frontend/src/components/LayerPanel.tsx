import { useEffect, useMemo, useState } from "react";
import { api } from "../state/api";
import type { LayerInfo, PresetId } from "../state/api";
import type { T } from "../i18n";

/**
 * Per-layer opt-out (SPEC.md FR-51).
 *
 * The recipe stores exclusions rather than inclusions, so a future release that adds a
 * layer switches it on for everyone instead of leaving old recipes silently missing it.
 *
 * There is no hiking-trails toggle: Swiss trails are an attribute of the road layer
 * (`tlm:wanderwege`), so switching them off separately is not possible. The panel says
 * so rather than offering a control that would not work.
 */
export function LayerPanel({
  t,
  preset,
  excluded,
  onExcluded,
}: {
  t: T;
  preset: PresetId;
  excluded: string[];
  onExcluded: (ids: string[]) => void;
}) {
  const [layers, setLayers] = useState<LayerInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listLayers().then(setLayers).catch((e) => setError(String(e)));
  }, []);

  // Only the layers this preset actually extracts; the rest would be dead controls.
  const active = useMemo(
    () => layers.filter((l) => l.presets.includes(preset)),
    [layers, preset],
  );
  const groups = useMemo(() => {
    const seen: string[] = [];
    for (const l of active) if (!seen.includes(l.group)) seen.push(l.group);
    return seen;
  }, [active]);

  const toggle = (id: string, on: boolean) =>
    onExcluded(on ? excluded.filter((x) => x !== id) : [...excluded, id]);

  const setGroup = (group: string, on: boolean) => {
    const ids = active.filter((l) => l.group === group).map((l) => l.id);
    onExcluded(on ? excluded.filter((x) => !ids.includes(x)) : [...new Set([...excluded, ...ids])]);
  };

  if (error) return <p className="error">{t("data.error", { message: error })}</p>;
  if (active.length === 0) return null;

  const offCount = active.filter((l) => excluded.includes(l.id)).length;

  return (
    <details className="notes layers">
      <summary>
        {t("layers.title")}
        {offCount > 0 && <span className="badge">{t("layers.offCount", { n: offCount })}</span>}
      </summary>

      <p className="muted small">{t("layers.intro")}</p>

      {groups.map((g) => {
        const items = active.filter((l) => l.group === g);
        const allOn = items.every((l) => !excluded.includes(l.id));
        return (
          <div key={g} className="layer-group">
            <div className="layer-group-head">
              <strong className="small">{t(`layers.group.${g}`)}</strong>
              <button type="button" className="link" onClick={() => setGroup(g, !allOn)}>
                {allOn ? t("layers.noneOfGroup") : t("layers.allOfGroup")}
              </button>
            </div>
            {items.map((l) => (
              <label key={l.id} className="check">
                <input
                  type="checkbox"
                  checked={!excluded.includes(l.id)}
                  onChange={(e) => toggle(l.id, e.target.checked)}
                />
                <span>{t(`layer.${l.id}`)}</span>
                <span className="mono muted small">{l.id}</span>
              </label>
            ))}
          </div>
        );
      })}

      <p className="muted small">{t("layers.trailsNote")}</p>
      {excluded.length > 0 && (
        <div className="row tight">
          <button type="button" onClick={() => onExcluded([])}>{t("layers.reset")}</button>
        </div>
      )}
    </details>
  );
}

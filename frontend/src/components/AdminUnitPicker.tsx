import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../state/api";
import type { AdminLevel, AdminUnitInfo, AreaSelection } from "../state/api";
import type { T } from "../i18n";

const LEVELS: AdminLevel[] = ["canton", "district", "commune"];

/**
 * Select cantons, districts or communes, optionally buffered (SPEC.md FR-33, FR-34).
 *
 * The whole level is fetched once and filtered here rather than queried per keystroke:
 * 2,123 communes is a small list in memory and a slow round trip per character.
 *
 * The canton is always shown beside the name because commune names are not unique —
 * `Greifensee` is two of them — and picking the wrong one silently builds the wrong map,
 * which is exactly the mistake the place search was fixed for once already.
 */
export function AdminUnitPicker({
  t,
  area,
  onArea,
}: {
  t: T;
  area: AreaSelection | null;
  onArea: (a: AreaSelection) => void;
}) {
  const [level, setLevel] = useState<AdminLevel>("canton");
  const [units, setUnits] = useState<AdminUnitInfo[]>([]);
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<number[]>([]);
  const [bufferKm, setBufferKm] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setError(null);
    api
      .listAdminUnits(level)
      .then((u) => {
        if (!live) return;
        setUnits(u);
        setChosen([]);
      })
      .catch((e) => live && setError(String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [level]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? units.filter(
          (u) => u.name.toLowerCase().includes(q) || u.canton.toLowerCase().includes(q),
        )
      : units;
    // Long lists are unusable in full; the search narrows them.
    return list.slice(0, 60);
  }, [units, query]);

  const apply = useCallback(
    async (numbers: number[], km: number) => {
      if (numbers.length === 0) return;
      setError(null);
      try {
        const [minE, minN, maxE, maxN] = await api.adminExtent(level, numbers, km);
        const picked = units.filter((u) => numbers.includes(u.number));
        onArea({
          kind: "adminUnits",
          level,
          numbers,
          names: picked.map((u) => u.name),
          bufferKm: km,
          minE,
          minN,
          maxE,
          maxN,
        });
      } catch (e) {
        setError(String(e));
      }
    },
    [level, units, onArea],
  );

  const toggle = (number: number) => {
    const next = chosen.includes(number)
      ? chosen.filter((n) => n !== number)
      : [...chosen, number];
    setChosen(next);
    void apply(next, bufferKm);
  };

  const selected = units.filter((u) => chosen.includes(u.number));
  const totalKm2 = selected.reduce((sum, u) => sum + u.areaKm2, 0);
  const active = area?.kind === "adminUnits";

  return (
    <div className="field">
      <label htmlFor="admin-level">{t("admin.title")}</label>
      <div className="row tight" id="admin-level">
        {LEVELS.map((l) => (
          <button
            key={l}
            type="button"
            className={level === l ? "current" : ""}
            aria-pressed={level === l}
            onClick={() => setLevel(l)}
          >
            {t(`admin.level.${l}`)}
          </button>
        ))}
      </div>

      {error && <p className="error small">{error}</p>}
      {loading && <p className="muted small">{t("common.loading")}</p>}

      {!error && !loading && (
        <>
          <input
            id="admin-filter"
            aria-label={t("admin.filter", { n: units.length })}
            value={query}
            placeholder={t("admin.filter", { n: units.length })}
            onChange={(e) => setQuery(e.target.value)}
          />

          <p className="muted small" aria-live="polite">
            {t("admin.matches", { n: matches.length })}
          </p>
          <ul className="units">
            {matches.map((u) => (
              <li key={u.number}>
                <label className="check">
                  <input
                    type="checkbox"
                    checked={chosen.includes(u.number)}
                    onChange={() => toggle(u.number)}
                  />
                  <span>{u.name}</span>
                  {/* Not decoration: commune names repeat, and the canton is what
                      tells them apart. */}
                  {u.canton && u.canton !== u.name && (
                    <span className="muted small">{u.canton}</span>
                  )}
                  <span className="muted small">{u.areaKm2.toFixed(0)} km²</span>
                </label>
              </li>
            ))}
            {units.length > matches.length && (
              <li className="muted small">
                {t("admin.more", { n: units.length - matches.length })}
              </li>
            )}
          </ul>

          {selected.length > 0 && (
            <>
              <p className="small">
                {t("admin.selected", {
                  n: selected.length,
                  km2: totalKm2.toFixed(0),
                })}
              </p>
              <label htmlFor="admin-buffer">{t("admin.buffer", { km: bufferKm })}</label>
              <input
                id="admin-buffer"
                type="range"
                min={0}
                max={20}
                step={0.5}
                value={bufferKm}
                onChange={(e) => {
                  const km = Number(e.target.value);
                  setBufferKm(km);
                  void apply(chosen, km);
                }}
              />
              <p className="muted small">{t("admin.bufferHint")}</p>
            </>
          )}
          {active && selected.length === 0 && (
            <p className="muted small">{t("admin.noneSelected")}</p>
          )}
        </>
      )}
    </div>
  );
}

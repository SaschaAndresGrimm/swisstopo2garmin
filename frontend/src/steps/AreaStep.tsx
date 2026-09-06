import { useCallback, useEffect, useState } from "react";
import { api, formatBytes } from "../state/api";
import type { AreaInfo, AreaSelection, PlaceMatch } from "../state/api";
import { AreaMap, type DrawnBox } from "../map/AreaMap";
import type { T } from "../i18n";

/**
 * Area selection (SPEC.md FR-30..FR-42).
 *
 * Two modes so far: draw a rectangle on the swisstopo basemap, or search a place and
 * take a radius around it. Administrative units and GPX corridors are specified and
 * still to come.
 */
export function AreaStep({
  t,
  deviceId,
  area,
  onArea,
  onNext,
  onBack,
}: {
  t: T;
  deviceId: string;
  area: AreaSelection | null;
  onArea: (a: AreaSelection) => void;
  onNext: () => void;
  onBack: () => void;
}) {
  const [box, setBox] = useState<DrawnBox | null>(null);
  const [info, setInfo] = useState<AreaInfo | null>(null);
  const [query, setQuery] = useState("");
  const [radiusKm, setRadiusKm] = useState(10);
  const [matches, setMatches] = useState<PlaceMatch[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Describe whatever is currently selected: area, projected bounds, size estimate.
  const describe = useCallback(
    async (a: AreaSelection) => {
      const b =
        a.kind === "bbox"
          ? { minE: a.minE, minN: a.minN, maxE: a.maxE, maxN: a.maxN }
          : {
              minE: a.easting - a.radiusKm * 1000,
              minN: a.northing - a.radiusKm * 1000,
              maxE: a.easting + a.radiusKm * 1000,
              maxN: a.northing + a.radiusKm * 1000,
            };
      try {
        const d = await api.describeArea(b.minE, b.minN, b.maxE, b.maxN, deviceId);
        setInfo(d);
        setBox({ west: d.wgs84[0]!, south: d.wgs84[1]!, east: d.wgs84[2]!, north: d.wgs84[3]! });
      } catch (e) {
        setError(String(e));
      }
    },
    [deviceId],
  );

  useEffect(() => {
    if (area) void describe(area);
  }, [area, describe]);

  const search = async () => {
    if (!query.trim()) return;
    setSearching(true);
    setError(null);
    setMatches(null);
    try {
      const found = await api.findPlaces(query.trim());
      setMatches(found);
      // Exactly one match can be taken directly; several must be shown, because
      // choosing the wrong one silently builds a map of the wrong valley.
      if (found.length === 1) choose(found[0]!);
    } catch (e) {
      setError(String(e));
    } finally {
      setSearching(false);
    }
  };

  const choose = (p: PlaceMatch) => {
    onArea({
      kind: "place",
      name: p.name,
      radiusKm,
      easting: p.easting,
      northing: p.northing,
    });
    setMatches(null);
  };

  return (
    <section className="screen wide">
      <h2>{t("step.area")}</h2>

      <div className="area-controls">
        <div className="field">
          <label htmlFor="place">{t("area.search")}</label>
          <div className="row tight">
            <input
              id="place"
              value={query}
              placeholder={t("area.searchPlaceholder")}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void search()}
            />
            <button type="button" onClick={() => void search()} disabled={searching}>
              {searching ? t("common.loading") : t("area.go")}
            </button>
          </div>
        </div>
        <div className="field">
          <label htmlFor="radius">{t("area.radius", { km: radiusKm })}</label>
          <input
            id="radius"
            type="range"
            min={2}
            max={60}
            step={1}
            value={radiusKm}
            onChange={(e) => {
              const km = Number(e.target.value);
              setRadiusKm(km);
              if (area?.kind === "place") onArea({ ...area, radiusKm: km });
            }}
          />
        </div>
      </div>

      {matches && matches.length > 1 && (
        <div className="notice">
          <strong>{t("area.ambiguous", { n: matches.length, name: query })}</strong>
          <ul className="matches">
            {matches.map((m, i) => (
              <li key={`${m.easting}-${i}`}>
                <button type="button" onClick={() => choose(m)}>
                  {m.name}
                  {m.populationCategory ? ` — ${m.populationCategory}` : ""}
                  <span className="mono muted small">
                    {" "}
                    {m.lat.toFixed(4)}N {m.lon.toFixed(4)}E
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {matches && matches.length === 0 && (
        <p className="muted">{t("area.noMatch", { name: query })}</p>
      )}

      <AreaMap
        t={t}
        box={box}
        onBox={(b) => {
          setBox(b);
          // MapLibre works in WGS84 and the recipe stores LV95, so the rectangle is
          // converted by the backend rather than reprojected here.
          void (async () => {
            try {
              const [minE, minN, maxE, maxN] = await api.wgs84BboxToLv95(
                b.west,
                b.south,
                b.east,
                b.north,
              );
              onArea({ kind: "bbox", minE, minN, maxE, maxN });
            } catch (e) {
              setError(String(e));
            }
          })();
        }}
      />

      {info && (
        <dl className="facts">
          <div>
            <dt>{t("area.size")}</dt>
            <dd>{info.areaKm2.toFixed(0)} km²</dd>
          </div>
          <div>
            <dt>{t("area.estimate")}</dt>
            <dd className={info.overBudget ? "error" : ""}>{formatBytes(info.estimatedBytes)}</dd>
          </div>
          <div>
            <dt>{t("area.coverage")}</dt>
            <dd>{info.withinSwitzerland ? t("common.yes") : t("area.outside")}</dd>
          </div>
        </dl>
      )}
      {info?.overBudget && <p className="error">{t("area.overBudget")}</p>}

      {error && <p className="error">{t("data.error", { message: error })}</p>}

      <div className="row">
        <button type="button" onClick={onBack}>{t("common.back")}</button>
        <button
          type="button"
          className="primary"
          disabled={!area || !info?.withinSwitzerland}
          onClick={onNext}
        >
          {t("common.next")}
        </button>
      </div>
    </section>
  );
}

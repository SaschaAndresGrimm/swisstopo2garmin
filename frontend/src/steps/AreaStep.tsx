import { useEffect, useState } from "react";
import { api } from "../state/api";
import type { AreaSelection, PlaceMatch } from "../state/api";
import { useAreaInfo } from "../state/useAreaInfo";
import { AreaMap, type DrawnBox } from "../map/AreaMap";
import { SizeEstimate } from "../components/SizeEstimate";
import { OverBudget } from "../components/OverBudget";
import { TrackImport } from "../components/TrackImport";
import { AdminUnitPicker } from "../components/AdminUnitPicker";
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
  preset,
  contourM,
  relief,
  area,
  onArea,
  onNext,
  onBack,
}: {
  t: T;
  deviceId: string;
  /** Content settings feed the estimate; they have wizard defaults before step 3. */
  preset: string;
  contourM: number;
  relief: string;
  area: AreaSelection | null;
  onArea: (a: AreaSelection) => void;
  onNext: () => void;
  onBack: () => void;
}) {
  const [box, setBox] = useState<DrawnBox | null>(null);
  const [query, setQuery] = useState("");
  const [radiusKm, setRadiusKm] = useState(10);
  const [matches, setMatches] = useState<PlaceMatch[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exported, setExported] = useState<string | null>(null);

  const { info, error: infoError } = useAreaInfo(area, deviceId, preset, contourM, relief);

  // The corridor centreline for the map, projected by the backend so the projection
  // has one implementation rather than two to keep in agreement.
  // Either the corridor centreline or the outlines of the chosen units, so the map
  // shows the actual shape rather than only its bounding box.
  const [trackLine, setTrackLine] = useState<[number, number][][] | null>(null);
  useEffect(() => {
    let live = true;
    if (area?.kind === "corridor") {
      api
        .lv95LineToWgs84(area.points as [number, number][])
        .then((line) => live && setTrackLine([line]))
        .catch(() => live && setTrackLine(null));
    } else if (area?.kind === "adminUnits") {
      api
        .adminOutline(area.level, area.numbers)
        .then((rings) => live && setTrackLine(rings))
        .catch(() => live && setTrackLine(null));
    } else {
      setTrackLine(null);
    }
    return () => {
      live = false;
    };
  }, [area]);

  // Frame whatever the estimate reports, so a place search moves the map too.
  useEffect(() => {
    if (info) {
      setBox({ west: info.wgs84[0]!, south: info.wgs84[1]!, east: info.wgs84[2]!, north: info.wgs84[3]! });
    }
  }, [info]);

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
        <AdminUnitPicker t={t} area={area} onArea={onArea} />

        <TrackImport t={t} area={area} onArea={onArea} />

        <div className="field">
          <label htmlFor="whole">{t("area.whole")}</label>
          <div className="row tight" id="whole">
            <button
              type="button"
              onClick={() => {
                // The extent comes from the backend so it cannot drift from the
                // coverage check that validates the selection.
                void api
                  .coverageBbox()
                  .then(([minE, minN, maxE, maxN]) =>
                    onArea({ kind: "bbox", minE, minN, maxE, maxN }),
                  )
                  .catch((e) => setError(String(e)));
              }}
            >
              {t("area.wholeAction")}
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
        track={trackLine}
        onPolygon={(points) => {
          void (async () => {
            try {
              // Projected by the backend, so the app has one projection.
              const lv95 = await Promise.all(
                points.map((p) => api.wgs84BboxToLv95(p[0], p[1], p[0], p[1])),
              );
              onArea({
                kind: "polygon",
                points: lv95.map(([e, n]) => [e, n] as [number, number]),
              });
            } catch (e) {
              setError(String(e));
            }
          })();
        }}
        onCircle={(centre, radiusM) => {
          void (async () => {
            try {
              const [e, n] = await api.wgs84BboxToLv95(centre[0], centre[1], centre[0], centre[1]);
              onArea({ kind: "circle", easting: e, northing: n, radiusKm: radiusM / 1000 });
            } catch (err) {
              setError(String(err));
            }
          })();
        }}
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

      {info && <SizeEstimate t={t} info={info} />}
      {info && !info.withinSwitzerland && <p className="error">{t("area.outside")}</p>}
      {info && <OverBudget t={t} info={info} />}

      {(error ?? infoError) && (
        <p className="error">{t("data.error", { message: error ?? infoError ?? "" })}</p>
      )}

      {/* Selections travel as GeoJSON, which QGIS and geojson.io both read (FR-42). */}
      <div className="row tight">
        <input
          type="file"
          accept=".geojson,.json"
          aria-label={t("area.import")}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (!f) return;
            void (async () => {
              try {
                onArea(await api.areaFromGeojson(await f.text()));
              } catch (err) {
                setError(String(err));
              }
            })();
            e.target.value = "";
          }}
        />
        <button
          type="button"
          disabled={!area}
          onClick={() => {
            if (!area) return;
            void (async () => {
              try {
                const { save } = await import("@tauri-apps/plugin-dialog");
                const path = await save({
                  title: t("area.exportTitle"),
                  defaultPath: "selection.geojson",
                  filters: [{ name: "GeoJSON", extensions: ["geojson"] }],
                });
                if (typeof path === "string") setExported(await api.exportArea(area, path));
              } catch (err) {
                setError(String(err));
              }
            })();
          }}
        >
          {t("area.export")}
        </button>
      </div>
      {exported && (
        <p className="muted small">
          {t("area.exported")} <span className="mono">{exported}</span>
        </p>
      )}

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

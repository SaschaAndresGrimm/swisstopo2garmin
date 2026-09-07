import { useEffect, useState } from "react";
import { api } from "../state/api";
import type { AreaEdit, AreaOutline, AreaSelection, PlaceMatch } from "../state/api";
import { useAreaInfo } from "../state/useAreaInfo";
import { AreaMap, type DrawnBox, type PendingEdit } from "../map/AreaMap";
import { SizeEstimate } from "../components/SizeEstimate";
import { OverBudget } from "../components/OverBudget";
import { TrackImport } from "../components/TrackImport";
import { AdminUnitPicker } from "../components/AdminUnitPicker";
import type { T } from "../i18n";

/** How the area is being chosen. Exactly one at a time. */
type Method = "draw" | "place" | "admin" | "route" | "whole";

const METHODS: Method[] = ["draw", "place", "admin", "route", "whole"];

/**
 * Which chooser produced this selection, so reopening the step -- or loading a saved
 * recipe -- shows the panel that made it rather than resetting to the map.
 */
function methodOf(area: AreaSelection | null): Method {
  switch (area?.kind) {
    case "place":
      return "place";
    case "adminUnits":
      return "admin";
    case "corridor":
      return "route";
    default:
      return "draw";
  }
}

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
  onClearArea,
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
  /** Discard the selection entirely, so the map can be cleared and started over. */
  onClearArea: () => void;
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
  const [outline, setOutline] = useState<AreaOutline | null>(null);
  // Which way of choosing an area is on show. One at a time, because all five at once
  // was 700 px of stacked controls that pushed the map -- the thing being chosen -- off
  // the bottom of the screen, and put the radius slider four columns away from the place
  // search it belongs to.
  const [method, setMethod] = useState<Method>(() => methodOf(area));
  // One step of undo. With editing, a single bad drag could lose a shape that took a
  // dozen clicks to draw -- and the previous selection is right here, so refusing to
  // keep it would be a choice.
  const [previous, setPrevious] = useState<AreaSelection | null>(null);

  const { info, error: infoError } = useAreaInfo(area, deviceId, preset, contourM, relief);

  // The selection's true shape and its drag handles (FR-31). Asked of the backend
  // rather than derived here: the geometry is LV95 and the map is WGS84, and only Rust
  // projects (FR-P1).
  useEffect(() => {
    let live = true;
    if (!area) {
      setOutline(null);
      return;
    }
    api
      .areaOutline(area)
      .then((o) => live && setOutline(o))
      .catch(() => live && setOutline(null));
    return () => {
      live = false;
    };
  }, [area]);

  /**
   * A handle was dropped: convert where it landed to LV95, then let Rust apply it.
   *
   * One conversion per drag, on release rather than on every mouse move. The map
   * previews the drag locally and this replaces the preview with the real answer, so a
   * refused edit -- a rectangle collapsed to a line, a radius dragged to nothing --
   * snaps back and says why instead of silently building nothing.
   */
  const applyEdit = async (pending: PendingEdit) => {
    if (!area) return;
    try {
      const [easting, northing] = await api.wgs84BboxToLv95(
        pending.lon,
        pending.lat,
        pending.lon,
        pending.lat,
      );
      let edit: AreaEdit;
      switch (pending.handle.role) {
        case "vertex":
          edit = { kind: "moveVertex", index: pending.handle.index, easting, northing };
          break;
        case "midpoint":
          edit = { kind: "insertVertex", after: pending.handle.index, easting, northing };
          break;
        case "radius":
          edit = { kind: "setRadiusKm", radiusKm: (pending.radiusM ?? 0) / 1000 };
          break;
        case "centre": {
          // The displacement, not the destination: converting both ends and
          // subtracting keeps the shape rigid, where converting a degree offset
          // directly would stretch it slightly with latitude.
          const [fromE, fromN] = await api.wgs84BboxToLv95(
            pending.lon - (pending.dLon ?? 0),
            pending.lat - (pending.dLat ?? 0),
            pending.lon - (pending.dLon ?? 0),
            pending.lat - (pending.dLat ?? 0),
          );
          edit = {
            kind: "translate",
            dEasting: easting - fromE,
            dNorthing: northing - fromN,
          };
          break;
        }
      }
      const next = await api.editArea(area, edit);
      setError(null);
      setPrevious(area);
      // `box` follows from the estimate's `wgs84` extent, so it does not need setting
      // here -- and the outline, not the box, is what the map draws.
      onArea(next);
    } catch (e) {
      // A refused edit is normal: the shape snaps back to what the backend still holds.
      setError(String(e));
      if (area) {
        api
          .areaOutline(area)
          .then(setOutline)
          .catch(() => setOutline(null));
      }
    }
  };

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

      {/* One row of methods, one panel. The map stays visible below whichever is
          chosen, because it is the thing being chosen. */}
      <div className="method-tabs" role="tablist" aria-label={t("area.methodLabel")}>
        {METHODS.map((x) => (
          <button
            key={x}
            type="button"
            role="tab"
            aria-selected={method === x}
            className={method === x ? "primary" : ""}
            onClick={() => setMethod(x)}
          >
            {t(`area.method.${x}`)}
          </button>
        ))}
      </div>

      {/* What is selected, wherever the chooser has been left. Pick a canton, switch to
          Draw, and the canton is still what will be built -- which the panel alone does
          not say. */}
      {outline && (
        <p className="selected-area">
          <span className="badge">{t("area.selectedLabel")}</span>{" "}
          {t(`area.selected.${outline.kind}`, { detail: outline.detail })}
          {previous && (
            <>
              {" "}
              <button
                type="button"
                className="link"
                onClick={() => {
                  onArea(previous);
                  setPrevious(null);
                  setError(null);
                }}
              >
                {t("area.undo")}
              </button>
            </>
          )}
        </p>
      )}

      <div className="method-panel">
        {method === "draw" && <p className="muted small">{t("area.method.drawHelp")}</p>}

        {method === "place" && (
          <>
            <div className="row wrap">
              <div className="field grow">
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
              {/* Beside the search, not four columns away: the radius only means
                  anything for a place. */}
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
            <p className="muted small">{t("area.searchHelp")}</p>

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
          </>
        )}

        {method === "admin" && <AdminUnitPicker t={t} area={area} onArea={onArea} />}

        {method === "route" && <TrackImport t={t} area={area} onArea={onArea} />}

        {method === "whole" && (
          <div className="row wrap">
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
            <p className="muted small">{t("area.wholeHelp")}</p>
          </div>
        )}
      </div>

      <AreaMap
        t={t}
        track={trackLine}
        outline={outline}
        onEdit={(e) => void applyEdit(e)}
        onClear={() => {
          setBox(null);
          setOutline(null);
          setError(null);
          // Deliberately discarding a selection is not the case undo is for.
          setPrevious(null);
          onClearArea();
        }}
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

      {/* Selections travel as GeoJSON, which QGIS and geojson.io both read (FR-42).
          Titled and grouped with Export, because this and the route importer put two
          unlabelled "Choose File" buttons on the same screen -- indistinguishable, and
          one of them silently replaces the selection. */}
      <details className="exchange">
        <summary>{t("area.exchange")}</summary>
        <p className="muted small">{t("area.exchangeHelp")}</p>
        <div className="row tight">
          <label className="field" htmlFor="geojson-in">
            <span>{t("area.import")}</span>
            <input
              id="geojson-in"
              type="file"
              accept=".geojson,.json"
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
          </label>
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
      </details>

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

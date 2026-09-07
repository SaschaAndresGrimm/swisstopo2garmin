import { useCallback, useEffect, useRef, useState } from "react";
import maplibregl, { type Map as MlMap } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { ATTRIBUTION, BASE_LAYERS, SWITZERLAND, tileUrl, type BaseLayerId } from "./wmts";
import type { AreaHandle, AreaOutline } from "../state/api";
import type { T } from "../i18n";

/**
 * A completed drag, in WGS84 and in terms of which handle moved.
 *
 * Deliberately not an `AreaEdit`: that one is in LV95, and the map has no business
 * projecting. The caller converts and applies it through Rust, where the geometry and
 * the projection both live.
 */
export interface PendingEdit {
  handle: AreaHandle;
  lon: number;
  lat: number;
  /** Metres from the shape's centre, for a radius drag. */
  radiusM?: number;
  /** Displacement in WGS84 degrees, for a whole-shape move. */
  dLon?: number;
  dLat?: number;
}

/** A rectangle in WGS84, as the map produces it. */
export interface DrawnBox {
  west: number;
  south: number;
  east: number;
  north: number;
}

const RECT_SOURCE = "s2g-rect";
const TRACK_SOURCE = "s2g-track";
const HANDLE_SOURCE = "s2g-handles";
const HANDLE_LAYER = "s2g-handle-points";

/** How close the pointer must be to grab a handle, in pixels. */
const GRAB_RADIUS = 10;

/**
 * swisstopo basemap with a drag-to-draw rectangle (FR-30, FR-31).
 *
 * Drawing is implemented directly rather than via a draw plugin: one rectangle is all
 * the area step needs, and a plugin would add a dependency and its own styling.
 */
/** Which drawing tool is armed. */
export type DrawTool = "rect" | "polygon" | "circle";

export function AreaMap({
  t,
  box,
  onBox,
  onPolygon,
  onCircle,
  track,
  outline,
  onEdit,
}: {
  t: T;
  box: DrawnBox | null;
  onBox: (b: DrawnBox) => void;
  /** Freehand polygon in WGS84 `[lon, lat]`, closed by the caller (FR-31). */
  onPolygon?: (points: [number, number][]) => void;
  /** Centre in WGS84 and a radius in metres (FR-31). */
  onCircle?: (centre: [number, number], radiusM: number) => void;
  /** Lines to draw over the rectangle in WGS84 `[lon, lat]`: an imported track's
   *  centreline, or the outlines of the chosen administrative units. */
  track?: [number, number][][] | null;
  /** The current selection's true shape and drag handles, from the backend (FR-31).
   *  Drawn instead of `box`, which is only a bounding rectangle -- and drawing that
   *  rectangle for a polygon or circle was the defect this replaces. */
  outline?: AreaOutline | null;
  /** A handle was dragged and released. */
  onEdit?: (edit: PendingEdit) => void;
}) {
  const container = useRef<HTMLDivElement | null>(null);
  const map = useRef<MlMap | null>(null);
  const [base, setBase] = useState<BaseLayerId>("pixelkarte");
  const [drawing, setDrawing] = useState(false);
  const [tool, setTool] = useState<DrawTool>("rect");
  const dragStart = useRef<maplibregl.LngLat | null>(null);
  // Vertices of a polygon in progress, kept in a ref so the map handlers see the
  // current value without being rebound on every click.
  const vertices = useRef<[number, number][]>([]);

  // Keep the latest callback without re-creating the map.
  const onBoxRef = useRef(onBox);
  onBoxRef.current = onBox;
  const onPolygonRef = useRef(onPolygon);
  onPolygonRef.current = onPolygon;
  const onCircleRef = useRef(onCircle);
  onCircleRef.current = onCircle;
  const onEditRef = useRef(onEdit);
  onEditRef.current = onEdit;
  // The handlers below are bound once; they read the current outline through a ref so a
  // re-render does not have to rebind them mid-drag.
  const outlineRef = useRef(outline);
  outlineRef.current = outline;
  /** The handle being dragged, and where the drag started. */
  const grabbed = useRef<{ handle: AreaHandle; from: maplibregl.LngLat } | null>(null);
  const [hovering, setHovering] = useState(false);

  const rectGeoJson = useCallback((b: DrawnBox | null) => {
    if (!b) return { type: "FeatureCollection" as const, features: [] };
    return {
      type: "FeatureCollection" as const,
      features: [
        {
          type: "Feature" as const,
          properties: {},
          geometry: {
            type: "Polygon" as const,
            coordinates: [
              [
                [b.west, b.south],
                [b.east, b.south],
                [b.east, b.north],
                [b.west, b.north],
                [b.west, b.south],
              ],
            ],
          },
        },
      ],
    };
  }, []);

  useEffect(() => {
    if (!container.current || map.current) return;
    const m = new maplibregl.Map({
      container: container.current,
      style: {
        version: 8,
        sources: {
          swisstopo: {
            type: "raster",
            tiles: [tileUrl("pixelkarte")],
            tileSize: 256,
            maxzoom: 18,
            attribution: ATTRIBUTION,
          },
        },
        layers: [{ id: "swisstopo", type: "raster", source: "swisstopo" }],
      },
      bounds: SWITZERLAND,
      fitBoundsOptions: { padding: 20 },
      attributionControl: false,
    });
    m.addControl(new maplibregl.NavigationControl({ showCompass: false }), "top-right");
    m.addControl(new maplibregl.ScaleControl({ unit: "metric" }), "bottom-left");
    m.addControl(new maplibregl.AttributionControl({ compact: false }), "bottom-right");

    m.on("load", () => {
      m.addSource(RECT_SOURCE, { type: "geojson", data: rectGeoJson(null) });
      m.addLayer({
        id: "s2g-rect-fill",
        type: "fill",
        source: RECT_SOURCE,
        paint: { "fill-color": "#da291c", "fill-opacity": 0.12 },
      });
      m.addLayer({
        id: "s2g-rect-line",
        type: "line",
        source: RECT_SOURCE,
        paint: { "line-color": "#da291c", "line-width": 2 },
      });
      m.addSource(TRACK_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      m.addLayer({
        id: "s2g-track-line",
        type: "line",
        source: TRACK_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: { "line-color": "#1b3fa0", "line-width": 3 },
      });

      // Drag handles, on top of everything. Midpoints are drawn smaller and hollow so
      // they read as "this is not a corner yet" rather than competing with the real
      // vertices for the eye.
      m.addSource(HANDLE_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      m.addLayer({
        id: HANDLE_LAYER,
        type: "circle",
        source: HANDLE_SOURCE,
        paint: {
          "circle-radius": ["case", ["==", ["get", "role"], "midpoint"], 4, 6],
          "circle-color": [
            "match",
            ["get", "role"],
            "midpoint",
            "#ffffff",
            "centre",
            "#1b3fa0",
            "radius",
            "#1b3fa0",
            "#da291c",
          ],
          "circle-stroke-width": 2,
          "circle-stroke-color": "#ffffff",
          "circle-opacity": ["case", ["==", ["get", "role"], "midpoint"], 0.9, 1],
        },
      });
    });

    map.current = m;
    return () => {
      m.remove();
      map.current = null;
    };
  }, [rectGeoJson]);

  // Swap the basemap without rebuilding the map or losing the drawn rectangle.
  useEffect(() => {
    const m = map.current;
    if (!m || !m.isStyleLoaded()) return;
    const src = m.getSource("swisstopo") as maplibregl.RasterTileSource | undefined;
    src?.setTiles?.([tileUrl(base)]);
  }, [base]);

  // The selection's true shape, and its handles.
  //
  // `outline` wins over `box` whenever it exists: `box` is only a bounding rectangle,
  // and drawing that for a polygon or a circle replaced the shape the user had just
  // drawn with one they had not.
  useEffect(() => {
    const m = map.current;
    if (!m) return;
    const apply = () => {
      const shape = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
      if (outline && outline.rings.length > 0) {
        shape?.setData({
          type: "FeatureCollection",
          features: outline.rings.map((ring) => ({
            type: "Feature",
            properties: {},
            geometry: { type: "Polygon", coordinates: [ring] },
          })),
        } as never);
      } else {
        shape?.setData(rectGeoJson(box) as never);
      }

      const handles = m.getSource(HANDLE_SOURCE) as maplibregl.GeoJSONSource | undefined;
      handles?.setData({
        type: "FeatureCollection",
        features: (outline?.handles ?? []).map((h, i) => ({
          type: "Feature",
          // The index into the handle array, so a hit test can find the handle itself
          // rather than reconstructing which one it was from coordinates.
          id: i,
          properties: { role: h.role, index: h.index, at: i },
          geometry: { type: "Point", coordinates: [h.lon, h.lat] },
        })),
      } as never);
    };
    if (m.isStyleLoaded()) apply();
    else m.once("load", apply);
  }, [box, outline, rectGeoJson]);

  useEffect(() => {
    const m = map.current;
    if (!m) return;
    const apply = () => {
      const src = m.getSource(TRACK_SOURCE) as maplibregl.GeoJSONSource | undefined;
      src?.setData({
        type: "FeatureCollection",
        features: (track ?? [])
          .filter((line) => line.length > 1)
          .map((line) => ({
            type: "Feature",
            properties: {},
            geometry: { type: "LineString", coordinates: line },
          })),
      } as never);
    };
    if (m.isStyleLoaded()) apply();
    else m.once("load", apply);
  }, [track]);

  // Drag to draw. Panning is disabled only while the draw tool is armed, so the map
  // stays normally navigable.
  useEffect(() => {
    const m = map.current;
    if (!m) return;
    if (!drawing) {
      m.dragPan.enable();
      return;
    }
    m.dragPan.disable();

    // Metres between two positions, for the circle radius. Small distances at Swiss
    // latitudes, so the spherical approximation is well within a pixel.
    const metres = (a: maplibregl.LngLat, b: maplibregl.LngLat) => {
      const R = 6_371_000;
      const dLat = ((b.lat - a.lat) * Math.PI) / 180;
      const dLon = ((b.lng - a.lng) * Math.PI) / 180;
      const lat = ((a.lat + b.lat) / 2) * (Math.PI / 180);
      const x = dLon * Math.cos(lat);
      return Math.sqrt(x * x + dLat * dLat) * R;
    };

    const ringOf = (points: [number, number][]) => ({
      type: "FeatureCollection" as const,
      features: [
        {
          type: "Feature" as const,
          properties: {},
          geometry: { type: "Polygon" as const, coordinates: [[...points, points[0]]] },
        },
      ],
    });

    const click = (e: maplibregl.MapMouseEvent) => {
      if (tool !== "polygon") return;
      vertices.current = [...vertices.current, [e.lngLat.lng, e.lngLat.lat]];
      const src = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
      if (vertices.current.length >= 3) src?.setData(ringOf(vertices.current) as never);
    };

    // Double-click finishes a polygon; the map's own zoom is suppressed while drawing.
    const finish = (e: maplibregl.MapMouseEvent) => {
      if (tool !== "polygon") return;
      e.preventDefault();
      if (vertices.current.length >= 3) onPolygonRef.current?.(vertices.current);
      vertices.current = [];
      setDrawing(false);
    };

    const down = (e: maplibregl.MapMouseEvent) => {
      if (tool === "polygon") return;
      dragStart.current = e.lngLat;
    };
    const circleRing = (centre: maplibregl.LngLat, radiusM: number) => {
      const points: [number, number][] = [];
      for (let i = 0; i < 64; i++) {
        const a = (i / 64) * Math.PI * 2;
        const dLat = ((radiusM * Math.sin(a)) / 6_371_000) * (180 / Math.PI);
        const dLon =
          ((radiusM * Math.cos(a)) / (6_371_000 * Math.cos((centre.lat * Math.PI) / 180))) *
          (180 / Math.PI);
        points.push([centre.lng + dLon, centre.lat + dLat]);
      }
      return points;
    };

    const move = (e: maplibregl.MapMouseEvent) => {
      const s = dragStart.current;
      if (!s) return;
      const src = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
      if (tool === "circle") {
        src?.setData(ringOf(circleRing(s, metres(s, e.lngLat))) as never);
        return;
      }
      src?.setData(
        rectGeoJson({
          west: Math.min(s.lng, e.lngLat.lng),
          south: Math.min(s.lat, e.lngLat.lat),
          east: Math.max(s.lng, e.lngLat.lng),
          north: Math.max(s.lat, e.lngLat.lat),
        }) as never,
      );
    };
    const up = (e: maplibregl.MapMouseEvent) => {
      const s = dragStart.current;
      dragStart.current = null;
      if (!s) return;
      if (tool === "circle") {
        const r = metres(s, e.lngLat);
        // An accidental click is not a circle.
        if (r > 200) {
          onCircleRef.current?.([s.lng, s.lat], r);
          setDrawing(false);
        }
        return;
      }
      const b = {
        west: Math.min(s.lng, e.lngLat.lng),
        south: Math.min(s.lat, e.lngLat.lat),
        east: Math.max(s.lng, e.lngLat.lng),
        north: Math.max(s.lat, e.lngLat.lat),
      };
      // Ignore an accidental click; a degenerate box would build nothing.
      if (b.east - b.west < 0.002 || b.north - b.south < 0.002) return;
      onBoxRef.current(b);
      setDrawing(false);
    };

    m.on("mousedown", down);
    m.on("mousemove", move);
    m.on("mouseup", up);
    m.on("click", click);
    m.on("dblclick", finish);
    m.doubleClickZoom.disable();
    return () => {
      m.off("mousedown", down);
      m.off("mousemove", move);
      m.off("mouseup", up);
      m.off("click", click);
      m.off("dblclick", finish);
      m.doubleClickZoom.enable();
      vertices.current = [];
    };
  }, [drawing, tool, rectGeoJson]);

  // Editing: grab a handle, drag it, drop it. Bound once and only active when the
  // draw tool is *not* armed, so drawing a new shape and editing the current one cannot
  // both claim the pointer.
  useEffect(() => {
    const m = map.current;
    if (!m || drawing) return;

    /** Metres between two positions. Only used for a circle's radius, over a few
     *  kilometres at Swiss latitudes, where the spherical approximation is well inside
     *  a pixel. The authoritative value is recomputed in LV95 by the backend. */
    const metres = (a1: maplibregl.LngLat, b: maplibregl.LngLat) => {
      const R = 6_371_000;
      const dLat = ((b.lat - a1.lat) * Math.PI) / 180;
      const dLon = ((b.lng - a1.lng) * Math.PI) / 180;
      const lat = ((a1.lat + b.lat) / 2) * (Math.PI / 180);
      const x = dLon * Math.cos(lat);
      return Math.sqrt(x * x + dLat * dLat) * R;
    };

    /** The handle under the pointer, if any. */
    const handleAt = (e: maplibregl.MapMouseEvent): AreaHandle | null => {
      const current = outlineRef.current;
      if (!current?.editable || !m.getLayer(HANDLE_LAYER)) return null;
      const box: [maplibregl.PointLike, maplibregl.PointLike] = [
        [e.point.x - GRAB_RADIUS, e.point.y - GRAB_RADIUS],
        [e.point.x + GRAB_RADIUS, e.point.y + GRAB_RADIUS],
      ];
      const hit = m.queryRenderedFeatures(box, { layers: [HANDLE_LAYER] })[0];
      const at = hit?.properties?.at;
      return typeof at === "number" ? (current.handles[at] ?? null) : null;
    };

    /** Whether the pointer is inside the shape, which is what makes it draggable. */
    const insideShape = (e: maplibregl.MapMouseEvent) =>
      !!outlineRef.current?.editable &&
      !!m.getLayer("s2g-rect-fill") &&
      m.queryRenderedFeatures(e.point, { layers: ["s2g-rect-fill"] }).length > 0;

    const hover = (e: maplibregl.MapMouseEvent) => {
      if (grabbed.current) return;
      const over = !!handleAt(e) || insideShape(e);
      setHovering(over);
      m.getCanvas().style.cursor = over ? "move" : "";
    };

    const down = (e: maplibregl.MapMouseEvent) => {
      const handle = handleAt(e);
      // Clicking the body of the shape moves the whole thing: the "area pannable" half
      // of being able to edit a selection. A synthetic centre handle carries it, so the
      // drop path below has one shape to deal with.
      const grabbedHandle =
        handle ??
        (insideShape(e)
          ? { lon: e.lngLat.lng, lat: e.lngLat.lat, role: "centre" as const, index: 0 }
          : null);
      if (!grabbedHandle) return;
      grabbed.current = { handle: grabbedHandle, from: e.lngLat };
      // Only now: otherwise the map pans out from under every handle drag.
      m.dragPan.disable();
      e.preventDefault();
    };

    const move = (e: maplibregl.MapMouseEvent) => {
      const g = grabbed.current;
      if (!g) {
        hover(e);
        return;
      }
      // A local preview while dragging. Approximate on purpose -- it is redrawn from
      // the backend's answer on release, which is the authoritative one.
      const current = outlineRef.current;
      if (!current) return;
      const src = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
      const dLon = e.lngLat.lng - g.from.lng;
      const dLat = e.lngLat.lat - g.from.lat;
      if (g.handle.role === "centre") {
        // Moving the whole shape previews exactly: every point shifts by the same
        // amount, so the local approximation and the backend's answer agree.
        src?.setData({
          type: "FeatureCollection",
          features: current.rings.map((ring) => ({
            type: "Feature",
            properties: {},
            geometry: {
              type: "Polygon",
              coordinates: [ring.map(([lon, lat]) => [lon + dLon, lat + dLat])],
            },
          })),
        } as never);
      }

      // Every other drag previews by moving the *handle*, not the outline. Reshaping the
      // outline locally would mean reimplementing which corner anchors which -- the one
      // thing deliberately kept in Rust and tested there -- and getting it subtly
      // different here would show as the shape jumping on release. The dot following the
      // cursor is enough to make the drag feel connected; the shape snaps to the
      // authoritative result when the button comes up.
      const handles = m.getSource(HANDLE_SOURCE) as maplibregl.GeoJSONSource | undefined;
      handles?.setData({
        type: "FeatureCollection",
        features: current.handles.map((h, i) => {
          const dragged = h.role === g.handle.role && h.index === g.handle.index;
          return {
            type: "Feature",
            id: i,
            properties: { role: h.role, index: h.index, at: i },
            geometry: {
              type: "Point",
              coordinates: dragged ? [e.lngLat.lng, e.lngLat.lat] : [h.lon, h.lat],
            },
          };
        }),
      } as never);
    };

    const up = (e: maplibregl.MapMouseEvent) => {
      const g = grabbed.current;
      grabbed.current = null;
      m.dragPan.enable();
      if (!g) return;

      const moved =
        Math.abs(e.point.x - m.project(g.from).x) + Math.abs(e.point.y - m.project(g.from).y);
      // A click is not a drag. Without this, selecting the shape would nudge it.
      if (moved < 3) return;

      const centre = outlineRef.current?.handles.find((h) => h.role === "centre");
      onEditRef.current?.({
        handle: g.handle,
        lon: e.lngLat.lng,
        lat: e.lngLat.lat,
        radiusM:
          g.handle.role === "radius" && centre
            ? metres(new maplibregl.LngLat(centre.lon, centre.lat), e.lngLat)
            : undefined,
        dLon: e.lngLat.lng - g.from.lng,
        dLat: e.lngLat.lat - g.from.lat,
      });
    };

    m.on("mousedown", down);
    m.on("mousemove", move);
    m.on("mouseup", up);
    return () => {
      m.off("mousedown", down);
      m.off("mousemove", move);
      m.off("mouseup", up);
      m.getCanvas().style.cursor = "";
      grabbed.current = null;
      m.dragPan.enable();
    };
  }, [drawing]);

  /** Centre and frame a box the user chose elsewhere (a place search result).
   *
   * Only when it is not already on screen. Refitting on every change would yank the
   * view away mid-edit: dragging a corner changes the selection, which changes the box,
   * which would re-frame the map under the cursor on every single drag.
   */
  useEffect(() => {
    const m = map.current;
    if (!m || !box) return;
    const view = m.getBounds();
    const onScreen =
      box.west >= view.getWest() &&
      box.east <= view.getEast() &&
      box.south >= view.getSouth() &&
      box.north <= view.getNorth();
    if (onScreen) return;
    m.fitBounds(
      [
        [box.west, box.south],
        [box.east, box.north],
      ],
      { padding: 40, duration: 400 },
    );
  }, [box]);

  return (
    <div className="areamap">
      <div className="areamap-toolbar">
        {(["rect", "polygon", "circle"] as DrawTool[]).map((x) => (
          <button
            key={x}
            type="button"
            className={drawing && tool === x ? "primary" : ""}
            aria-pressed={drawing && tool === x}
            onClick={() => {
              vertices.current = [];
              if (drawing && tool === x) {
                setDrawing(false);
              } else {
                setTool(x);
                setDrawing(true);
              }
            }}
          >
            {t(`map.tool.${x}`)}
          </button>
        ))}
        {drawing && (
          <span className="muted small">
            {tool === "polygon" ? t("map.polygonHint") : t("map.drawing")}
          </span>
        )}
        {/* Draggable handles are invisible as an affordance until the pointer is over
            one, so say what they do the moment the pointer finds them -- and say why
            not, for the selections whose shape is derived from data. */}
        {!drawing && outline?.editable && (
          <span className="muted small">
            {hovering ? t("map.editDragging") : t("map.editHint")}
          </span>
        )}
        {!drawing && outline && !outline.editable && outline.notEditableBecause && (
          <span className="muted small">{outline.notEditableBecause}</span>
        )}
        <div className="spacer" />
        <select
          aria-label={t("map.base")}
          value={base}
          onChange={(e) => setBase(e.target.value as BaseLayerId)}
        >
          {BASE_LAYERS.map((l) => (
            <option key={l.id} value={l.id}>
              {t(l.labelKey)}
            </option>
          ))}
        </select>
      </div>
      <div ref={container} className="areamap-canvas" />
    </div>
  );
}

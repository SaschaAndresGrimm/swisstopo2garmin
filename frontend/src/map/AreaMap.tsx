import { useCallback, useEffect, useRef, useState } from "react";
import maplibregl, { type Map as MlMap } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { ATTRIBUTION, BASE_LAYERS, SWITZERLAND, tileUrl, type BaseLayerId } from "./wmts";
import type { T } from "../i18n";

/** A rectangle in WGS84, as the map produces it. */
export interface DrawnBox {
  west: number;
  south: number;
  east: number;
  north: number;
}

const RECT_SOURCE = "s2g-rect";
const TRACK_SOURCE = "s2g-track";

/**
 * swisstopo basemap with a drag-to-draw rectangle (FR-30, FR-31).
 *
 * Drawing is implemented directly rather than via a draw plugin: one rectangle is all
 * the area step needs, and a plugin would add a dependency and its own styling.
 */
export function AreaMap({
  t,
  box,
  onBox,
  track,
}: {
  t: T;
  box: DrawnBox | null;
  onBox: (b: DrawnBox) => void;
  /** Imported track centreline in WGS84 `[lon, lat]`, drawn over the rectangle. */
  track?: [number, number][] | null;
}) {
  const container = useRef<HTMLDivElement | null>(null);
  const map = useRef<MlMap | null>(null);
  const [base, setBase] = useState<BaseLayerId>("pixelkarte");
  const [drawing, setDrawing] = useState(false);
  const dragStart = useRef<maplibregl.LngLat | null>(null);

  // Keep the latest callback without re-creating the map.
  const onBoxRef = useRef(onBox);
  onBoxRef.current = onBox;

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

  useEffect(() => {
    const m = map.current;
    if (!m) return;
    const apply = () => {
      const src = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
      src?.setData(rectGeoJson(box) as never);
    };
    if (m.isStyleLoaded()) apply();
    else m.once("load", apply);
  }, [box, rectGeoJson]);

  useEffect(() => {
    const m = map.current;
    if (!m) return;
    const apply = () => {
      const src = m.getSource(TRACK_SOURCE) as maplibregl.GeoJSONSource | undefined;
      src?.setData({
        type: "FeatureCollection",
        features:
          track && track.length > 1
            ? [
                {
                  type: "Feature",
                  properties: {},
                  geometry: { type: "LineString", coordinates: track },
                },
              ]
            : [],
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

    const down = (e: maplibregl.MapMouseEvent) => {
      dragStart.current = e.lngLat;
    };
    const move = (e: maplibregl.MapMouseEvent) => {
      const s = dragStart.current;
      if (!s) return;
      const src = m.getSource(RECT_SOURCE) as maplibregl.GeoJSONSource | undefined;
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
    return () => {
      m.off("mousedown", down);
      m.off("mousemove", move);
      m.off("mouseup", up);
    };
  }, [drawing, rectGeoJson]);

  /** Centre and frame a box the user chose elsewhere (a place search result). */
  const frame = useCallback((b: DrawnBox) => {
    map.current?.fitBounds(
      [
        [b.west, b.south],
        [b.east, b.north],
      ],
      { padding: 40, duration: 400 },
    );
  }, []);
  useEffect(() => {
    if (box) frame(box);
  }, [box, frame]);

  return (
    <div className="areamap">
      <div className="areamap-toolbar">
        <button
          type="button"
          className={drawing ? "primary" : ""}
          aria-pressed={drawing}
          onClick={() => setDrawing((d) => !d)}
        >
          {drawing ? t("map.drawing") : t("map.draw")}
        </button>
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

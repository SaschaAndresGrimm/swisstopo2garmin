// swisstopo WMTS basemaps for the area picker (SPEC.md §3.4, FR-30).
//
// Display only, served in Web Mercator so they drop straight into MapLibre. The CSP in
// tauri.conf.json must allow img-src from wmts.geo.admin.ch.

export type BaseLayerId = "pixelkarte" | "tlm" | "imagery";

interface BaseLayer {
  id: BaseLayerId;
  labelKey: string;
  layer: string;
  format: "jpeg" | "png";
}

export const BASE_LAYERS: BaseLayer[] = [
  { id: "pixelkarte", labelKey: "map.base.pixelkarte", layer: "ch.swisstopo.pixelkarte-farbe", format: "jpeg" },
  { id: "tlm", labelKey: "map.base.tlm", layer: "ch.swisstopo.swisstlm3d-karte-farbe", format: "png" },
  { id: "imagery", labelKey: "map.base.imagery", layer: "ch.swisstopo.swissimage", format: "jpeg" },
];

export function tileUrl(id: BaseLayerId): string {
  const l = BASE_LAYERS.find((b) => b.id === id) ?? BASE_LAYERS[0]!;
  return (
    `https://wmts.geo.admin.ch/1.0.0/${l.layer}/default/current/3857/` +
    `{z}/{x}/{y}.${l.format}`
  );
}

/** swisstopo requires the source to be indicated wherever the data is shown (FR-L1). */
export const ATTRIBUTION = "© swisstopo";

/** Roughly the swissTLM3D coverage area, as [west, south, east, north]. */
export const SWITZERLAND: [number, number, number, number] = [5.8, 45.7, 10.6, 47.9];

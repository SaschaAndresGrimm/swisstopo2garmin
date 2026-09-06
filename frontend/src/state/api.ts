// Thin wrapper over the Tauri IPC surface. Types come from bindings.ts, which is
// generated from the Rust definitions (see src-tauri/src/ipc.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CacheStatus, ReleaseInfo, TaskDone, TaskError, TaskProgress } from "./bindings";

export type {
  AdminUnitInfo,
  AreaInfo,
  AreaQuery,
  BuildFinished,
  BuildFailure,
  BuildProgress,
  CacheStatus,
  ConnectedDevice,
  DataLocation,
  DeviceOverrideInfo,
  DatasetEntry,
  DeviceSummary,
  InstallInstructions,
  InstallPlan,
  LayerInfo,
  PlaceMatch,
  PresetInfo,
  ReleaseInfo,
  SavedRecipeInfo,
  TrackImport,
  UsbDeviceInfo,
  TaskProgress,
} from "./bindings";

/** Mirrors s2g_core::recipe::Recipe. Hand-written because ts-rs cannot export the
 *  tagged area union in a shape TypeScript narrows well. */
export type AreaSelection =
  | { kind: "bbox"; minE: number; minN: number; maxE: number; maxN: number }
  | { kind: "place"; name: string; radiusKm: number; easting: number; northing: number }
  | { kind: "corridor"; name: string; bufferKm: number; points: [number, number][] }
  | { kind: "polygon"; points: [number, number][] }
  | { kind: "circle"; easting: number; northing: number; radiusKm: number }
  | { kind: "composite"; parts: AreaSelection[] }
  | {
      kind: "adminUnits";
      level: AdminLevel;
      numbers: number[];
      names: string[];
      bufferKm: number;
      minE: number;
      minN: number;
      maxE: number;
      maxN: number;
    };

export type PresetId = "hiking" | "cycling" | "skimo" | "full";
export type ReliefDetail = "off" | "gentle" | "detailed";
export type Palette = "summer" | "winter";
export type AdminLevel = "canton" | "district" | "commune";

export interface Recipe {
  schemaVersion: number;
  name: string;
  deviceId: string;
  area: AreaSelection;
  preset: PresetId;
  contours: { intervalM: number; indexM: number; simplifyM: number };
  relief: ReliefDetail;
  palette: Palette;
  slopeClasses: boolean;
  excludedLayers: string[];
}

export const TLM3D = "ch.swisstopo.swisstlm3d";
export const WANDERWEGE = "ch.swisstopo.swisstlm3d-wanderwege";

/**
 * Sources the Data screen manages, in the order they are shown.
 *
 * `group` decides which presets a source unlocks, which is what the content step's
 * "needs winter route data" message points at.
 */
export const SOURCES = [
  { id: TLM3D, key: "source.tlm3d", group: "base" },
  { id: WANDERWEGE, key: "source.wanderwege", group: "base" },
  { id: "ch.swisstopo-karto.skitouren", key: "source.skitouren", group: "winter" },
  { id: "ch.astra.schneeschuhwanderwege", key: "source.schneeschuh", group: "winter" },
  { id: "ch.astra.winterwanderwege", key: "source.winterwandern", group: "winter" },
  { id: "ch.astra.veloland", key: "source.veloland", group: "cycling" },
  { id: "ch.astra.mountainbikeland", key: "source.mountainbikeland", group: "cycling" },
  { id: "ch.astra.wanderland", key: "source.wanderland", group: "cycling" },
] as const;

export const api = {
  listDevices: () => invoke<import("./bindings").DeviceSummary[]>("list_devices"),
  deviceOverride: (deviceId: string) =>
    invoke<import("./bindings").DeviceOverrideInfo>("device_override", { deviceId }),
  /** `null` clears the override and restores the shipped limits. */
  setDeviceOverride: (deviceId: string, value: import("./bindings").DeviceOverrideInfo | null) =>
    invoke<void>("set_device_override", { deviceId, value }),
  detectDevices: () => invoke<import("./bindings").ConnectedDevice[]>("detect_devices"),
  /** Garmin devices on the USB bus, including ones not mounted as a filesystem. */
  usbDevices: () => invoke<import("./bindings").UsbDeviceInfo[]>("usb_devices"),
  installInstructions: (deviceId: string, mapName: string) =>
    invoke<import("./bindings").InstallInstructions>("install_instructions", {
      deviceId,
      mapName,
    }),
  /** Copy the built map into a folder the user picked (FR-83). */
  exportMap: (gmapsupp: string, dir: string, deviceId: string, mapName: string) =>
    invoke<string>("export_map", { gmapsupp, dir, deviceId, mapName }),
  listPresets: () => invoke<import("./bindings").PresetInfo[]>("list_presets"),
  listLayers: () => invoke<import("./bindings").LayerInfo[]>("list_layers"),
  listRecipes: () => invoke<import("./bindings").SavedRecipeInfo[]>("list_recipes"),
  saveRecipe: (recipe: Recipe) => invoke<string>("save_recipe", { recipe }),
  loadRecipe: (id: string) => invoke<Recipe>("load_recipe", { id }),
  deleteRecipe: (id: string) => invoke<void>("delete_recipe", { id }),
  findPlaces: (name: string) => invoke<import("./bindings").PlaceMatch[]>("find_places", { name }),
  /** LV95 [minE, minN, maxE, maxN] for a WGS84 rectangle. The projection lives only
   *  in Rust so there is one implementation, not two to keep in agreement. */
  /** Full swissTLM3D coverage in LV95, for the whole-Switzerland action (FR-35). */
  dataLocation: () => invoke<import("./bindings").DataLocation>("data_location"),
  inspectDataLocation: (path: string) =>
    invoke<import("./bindings").DataLocation>("inspect_data_location", { path }),
  /** `null` restores the platform default. */
  setDataLocation: (path: string | null) =>
    invoke<import("./bindings").DataLocation>("set_data_location", { path }),
  clearElevationCache: () =>
    invoke<import("./bindings").DataLocation>("clear_elevation_cache"),
  clearBuildFiles: () => invoke<import("./bindings").DataLocation>("clear_build_files"),
  coverageBbox: () => invoke<[number, number, number, number]>("coverage_bbox"),
  /** Every unit at one level; the picker filters locally as the user types. */
  listAdminUnits: (level: AdminLevel) =>
    invoke<import("./bindings").AdminUnitInfo[]>("list_admin_units", { level }),
  /** Unit outlines in WGS84, simplified for display. */
  adminOutline: (level: AdminLevel, numbers: number[]) =>
    invoke<[number, number][][]>("admin_outline", { level, numbers }),
  adminExtent: (level: AdminLevel, numbers: number[], bufferKm: number) =>
    invoke<[number, number, number, number]>("admin_extent", { level, numbers, bufferKm }),
  /** Selections travel as GeoJSON in WGS84, which QGIS and geojson.io both read. */
  areaToGeojson: (area: AreaSelection) => invoke<string>("area_to_geojson", { area }),
  areaFromGeojson: (text: string) => invoke<AreaSelection>("area_from_geojson", { text }),
  exportArea: (area: AreaSelection, path: string) =>
    invoke<string>("export_area", { area, path }),

  /** Project a polyline for display. Kept in Rust so there is one projection. */
  lv95LineToWgs84: (points: [number, number][]) =>
    invoke<[number, number][]>("lv95_line_to_wgs84", { points }),
  /** Parse a GPX or FIT file the frontend has already read (FR-38). */
  importTrack: (name: string, bytes: Uint8Array) =>
    invoke<import("./bindings").TrackImport>("import_track", {
      name,
      bytes: Array.from(bytes),
    }),
  wgs84BboxToLv95: (west: number, south: number, east: number, north: number) =>
    invoke<[number, number, number, number]>("wgs84_bbox_to_lv95", { west, south, east, north }),
  describeArea: (query: import("./bindings").AreaQuery) =>
    invoke<import("./bindings").AreaInfo>("describe_area", { query }),
  startBuild: (recipe: Recipe) => invoke<string>("start_build", { recipe }),
  planInstall: (gmapsupp: string, mount: string, deviceId: string, mapName: string) =>
    invoke<import("./bindings").InstallPlan>("plan_install", { gmapsupp, mount, deviceId, mapName }),
  installMap: (plan: import("./bindings").InstallPlan, backup: boolean) =>
    invoke<string>("install_map", { plan, backup }),
  cacheStatus: () => invoke<CacheStatus>("cache_status"),
  latestRelease: (collection: string) => invoke<ReleaseInfo>("latest_release", { collection }),
  acquire: (collection: string) => invoke<string>("acquire_dataset", { collection }),
  cancel: (taskId: string) => invoke<void>("cancel_task", { taskId }),
  remove: (collection: string, item: string) => invoke<void>("remove_dataset", { collection, item }),
};

/**
 * Subscribe to task events.
 *
 * `onFailure` matters: Tauri 2 denies `listen()` unless the window has an event
 * capability, and the rejection is otherwise silent — the UI simply never updates
 * while `invoke` keeps working, which looks like a frontend bug. Surface it.
 */
/** Build events. Separate from dataset task events so a build cannot be confused
 *  with a download. */
export function onBuildEvents(handlers: {
  progress?: (p: import("./bindings").BuildProgress) => void;
  done?: (d: import("./bindings").BuildFinished) => void;
  error?: (e: TaskError) => void;
  /** The interpreted failure (FR-73); arrives alongside `error`. */
  failed?: (f: import("./bindings").BuildFailure) => void;
  onFailure?: (reason: string) => void;
}): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<import("./bindings").BuildProgress>("build:progress", (e) => handlers.progress?.(e.payload)),
    listen<import("./bindings").BuildFinished>("build:done", (e) => handlers.done?.(e.payload)),
    listen<TaskError>("build:error", (e) => handlers.error?.(e.payload)),
    listen<import("./bindings").BuildFailure>("build:failed", (e) => handlers.failed?.(e.payload)),
  ]).catch((reason) => {
    const msg = `cannot subscribe to build events: ${String(reason)}`;
    console.error(msg);
    handlers.onFailure?.(msg);
    return [] as UnlistenFn[];
  });
}

export function onTaskEvents(handlers: {
  progress?: (p: TaskProgress) => void;
  done?: (d: TaskDone) => void;
  error?: (e: TaskError) => void;
  onFailure?: (reason: string) => void;
}): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<TaskProgress>("task:progress", (e) => handlers.progress?.(e.payload)),
    listen<TaskDone>("task:done", (e) => handlers.done?.(e.payload)),
    listen<TaskError>("task:error", (e) => handlers.error?.(e.payload)),
  ]).catch((reason) => {
    const msg = `cannot subscribe to task events: ${String(reason)}`;
    console.error(msg);
    handlers.onFailure?.(msg);
    return [] as UnlistenFn[];
  });
}

/**
 * A duration as a person would say it. Deliberately coarse: a build ETA that reads
 * "3 min" and drifts is honest, one that reads "3 min 12 s" and drifts looks broken.
 */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min`;
}

export function formatBytes(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

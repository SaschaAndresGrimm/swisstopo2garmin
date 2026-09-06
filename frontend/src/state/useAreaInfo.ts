import { useEffect, useState } from "react";
import { api } from "./api";
import type { AreaInfo, AreaSelection } from "./api";

/** The LV95 rectangle an area selection covers. */
export interface Lv95Box {
  minE: number;
  minN: number;
  maxE: number;
  maxN: number;
}

export function bboxOf(a: AreaSelection): Lv95Box {
  if (a.kind === "corridor") {
    // The corridor's extent is the track's bounds grown by the buffer, which is the
    // same rule the backend applies.
    const m = a.bufferKm * 1000;
    const es = a.points.map((p) => p[0]);
    const ns = a.points.map((p) => p[1]);
    return {
      minE: Math.min(...es) - m,
      minN: Math.min(...ns) - m,
      maxE: Math.max(...es) + m,
      maxN: Math.max(...ns) + m,
    };
  }
  if (a.kind === "polygon" || a.kind === "composite" || a.kind === "circle") {
    // The extent of whatever the shape covers; the backend masks the detail.
    const boxes: Lv95Box[] =
      a.kind === "composite"
        ? a.parts.map(bboxOf)
        : a.kind === "circle"
          ? [
              {
                minE: a.easting - a.radiusKm * 1000,
                minN: a.northing - a.radiusKm * 1000,
                maxE: a.easting + a.radiusKm * 1000,
                maxN: a.northing + a.radiusKm * 1000,
              },
            ]
          : [
              {
                minE: Math.min(...a.points.map((p) => p[0])),
                minN: Math.min(...a.points.map((p) => p[1])),
                maxE: Math.max(...a.points.map((p) => p[0])),
                maxN: Math.max(...a.points.map((p) => p[1])),
              },
            ];
    return {
      minE: Math.min(...boxes.map((b) => b.minE)),
      minN: Math.min(...boxes.map((b) => b.minN)),
      maxE: Math.max(...boxes.map((b) => b.maxE)),
      maxN: Math.max(...boxes.map((b) => b.maxN)),
    };
  }
  if (a.kind === "adminUnits") {
    // Resolved when the units were chosen, so no file access is needed here.
    return { minE: a.minE, minN: a.minN, maxE: a.maxE, maxN: a.maxN };
  }
  return a.kind === "bbox"
    ? { minE: a.minE, minN: a.minN, maxE: a.maxE, maxN: a.maxN }
    : {
        minE: a.easting - a.radiusKm * 1000,
        minN: a.northing - a.radiusKm * 1000,
        maxE: a.easting + a.radiusKm * 1000,
        maxN: a.northing + a.radiusKm * 1000,
      };
}

/**
 * Live area facts and size estimate (SPEC.md FR-54).
 *
 * Debounced because the radius slider fires continuously and each estimate counts
 * features across the GeoPackage R-trees. Both the area step and the content step use
 * this, so the number they show can never disagree.
 */
export function useAreaInfo(
  area: AreaSelection | null,
  deviceId: string,
  preset: string,
  contourM: number,
  relief: string,
): { info: AreaInfo | null; error: string | null } {
  const [info, setInfo] = useState<AreaInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!area) {
      setInfo(null);
      return;
    }
    let live = true;
    const id = setTimeout(() => {
      api
        .describeArea({ ...bboxOf(area), deviceId, preset, contourM, relief })
        .then((d) => {
          // A response that arrived after the inputs changed must not be shown.
          if (live) {
            setInfo(d);
            setError(null);
          }
        })
        .catch((e) => live && setError(String(e)));
    }, 200);
    return () => {
      live = false;
      clearTimeout(id);
    };
  }, [area, deviceId, preset, contourM, relief]);

  return { info, error };
}

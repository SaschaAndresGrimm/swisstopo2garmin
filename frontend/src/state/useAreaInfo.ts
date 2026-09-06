import { useEffect, useState } from "react";
import { api } from "./api";
import type { AreaInfo, AreaSelection } from "./api";

/** The LV95 rectangle an area selection covers. */
export function bboxOf(a: AreaSelection) {
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

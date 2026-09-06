import { useRef, useState } from "react";
import { api } from "../state/api";
import type { AreaSelection, TrackImport as Imported } from "../state/api";
import type { T } from "../i18n";

/**
 * GPX / FIT import and corridor buffering (SPEC.md FR-38..FR-40).
 *
 * The file is read in the frontend and its bytes are sent to Rust, so importing needs
 * no native file dialog and no filesystem permission for what is a read-only operation.
 */
export function TrackImport({
  t,
  area,
  onArea,
}: {
  t: T;
  area: AreaSelection | null;
  onArea: (a: AreaSelection) => void;
}) {
  const input = useRef<HTMLInputElement | null>(null);
  const [imported, setImported] = useState<Imported | null>(null);
  const [bufferKm, setBufferKm] = useState(5);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async (file: File) => {
    setBusy(true);
    setError(null);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const track = await api.importTrack(file.name, bytes);
      setImported(track);
      onArea({
        kind: "corridor",
        name: track.name,
        bufferKm,
        points: track.points as [number, number][],
      });
    } catch (e) {
      setError(String(e));
      setImported(null);
    } finally {
      setBusy(false);
      // Allow re-importing the same file after a correction.
      if (input.current) input.current.value = "";
    }
  };

  const setBuffer = (km: number) => {
    setBufferKm(km);
    if (area?.kind === "corridor") onArea({ ...area, bufferKm: km });
  };

  return (
    <div className="field">
      <label htmlFor="track">{t("track.import")}</label>
      <div className="row tight">
        <input
          id="track"
          ref={input}
          type="file"
          accept=".gpx,.fit"
          disabled={busy}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void load(f);
          }}
        />
      </div>
      <p className="muted small">{t("track.hint")}</p>

      {imported && (
        <>
          <dl className="facts">
            <div>
              <dt>{t("track.name")}</dt>
              <dd>{imported.name}</dd>
            </div>
            <div>
              <dt>{t("track.length")}</dt>
              <dd>{imported.lengthKm.toFixed(1)} km</dd>
            </div>
            <div>
              <dt>{t("track.ascent")}</dt>
              <dd>{imported.ascentM > 0 ? `${imported.ascentM.toFixed(0)} m` : "—"}</dd>
            </div>
            <div>
              <dt>{t("track.points")}</dt>
              <dd>
                {imported.points.length} / {imported.originalPoints}
              </dd>
            </div>
          </dl>
          {!imported.fullyWithinSwitzerland && (
            <p className="error small">{t("track.partlyOutside")}</p>
          )}
          <label htmlFor="buffer">{t("track.buffer", { km: bufferKm })}</label>
          <input
            id="buffer"
            type="range"
            min={0.5}
            max={50}
            step={0.5}
            value={bufferKm}
            onChange={(e) => setBuffer(Number(e.target.value))}
          />
        </>
      )}

      {error && <p className="error small">{error}</p>}
    </div>
  );
}

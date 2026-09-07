import { useEffect, useState } from "react";
import { api, formatBytes } from "../state/api";
import type { RasterPreview } from "../state/bindings";
import { LayerPanel } from "../components/LayerPanel";
import type {
  AreaSelection,
  LabelLanguage,
  Palette,
  PresetId,
  PresetInfo,
  ReliefDetail,
} from "../state/api";
import { useAreaInfo } from "../state/useAreaInfo";
import { SizeEstimate } from "../components/SizeEstimate";
import { OverBudget } from "../components/OverBudget";
import type { T } from "../i18n";

/**
 * Content configuration (SPEC.md FR-50..FR-53).
 *
 * A preset whose data is not downloaded is shown disabled **with the reason**, never
 * hidden: a user looking for ski routes must be able to see that the feature exists
 * and what it needs.
 */
export function ContentStep({
  t,
  preset,
  onPreset,
  contourM,
  onContourM,
  relief,
  onRelief,
  palette,
  onPalette,
  slopeClasses,
  routing,
  onRouting,
  addresses,
  onAddresses,
  raster,
  onRaster,
  onSlopeClasses,
  labelLanguage,
  onLabelLanguage,
  supportsDem,
  area,
  deviceId,
  excluded,
  onExcluded,
  onNext,
  onBack,
}: {
  t: T;
  preset: PresetId;
  onPreset: (p: PresetId, contourM: number, indexM: number) => void;
  contourM: number;
  onContourM: (m: number) => void;
  relief: ReliefDetail;
  onRelief: (r: ReliefDetail) => void;
  palette: Palette;
  onPalette: (p: Palette) => void;
  slopeClasses: boolean;
  routing: boolean;
  onRouting: (v: boolean) => void;
  addresses: boolean;
  onAddresses: (v: boolean) => void;
  raster: boolean;
  onRaster: (v: boolean) => void;
  onSlopeClasses: (on: boolean) => void;
  labelLanguage: LabelLanguage;
  onLabelLanguage: (l: LabelLanguage) => void;
  supportsDem: boolean;
  /** The chosen area, so the estimate can update as content changes (FR-54). */
  area: AreaSelection | null;
  deviceId: string;
  excluded: string[];
  onExcluded: (ids: string[]) => void;
  onNext: () => void;
  onBack: () => void;
}) {
  const [presets, setPresets] = useState<PresetInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [rasterInfo, setRasterInfo] = useState<RasterPreview | null>(null);

  useEffect(() => {
    api.listPresets().then(setPresets).catch((e) => setError(String(e)));
  }, []);

  // What an overlay would cost is worth knowing *before* a download of one tile per
  // second, so the plan is previewed whenever the area changes -- and it is what tells
  // the user their area is too large to raster at a useful resolution.
  useEffect(() => {
    if (!area) {
      setRasterInfo(null);
      return;
    }
    let live = true;
    api
      .previewRaster(area, deviceId)
      .then((p) => live && setRasterInfo(p))
      .catch(() => live && setRasterInfo(null));
    return () => {
      live = false;
    };
  }, [area, deviceId]);

  // Routing is a content choice, so the estimate on this screen has to include it --
  // it is 12 % of a typical map and the whole point of showing a budget is that it
  // reflects what is about to be built.
  const { info } = useAreaInfo(area, deviceId, preset, contourM, relief, routing, addresses);

  const intervals = [10, 20, 50, 100];
  const reliefOptions: ReliefDetail[] = ["off", "gentle", "detailed"];

  return (
    <section className="screen">
      <h2>{t("step.content")}</h2>
      <p className="muted">{t("content.intro")}</p>

      <div className="presets">
        {presets.map((p) => {
          const id = p.id as PresetId;
          const disabled = !p.dataReady;
          return (
            <button
              key={p.id}
              type="button"
              className={`preset ${preset === id ? "current" : ""}`}
              aria-pressed={preset === id}
              disabled={disabled}
              onClick={() => onPreset(id, p.contourM, p.indexContourM)}
            >
              <div className="preset-name">{t(`preset.${p.id}`)}</div>
              <p className="muted small">{t(`preset.${p.id}.desc`)}</p>
              {disabled && (
                <p className="error small">
                  {t("content.needsData", { what: p.missing.join(", ") })}
                </p>
              )}
            </button>
          );
        })}
      </div>

      <div className="field">
        <label htmlFor="contour">{t("content.contours")}</label>
        <div className="row tight" id="contour">
          {intervals.map((m) => (
            <button
              key={m}
              type="button"
              className={contourM === m ? "current" : ""}
              aria-pressed={contourM === m}
              onClick={() => onContourM(m)}
            >
              {m} m
            </button>
          ))}
          <button
            type="button"
            className={contourM === 0 ? "current" : ""}
            aria-pressed={contourM === 0}
            onClick={() => onContourM(0)}
          >
            {t("common.off")}
          </button>
        </div>
        <p className="muted small">{t("content.contourHint")}</p>
      </div>

      <div className="field">
        <label htmlFor="labels">{t("content.labels")}</label>
        <div className="row tight" id="labels">
          {(["local", "german", "french", "italian", "romansh"] as LabelLanguage[]).map((l) => (
            <button
              key={l}
              type="button"
              className={labelLanguage === l ? "current" : ""}
              aria-pressed={labelLanguage === l}
              onClick={() => onLabelLanguage(l)}
            >
              {t(`content.labels.${l}`)}
            </button>
          ))}
        </div>
        <p className="muted small">{t("content.labelsHint")}</p>
      </div>

      <div className="field">
        <label htmlFor="palette">{t("content.palette")}</label>
        <div className="row tight" id="palette">
          {(["summer", "winter"] as Palette[]).map((p) => (
            <button
              key={p}
              type="button"
              className={palette === p ? "current" : ""}
              aria-pressed={palette === p}
              onClick={() => onPalette(p)}
            >
              {t(`content.palette.${p}`)}
            </button>
          ))}
        </div>
        <p className="muted small">{t("content.paletteHint")}</p>
      </div>

      <div className="field">
        <label htmlFor="relief">{t("content.relief")}</label>
        <div className="row tight" id="relief">
          {reliefOptions.map((r) => (
            <button
              key={r}
              type="button"
              className={relief === r ? "current" : ""}
              aria-pressed={relief === r}
              disabled={!supportsDem && r !== "off"}
              onClick={() => onRelief(r)}
            >
              {t(`content.relief.${r}`)}
            </button>
          ))}
        </div>
        <p className="muted small">
          {supportsDem ? t("content.reliefHint") : t("content.reliefUnsupported")}
        </p>
      </div>

      <div className="field">
        <label className="check">
          <input
            type="checkbox"
            checked={slopeClasses}
            onChange={(e) => onSlopeClasses(e.target.checked)}
          />
          <span>{t("content.slope")}</span>
        </label>
        <p className="muted small">{t("content.slopeHint")}</p>
      </div>

      {/* Routing. Off by default and labelled as untested, because nothing about it
          has been on a device -- including the assumption that a directionally
          separated carriageway is digitised the way traffic moves. */}
      <div className="field">
        <label className="check">
          <input
            type="checkbox"
            checked={routing}
            onChange={(e) => onRouting(e.target.checked)}
          />
          <span>{t("content.routing")}</span>
        </label>
        <p className="muted small">{t("content.routingHint")}</p>
        {routing && <p className="muted small">{t("content.routingWarning")}</p>}
      </div>

      {/* Address search. Its own dataset, so the Data screen has to have it. */}
      <div className="field">
        <label className="check">
          <input
            type="checkbox"
            checked={addresses}
            onChange={(e) => onAddresses(e.target.checked)}
          />
          <span>{t("content.addresses")}</span>
        </label>
        <p className="muted small">{t("content.addressesHint")}</p>
      </div>

      {/* Raster overlay of the paper map. Shown disabled *with the reason* when the
          device profile states no Custom Map limits, following the same rule as a
          preset whose data is missing: a user must be able to see that the feature
          exists and what it needs. */}
      <div className="field">
        <label className="check">
          <input
            type="checkbox"
            checked={raster && (rasterInfo?.available ?? false)}
            disabled={!rasterInfo?.available}
            onChange={(e) => onRaster(e.target.checked)}
          />
          <span>{t("content.raster")}</span>
        </label>
        <p className="muted small">{t("content.rasterHint")}</p>
        {rasterInfo && !rasterInfo.available && (
          <p className="muted small">{rasterInfo.unavailableBecause}</p>
        )}
        {raster && rasterInfo?.available && (
          <>
            <p className="muted small">
              {t("content.rasterPlan", {
                tiles: rasterInfo.tiles,
                cols: rasterInfo.cols,
                rows: rasterInfo.rows,
                res: rasterInfo.mPerPx.toFixed(1),
                size: formatBytes(rasterInfo.approxBytes),
                seconds: Math.round(rasterInfo.approxSeconds),
              })}
            </p>
            {/* The planner's own notes, which say why the resolution is what it is --
                including when it is too coarse to be worth having. */}
            {rasterInfo.notes.map((n) => (
              <p className="muted small" key={n}>{n}</p>
            ))}
            <p className="muted small">{t("content.rasterWarning")}</p>
          </>
        )}
      </div>

      <LayerPanel t={t} preset={preset} excluded={excluded} onExcluded={onExcluded} />

      {info && <SizeEstimate t={t} info={info} />}
      {info && <OverBudget t={t} info={info} />}

      {error && <p className="error">{t("data.error", { message: error })}</p>}

      <div className="row">
        <button type="button" onClick={onBack}>{t("common.back")}</button>
        <button type="button" className="primary" onClick={onNext}>{t("common.next")}</button>
      </div>
    </section>
  );
}

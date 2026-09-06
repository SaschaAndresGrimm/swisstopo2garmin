import { useEffect, useState } from "react";
import { api } from "../state/api";
import { LayerPanel } from "../components/LayerPanel";
import type { PresetId, PresetInfo, ReliefDetail } from "../state/api";
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
  supportsDem,
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
  supportsDem: boolean;
  excluded: string[];
  onExcluded: (ids: string[]) => void;
  onNext: () => void;
  onBack: () => void;
}) {
  const [presets, setPresets] = useState<PresetInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listPresets().then(setPresets).catch((e) => setError(String(e)));
  }, []);

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

      <LayerPanel t={t} preset={preset} excluded={excluded} onExcluded={onExcluded} />

      {error && <p className="error">{t("data.error", { message: error })}</p>}

      <div className="row">
        <button type="button" onClick={onBack}>{t("common.back")}</button>
        <button type="button" className="primary" onClick={onNext}>{t("common.next")}</button>
      </div>
    </section>
  );
}

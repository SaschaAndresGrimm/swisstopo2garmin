import { formatBytes } from "../state/api";
import type { AreaInfo } from "../state/api";
import type { T } from "../i18n";

/**
 * Estimate, device budget and remaining margin, side by side (SPEC.md FR-63).
 *
 * The estimate's provenance is shown, not hidden: an area-only estimate (swissTLM3D
 * not yet downloaded) and one refit from the user's own builds are very different
 * claims, and a user deciding whether to trust it needs to know which they have.
 */
export function SizeEstimate({ t, info }: { t: T; info: AreaInfo }) {
  const margin = info.budgetBytes - info.estimatedBytes;
  const pct = info.budgetBytes > 0 ? (info.estimatedBytes / info.budgetBytes) * 100 : 0;

  return (
    <div className="estimate">
      <dl className="facts">
        <div>
          <dt>{t("area.size")}</dt>
          <dd>{info.areaKm2.toFixed(0)} km²</dd>
        </div>
        <div>
          <dt>{t("estimate.predicted")}</dt>
          <dd className={info.overBudget ? "error" : ""}>{formatBytes(info.estimatedBytes)}</dd>
        </div>
        <div>
          <dt>{t("estimate.budget")}</dt>
          <dd>{formatBytes(info.budgetBytes)}</dd>
        </div>
        <div>
          <dt>{t("estimate.margin")}</dt>
          <dd className={margin < 0 ? "error" : ""}>
            {margin >= 0 ? formatBytes(margin) : `−${formatBytes(-margin)}`}
          </dd>
        </div>
      </dl>

      <div className="track" role="progressbar" aria-valuemin={0} aria-valuemax={100}
           aria-valuenow={Math.round(Math.min(pct, 999))}
           aria-label={t("estimate.budget")}>
        <div
          className={`fill ${info.overBudget ? "over" : ""}`}
          style={{ width: `${Math.min(pct, 100)}%` }}
        />
      </div>

      <p className="muted small">
        {!info.countedFeatures
          ? t("estimate.areaOnly")
          : info.calibrated
            ? t("estimate.calibrated", { n: info.modelSamples })
            : t("estimate.shippedModel")}
      </p>
    </div>
  );
}

import { formatBytes } from "../state/api";
import type { AreaInfo } from "../state/api";
import type { T } from "../i18n";

/**
 * How much of the budget is used, in words a fraction of a percent can carry.
 *
 * A 9.6 MB map against a 3.0 GB budget is 0.3 %, which renders as an invisible sliver
 * and reads as a broken progress bar. The number is what is informative at that end of
 * the range, and "under 1 %" is more honest than "0 %".
 */
function share(pct: number): string {
  if (pct >= 10) return `${pct.toFixed(0)} %`;
  if (pct >= 1) return `${pct.toFixed(1)} %`;
  if (pct > 0) return "< 1 %";
  return "0 %";
}

/**
 * Estimate, device budget and how much of it is used (SPEC.md FR-63).
 *
 * The estimate's provenance is shown, not hidden: an area-only estimate (swissTLM3D
 * not yet downloaded) and one refit from the user's own builds are very different
 * claims, and a user deciding whether to trust it needs to know which they have. The
 * wording says what that means for them rather than what the model did — "refitted from
 * 45 of your own builds" told the user nothing they could act on.
 */
export function SizeEstimate({ t, info }: { t: T; info: AreaInfo }) {
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
        {/* Replaces "margin left", which for a small map restated the budget to the same
            rounded figure -- "3.0 GB" against a budget of "3.0 GB" says nothing. */}
        <div>
          <dt>{t("estimate.used")}</dt>
          <dd className={info.overBudget ? "error" : ""}>{share(pct)}</dd>
        </div>
      </dl>

      <div className="track" role="progressbar" aria-valuemin={0} aria-valuemax={100}
           aria-valuenow={Math.round(Math.min(pct, 999))}
           aria-valuetext={share(pct)}
           aria-label={t("estimate.budget")}>
        {/* A floor of half a percent, so a map that is genuinely tiny still shows *some*
            fill. A zero-width bar looks like a bar that failed to render. */}
        <div
          className={`fill ${info.overBudget ? "over" : ""}`}
          style={{ width: `${Math.min(Math.max(pct, pct > 0 ? 0.5 : 0), 100)}%` }}
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

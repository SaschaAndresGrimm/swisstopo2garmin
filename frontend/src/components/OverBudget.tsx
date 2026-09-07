import { formatBytes } from "../state/api";
import type { AreaInfo } from "../state/bindings";
import type { T } from "../i18n";

/**
 * What to do about a map that is predicted not to fit (SPEC.md §12).
 *
 * Replaces a single sentence that named two remedies whether or not they applied, which
 * read as the app not knowing its own state: it suggested coarsening contours that were
 * already off. Which remedies apply to *this* recipe is decided in
 * `s2g_core::estimate::budget_verdict`, where it is tested; this only translates them.
 */
export function OverBudget({ t, info }: { t: T; info: AreaInfo }) {
  if (!info.overBudget) return null;
  return (
    <div className="notice error" role="status">
      <strong>
        {t("budget.over", {
          over: formatBytes(info.overshootBytes),
          budget: formatBytes(info.budgetBytes),
        })}
      </strong>
      <ul className="small">
        {info.remedies.map((r) => (
          <li key={r}>{t(`remedy.${r}`)}</li>
        ))}
      </ul>
    </div>
  );
}

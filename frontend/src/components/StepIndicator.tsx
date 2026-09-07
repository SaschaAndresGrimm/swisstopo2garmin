import type { T } from "../i18n";

export const STEPS = ["device", "area", "content", "build", "install"] as const;
export type Step = (typeof STEPS)[number];

/**
 * The wizard's progress, and what is reachable from here.
 *
 * A step that cannot be opened yet is *dimmed and titled*, not merely dimmed: a greyed
 * "Install" with no explanation reads as a broken button rather than as a step waiting
 * on something. A `title` and the same text as a screen-reader description say which,
 * and the tick on a completed step distinguishes "done" from "not yet" -- both of which
 * dimming alone rendered identically.
 */
export function StepIndicator({
  t,
  current,
  reachable,
  onSelect,
}: {
  t: T;
  current: Step;
  reachable: Set<Step>;
  onSelect: (s: Step) => void;
}) {
  const currentIndex = STEPS.indexOf(current);
  return (
    <nav className="steps" aria-label={t("app.title")}>
      <ol>
        {STEPS.map((s, i) => {
          const isCurrent = s === current;
          const enabled = reachable.has(s);
          const done = enabled && i < currentIndex;
          const why = enabled ? undefined : t(`step.locked.${s}`);
          return (
            <li key={s}>
              <button
                type="button"
                className={`step${isCurrent ? " current" : ""}${done ? " done" : ""}${
                  enabled ? "" : " locked"
                }`}
                aria-current={isCurrent ? "step" : undefined}
                disabled={!enabled}
                title={why}
                onClick={() => onSelect(s)}
              >
                <span className="step-num" aria-hidden="true">
                  {done ? "✓" : i + 1}
                </span>
                <span>{t(`step.${s}`)}</span>
                {/* Read aloud, and read by anyone who cannot see the dimming. */}
                {why && <span className="sr-only"> — {why}</span>}
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

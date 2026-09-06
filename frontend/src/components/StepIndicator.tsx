import type { T } from "../i18n";

export const STEPS = ["device", "area", "content", "build", "install"] as const;
export type Step = (typeof STEPS)[number];

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
  return (
    <nav className="steps" aria-label={t("app.title")}>
      <ol>
        {STEPS.map((s, i) => {
          const isCurrent = s === current;
          const enabled = reachable.has(s);
          return (
            <li key={s}>
              <button
                type="button"
                className={isCurrent ? "step current" : "step"}
                aria-current={isCurrent ? "step" : undefined}
                disabled={!enabled}
                onClick={() => onSelect(s)}
              >
                <span className="step-num">{i + 1}</span>
                <span>{t(`step.${s}`)}</span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

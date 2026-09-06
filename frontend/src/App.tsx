import { useMemo, useState } from "react";
import { StepIndicator, type Step } from "./components/StepIndicator";
import { DataScreen } from "./steps/DataScreen";
import { detectLang, makeT, type Lang } from "./i18n";

type View = "data" | Step;

export default function App() {
  const [lang, setLang] = useState<Lang>(detectLang);
  const [view, setView] = useState<View>("data");
  const t = useMemo(() => makeT(lang), [lang]);

  // Only the Data screen exists so far; the five build steps land in later
  // milestones (PLAN.md M6). Nothing else is reachable yet.
  const reachable = useMemo(() => new Set<Step>(), []);

  return (
    <div className="app">
      <header>
        <h1>{t("app.title")}</h1>
        <div className="spacer" />
        <nav className="views">
          <button
            type="button"
            className={view === "data" ? "current" : ""}
            onClick={() => setView("data")}
          >
            {t("nav.data")}
          </button>
        </nav>
        <select
          aria-label="language"
          value={lang}
          onChange={(e) => setLang(e.target.value as Lang)}
        >
          <option value="de">DE</option>
          <option value="fr">FR</option>
          <option value="it">IT</option>
          <option value="en">EN</option>
        </select>
      </header>

      <StepIndicator
        t={t}
        current={view === "data" ? "device" : view}
        reachable={reachable}
        onSelect={(s) => setView(s)}
      />

      <main>{view === "data" ? <DataScreen t={t} /> : <p>{t("common.notImplemented")}</p>}</main>

      <footer>{t("app.attribution")}</footer>
    </div>
  );
}

import { useCallback, useMemo, useState } from "react";
import { StepIndicator, type Step } from "./components/StepIndicator";
import { DataScreen } from "./steps/DataScreen";
import { DeviceStep } from "./steps/DeviceStep";
import { AreaStep } from "./steps/AreaStep";
import { ContentStep } from "./steps/ContentStep";
import { BuildStep } from "./steps/BuildStep";
import { InstallStep } from "./steps/InstallStep";
import { RecipeLibrary } from "./components/RecipeLibrary";
import { detectLang, makeT, type Lang } from "./i18n";
import { api } from "./state/api";
import type {
  AreaSelection,
  BuildFinished,
  PresetId,
  Recipe,
  ReliefDetail,
} from "./state/api";

type View = "data" | Step;

export default function App() {
  const [lang, setLang] = useState<Lang>(detectLang);
  const [view, setView] = useState<View>("device");
  const t = useMemo(() => makeT(lang), [lang]);

  // The recipe is assembled across the steps and is the only build input.
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [supportsDem, setSupportsDem] = useState(true);
  const [area, setArea] = useState<AreaSelection | null>(null);
  const [preset, setPreset] = useState<PresetId>("hiking");
  const [contourM, setContourM] = useState(20);
  const [indexM, setIndexM] = useState(100);
  const [relief, setRelief] = useState<ReliefDetail>("gentle");
  const [excluded, setExcluded] = useState<string[]>([]);
  const [built, setBuilt] = useState<BuildFinished | null>(null);

  const recipe: Recipe | null = useMemo(() => {
    if (!deviceId || !area) return null;
    const name =
      area.kind === "place" ? `${area.name} ${area.radiusKm} km` : t("recipe.customArea");
    return {
      schemaVersion: 1,
      name,
      deviceId,
      area,
      preset,
      contours: { intervalM: contourM, indexM, simplifyM: 8.0 },
      relief,
      excludedLayers: excluded,
    };
  }, [deviceId, area, preset, contourM, indexM, relief, excluded, t]);

  // A step is reachable only once the steps it depends on are satisfied, so the
  // indicator cannot jump to a screen that would have nothing to work with.
  const reachable = useMemo(() => {
    const s = new Set<Step>(["device"]);
    if (deviceId) s.add("area");
    if (deviceId && area) s.add("content");
    if (recipe) s.add("build");
    if (built) s.add("install");
    return s;
  }, [deviceId, area, recipe, built]);

  const go = useCallback((v: View) => setView(v), []);

  /** Relief is only offered where the device profile supports a DEM. */
  const selectDevice = useCallback(async (id: string) => {
    setDeviceId(id);
    const d = (await api.listDevices()).find((x) => x.id === id);
    setSupportsDem(d?.supportsDem ?? false);
    if (!d?.supportsDem) setRelief("off");
  }, []);

  /** Restore a saved recipe into every step, then jump to the build step. */
  const applyRecipe = useCallback(
    async (r: Recipe) => {
      await selectDevice(r.deviceId);
      setArea(r.area);
      setPreset(r.preset);
      setContourM(r.contours.intervalM);
      setIndexM(r.contours.indexM);
      setRelief(r.relief);
      setExcluded(r.excludedLayers);
      setBuilt(null);
      setView("build");
    },
    [selectDevice],
  );

  return (
    <div className="app">
      <header>
        <h1>{t("app.title")}</h1>
        <div className="spacer" />
        <nav className="views">
          <button
            type="button"
            className={view === "data" ? "current" : ""}
            onClick={() => go("data")}
          >
            {t("nav.data")}
          </button>
          <button
            type="button"
            className={view !== "data" ? "current" : ""}
            onClick={() => go(deviceId ? "area" : "device")}
          >
            {t("nav.build")}
          </button>
        </nav>
        <select
          aria-label={t("app.language")}
          value={lang}
          onChange={(e) => setLang(e.target.value as Lang)}
        >
          <option value="de">DE</option>
          <option value="fr">FR</option>
          <option value="it">IT</option>
          <option value="en">EN</option>
        </select>
      </header>

      {view !== "data" && (
        <StepIndicator
          t={t}
          current={view as Step}
          reachable={reachable}
          onSelect={(s) => go(s)}
        />
      )}

      <main>
        {view === "data" && <DataScreen t={t} />}

        {view === "device" && (
          <DeviceStep
            t={t}
            selected={deviceId}
            onSelect={(id) => void selectDevice(id)}
            onNext={() => go("area")}
            before={<RecipeLibrary t={t} recipe={null} onLoad={(r) => void applyRecipe(r)} />}
          />
        )}

        {view === "area" && deviceId && (
          <AreaStep
            t={t}
            deviceId={deviceId}
            area={area}
            onArea={setArea}
            onNext={() => go("content")}
            onBack={() => go("device")}
          />
        )}

        {view === "content" && (
          <ContentStep
            t={t}
            preset={preset}
            onPreset={(p, c, i) => {
              setPreset(p);
              setContourM(c);
              setIndexM(i);
            }}
            contourM={contourM}
            onContourM={setContourM}
            relief={relief}
            onRelief={setRelief}
            supportsDem={supportsDem}
            excluded={excluded}
            onExcluded={setExcluded}
            onNext={() => go("build")}
            onBack={() => go("area")}
          />
        )}

        {view === "build" && recipe && (
          <BuildStep
            t={t}
            recipe={recipe}
            onDone={(r) => {
              setBuilt(r);
              go("install");
            }}
            onBack={() => go("content")}
            before={<RecipeLibrary t={t} recipe={recipe} onLoad={(r) => void applyRecipe(r)} />}
          />
        )}

        {view === "install" && built && deviceId && (
          <InstallStep
            t={t}
            build={built}
            deviceId={deviceId}
            mapName={recipe?.name ?? "swisstopo"}
            onBack={() => go("build")}
          />
        )}

        {/* A step reached without its prerequisites shows why rather than a blank pane. */}
        {view !== "data" && !reachable.has(view as Step) && (
          <section className="screen">
            <p className="muted">{t("common.completeEarlierSteps")}</p>
          </section>
        )}
      </main>

      <footer>{t("app.attribution")}</footer>
    </div>
  );
}

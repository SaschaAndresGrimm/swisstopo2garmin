import { useEffect, useState } from "react";
import { api, formatBytes } from "../state/api";
import type { DeviceOverrideInfo, DeviceSummary } from "../state/api";
import type { T } from "../i18n";

/**
 * Record limits you have measured yourself (SPEC.md FR-DEV3).
 *
 * Most shipped limits are `community` or `assumed`, and carry a safety margin because of
 * it. Someone who has measured their own device knows it better than we do, so an
 * override replaces the number *and* the confidence — the margin for a guess should not
 * apply to a measurement.
 *
 * Stored in the settings file rather than in the device profiles, so an app update that
 * ships new profiles cannot silently discard them.
 */
export function DeviceLimits({ t, device }: { t: T; device: DeviceSummary }) {
  const [value, setValue] = useState<DeviceOverrideInfo | null>(null);
  const [budgetMb, setBudgetMb] = useState("");
  const [note, setNote] = useState("");
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSaved(false);
    api
      .deviceOverride(device.id)
      .then((o) => {
        setValue(o);
        setBudgetMb(o.mapBudgetBytes ? String(Math.round(o.mapBudgetBytes / 1e6)) : "");
        setNote(o.note ?? "");
      })
      .catch((e) => setError(String(e)));
  }, [device.id]);

  const save = async () => {
    setError(null);
    try {
      const mb = Number(budgetMb);
      const next: DeviceOverrideInfo = {
        mapBudgetBytes: budgetMb.trim() !== "" && mb > 0 ? Math.round(mb * 1e6) : null,
        maxImgBytes: value?.maxImgBytes ?? null,
        maxTilesPerMapset: value?.maxTilesPerMapset ?? null,
        note: note.trim() || null,
      };
      await api.setDeviceOverride(device.id, next);
      setValue(next);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const clear = async () => {
    setError(null);
    try {
      await api.setDeviceOverride(device.id, null);
      setValue(null);
      setBudgetMb("");
      setNote("");
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const overridden = value?.mapBudgetBytes != null;

  return (
    <details className="notes">
      <summary>
        {t("device.limits.title")}
        {overridden && <span className="badge">{t("device.limits.overridden")}</span>}
      </summary>

      <p className="muted small">{t("device.limits.intro")}</p>

      <div className="field">
        <label htmlFor="budget">
          {t("device.limits.budget", { shipped: formatBytes(device.budgetBytes) })}
        </label>
        <div className="row tight">
          <input
            id="budget"
            inputMode="numeric"
            value={budgetMb}
            placeholder={String(Math.round(device.budgetBytes / 1e6))}
            onChange={(e) => {
              setBudgetMb(e.target.value);
              setSaved(false);
            }}
          />
          <span className="muted small">MB</span>
        </div>
      </div>

      <div className="field">
        <label htmlFor="note">{t("device.limits.note")}</label>
        <input
          id="note"
          value={note}
          placeholder={t("device.limits.notePlaceholder")}
          onChange={(e) => {
            setNote(e.target.value);
            setSaved(false);
          }}
        />
      </div>

      <div className="row tight">
        <button type="button" onClick={() => void save()}>{t("device.limits.save")}</button>
        {overridden && (
          <button type="button" onClick={() => void clear()}>{t("device.limits.clear")}</button>
        )}
      </div>
      {saved && <p className="muted small">{t("device.limits.saved")}</p>}
      {error && <p className="error small">{error}</p>}
    </details>
  );
}

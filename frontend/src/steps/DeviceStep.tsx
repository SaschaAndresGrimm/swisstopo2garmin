import { useCallback, useEffect, useState } from "react";
import { api, formatBytes } from "../state/api";
import type { ConnectedDevice, DeviceSummary } from "../state/api";
import type { T } from "../i18n";

/**
 * Device selection (SPEC.md FR-20..FR-22).
 *
 * The confidence level of each profile is shown, not hidden: Garmin publishes none of
 * these limits, and a user should be able to see when a number is a community report
 * rather than something measured (FR-DEV1).
 */
export function DeviceStep({
  t,
  selected,
  onSelect,
  onNext,
}: {
  t: T;
  selected: string | null;
  onSelect: (id: string) => void;
  onNext: () => void;
}) {
  const [devices, setDevices] = useState<DeviceSummary[]>([]);
  const [connected, setConnected] = useState<ConnectedDevice[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [d, c] = await Promise.all([api.listDevices(), api.detectDevices()]);
      setDevices(d);
      setConnected(c);
      // Preselect a connected device, but never override an explicit choice.
      if (!selected) {
        const auto = c.find((x) => x.profileId)?.profileId ?? d[0]?.id;
        if (auto) onSelect(auto);
      }
    } catch (e) {
      setError(String(e));
    }
  }, [selected, onSelect]);

  useEffect(() => {
    void load();
    // eslint-disable-next-line
  }, []);

  const groups = ["edge", "wrist"] as const;

  return (
    <section className="screen">
      <h2>{t("step.device")}</h2>
      <p className="muted">{t("device.intro")}</p>

      {connected.length > 0 && (
        <div className="notice">
          <strong>{t("device.connected")}</strong>
          <ul>
            {connected.map((c) => (
              <li key={c.mount}>
                {c.model ?? t("device.unknownModel")} — <span className="mono">{c.mount}</span>
                {c.freeBytes !== null ? ` · ${formatBytes(c.freeBytes)} ${t("device.free")}` : ""}
                {c.existingMaps.length > 0
                  ? ` · ${t("device.existingMaps", { n: c.existingMaps.length })}`
                  : ""}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="row">
        <button type="button" onClick={() => void load()}>{t("device.rescan")}</button>
      </div>

      {groups.map((g) => {
        const items = devices.filter((d) =>
          g === "wrist" ? d.screenClass === "wrist" : d.screenClass !== "wrist",
        );
        if (items.length === 0) return null;
        return (
          <div key={g}>
            <h3 className="grouphead">{t(`device.group.${g}`)}</h3>
            <div className="devices">
              {items.map((d) => (
                <button
                  key={d.id}
                  type="button"
                  className={`device ${selected === d.id ? "current" : ""}`}
                  aria-pressed={selected === d.id}
                  onClick={() => onSelect(d.id)}
                >
                  <div className="device-name">
                    {d.displayName}
                    {d.connected && <span className="badge">{t("device.attached")}</span>}
                  </div>
                  <dl className="device-facts">
                    <div>
                      <dt>{t("device.budget")}</dt>
                      <dd>{formatBytes(d.budgetBytes)}</dd>
                    </div>
                    <div>
                      <dt>{t("device.relief")}</dt>
                      <dd>{d.supportsDem ? t("common.yes") : t("common.no")}</dd>
                    </div>
                    <div>
                      <dt>{t("device.limits")}</dt>
                      <dd>
                        <span className={d.confidenceVerified ? "conf verified" : "conf unverified"}>
                          {t(`device.confidence.${d.confidence}`)}
                        </span>
                      </dd>
                    </div>
                  </dl>
                  {!d.confidenceVerified && (
                    <p className="muted small">{t("device.unverifiedHint")}</p>
                  )}
                </button>
              ))}
            </div>
          </div>
        );
      })}

      {selected && (
        <details className="notes">
          <summary>{t("device.notes")}</summary>
          <p className="small">
            {devices.find((d) => d.id === selected)?.confidenceNotes || "—"}
          </p>
        </details>
      )}

      {error && <p className="error">{t("data.error", { message: error })}</p>}

      <div className="row">
        <button type="button" className="primary" disabled={!selected} onClick={onNext}>
          {t("common.next")}
        </button>
      </div>
    </section>
  );
}

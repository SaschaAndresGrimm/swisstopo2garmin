import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, formatBytes } from "../state/api";
import type {
  BuildFinished,
  ConnectedDevice,
  InstallInstructions,
  InstallPlan,
  UsbDeviceInfo,
} from "../state/api";
import { UnmountedDevices } from "../components/UnmountedDevices";
import type { T } from "../i18n";

/**
 * Install to the device (SPEC.md FR-80..FR-84).
 *
 * Nothing is written without an explicit confirmation, an existing map is never
 * silently replaced, and the plan is shown before it is carried out.
 */
export function InstallStep({
  t,
  build,
  deviceId,
  mapName,
  onBack,
}: {
  t: T;
  build: BuildFinished;
  deviceId: string;
  mapName: string;
  onBack: () => void;
}) {
  const [devices, setDevices] = useState<ConnectedDevice[]>([]);
  const [usb, setUsb] = useState<UsbDeviceInfo[]>([]);
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [backup, setBackup] = useState(true);
  const [installed, setInstalled] = useState<string | null>(null);
  const [exported, setExported] = useState<string | null>(null);
  const [howTo, setHowTo] = useState<InstallInstructions | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scan = useCallback(async () => {
    setError(null);
    try {
      const [found, onBus] = await Promise.all([api.detectDevices(), api.usbDevices()]);
      setDevices(found);
      setUsb(onBus);
      setPlan(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void scan();
  }, [scan]);

  // Always available, because the direct install cannot serve every case: a device in
  // MTP mode, an SD card in a reader, or simply a user who would rather copy it.
  useEffect(() => {
    api.installInstructions(deviceId, mapName).then(setHowTo).catch(() => setHowTo(null));
  }, [deviceId, mapName]);

  const exportToFolder = async () => {
    setError(null);
    try {
      const dir = await open({ directory: true, multiple: false, title: t("install.exportTitle") });
      if (typeof dir !== "string") return;
      setExported(await api.exportMap(build.gmapsupp, dir, deviceId, mapName));
    } catch (e) {
      setError(String(e));
    }
  };

  const prepare = async (mount: string) => {
    setError(null);
    try {
      setPlan(await api.planInstall(build.gmapsupp, mount, deviceId, mapName));
    } catch (e) {
      setError(String(e));
    }
  };

  const run = async () => {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      setInstalled(await api.installMap(plan, backup));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="screen">
      <h2>{t("step.install")}</h2>

      <dl className="facts">
        <div>
          <dt>{t("build.size")}</dt>
          <dd>{formatBytes(build.bytes)}</dd>
        </div>
        <div>
          <dt>{t("install.source")}</dt>
          <dd className="mono small">{build.gmapsupp}</dd>
        </div>
      </dl>

      <div className="row">
        <button type="button" onClick={() => void scan()}>{t("device.rescan")}</button>
      </div>

      <UnmountedDevices t={t} devices={usb} />

      {devices.length === 0 && (
        <div className="notice">
          <strong>{t("install.noDevice")}</strong>
          <p className="small">{t("install.noDeviceHint")}</p>
        </div>
      )}

      {devices.map((d) => (
        <div key={d.mount} className="source">
          <div className="source-head">
            <div>
              <h3>{d.model ?? t("device.unknownModel")}</h3>
              <p className="muted small mono">{d.mount}</p>
              {d.freeBytes !== null && (
                <p className="muted small">
                  {formatBytes(d.freeBytes)} {t("device.free")}
                </p>
              )}
            </div>
            <div className="row tight">
              <button type="button" onClick={() => void prepare(d.mount)}>
                {t("install.prepare")}
              </button>
            </div>
          </div>
        </div>
      ))}

      {plan && !installed && (
        <div className="notice">
          <strong>{t("install.confirm")}</strong>
          <dl className="facts">
            <div>
              <dt>{t("install.target")}</dt>
              <dd className="mono small">{plan.target}</dd>
            </div>
            <div>
              <dt>{t("install.free")}</dt>
              <dd className={plan.fits ? "" : "error"}>
                {plan.freeBytes !== null ? formatBytes(plan.freeBytes) : "—"}
              </dd>
            </div>
          </dl>
          {plan.overwrites && (
            <>
              <p className="error small">{t("install.willReplace")}</p>
              <label className="check">
                <input
                  type="checkbox"
                  checked={backup}
                  onChange={(e) => setBackup(e.target.checked)}
                />
                {t("install.backup")}
              </label>
            </>
          )}
          {!plan.fits && <p className="error small">{t("install.tooBig")}</p>}
          <div className="row">
            <button
              type="button"
              className="primary"
              disabled={busy || !plan.fits}
              onClick={() => void run()}
            >
              {busy ? t("install.copying") : t("install.write")}
            </button>
            <button type="button" onClick={() => setPlan(null)} disabled={busy}>
              {t("common.cancel")}
            </button>
          </div>
        </div>
      )}

      {installed && (
        <div className="notice">
          <strong>{t("install.done")}</strong>
          <p className="mono small">{installed}</p>
          <p className="small">{t("install.eject")}</p>
        </div>
      )}

      {/* The escape hatch, offered always rather than only when detection fails. */}
      <div className="notice">
        <strong>{t("install.export")}</strong>
        <p className="small">{t("install.exportHint")}</p>
        {howTo && (
          <dl className="facts">
            <div>
              <dt>{t("install.folder")}</dt>
              <dd className="mono small">{howTo.folders.join("  ")}</dd>
            </div>
            <div>
              <dt>{t("install.filename")}</dt>
              <dd className="mono small">{howTo.filename}</dd>
            </div>
            <div>
              <dt>{t("install.multiple")}</dt>
              <dd>{howTo.multipleMaps ? t("common.yes") : t("common.no")}</dd>
            </div>
          </dl>
        )}
        <div className="row tight">
          <button type="button" onClick={() => void exportToFolder()}>
            {t("install.exportAction")}
          </button>
        </div>
        {exported && (
          <p className="small">
            {t("install.exported")} <span className="mono">{exported}</span>
          </p>
        )}
      </div>

      {error && <p className="error">{t("data.error", { message: error })}</p>}

      <div className="row">
        <button type="button" onClick={onBack}>{t("common.back")}</button>
      </div>
    </section>
  );
}

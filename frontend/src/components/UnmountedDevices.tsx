import type { UsbDeviceInfo } from "../state/api";
import type { T } from "../i18n";

/**
 * Garmin devices attached over USB but not mounted as a filesystem (FR-DEV5).
 *
 * Recent Edge and fēnix models default to MTP, and macOS has no MTP filesystem, so the
 * device never appears under /Volumes. Before this the app reported no device at all
 * while one was plainly plugged in — a silent failure with nothing to act on.
 *
 * Nothing can be written to an MTP device from here, but saying which device it is and
 * what to change turns a dead end into a two-tap fix on the device itself.
 */
export function UnmountedDevices({
  t,
  devices,
  onSelectProfile,
}: {
  t: T;
  devices: UsbDeviceInfo[];
  /** Offered so the right profile can still be chosen without mounting. */
  onSelectProfile?: (id: string) => void;
}) {
  const unmounted = devices.filter((d) => !d.mounted);
  if (unmounted.length === 0) return null;

  return (
    <div className="notice">
      <strong>{t("device.usbOnly")}</strong>
      <ul>
        {unmounted.map((d) => (
          <li key={`${d.model}-${d.serial ?? ""}`}>
            {d.model}
            {d.serial && <span className="mono muted small"> {d.serial}</span>}
            {d.profileId && onSelectProfile && (
              <>
                {" "}
                <button type="button" className="link" onClick={() => onSelectProfile(d.profileId!)}>
                  {t("device.useProfile")}
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
      <p className="small">{t("device.mtpHint")}</p>
      <p className="muted small">{t("device.mtpAlternative")}</p>
    </div>
  );
}

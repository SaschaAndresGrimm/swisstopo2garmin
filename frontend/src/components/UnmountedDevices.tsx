import type { UsbDeviceInfo } from "../state/api";
import type { T } from "../i18n";

/**
 * Garmin devices attached over USB but not mounted as a filesystem (FR-DEV5).
 *
 * Recent Edge and fēnix models default to MTP, and macOS has no MTP filesystem, so the
 * device never appears under /Volumes. Before this the app reported no device at all
 * while one was plainly plugged in — a silent failure with nothing to act on.
 *
 * Nothing can be written to an MTP device from here, so what matters is saying something
 * true about what to do next. The hint used to be "Settings → System → USB Mode →
 * Garmin" for every device, which is **wrong for the Edge 840**: recent Edge models have
 * no USB-mode setting at all, and that advice sent their owners looking for a menu that
 * does not exist. The profile now records whether a device can be a USB drive, so the
 * text says the right thing per device and leads with the route that actually works.
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
      {/* Ordered by what will actually work for this device. When mass storage is not
          available there is no setting to change, so copying the file is not an
          "alternative" -- it is the only route, and is stated first. */}
      {unmounted.every((d) => d.usbMassStorage === false) ? (
        <>
          <p className="small">{t("device.mtpOnly")}</p>
          <p className="small">{t("device.mtpCopy")}</p>
        </>
      ) : (
        <>
          <p className="small">{t("device.mtpHint")}</p>
          <p className="muted small">{t("device.mtpCopy")}</p>
        </>
      )}
    </div>
  );
}

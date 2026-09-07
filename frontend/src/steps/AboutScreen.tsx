import { useEffect, useState } from "react";
import { api } from "../state/api";
import type { AboutInfo } from "../state/bindings";
import type { T } from "../i18n";

/**
 * Attribution, licences and disclaimers (SPEC.md FR-L1…FR-L4).
 *
 * Everything here is required to be visible in the app rather than only in a file next
 * to it. Three things are worth noting about how it is built:
 *
 * * The swisstopo copyright shown is read from the backend, from the same place the
 *   pipeline takes the string it embeds in every map. A hand-written copy here could
 *   drift from what actually travels with the file, and the file is what matters.
 * * Tool versions are the ones installed, not the ones the documentation remembers.
 * * A licence whose text is not bundled says so, rather than offering a path to a file
 *   that is not there.
 */
export function AboutScreen({ t }: { t: T }) {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .about()
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <section className="screen">
      <h2>{t("about.title")}</h2>

      {error && <p className="error">{t("data.error", { message: error })}</p>}
      {!info && !error && <p className="muted small">{t("data.checking")}</p>}

      {info && (
        <>
          <p className="muted small">
            {t("about.version", { version: info.appVersion })}
          </p>

          <h3>{t("about.dataTitle")}</h3>
          <p>{t("about.dataAttribution")}</p>
          {/* The exact bytes embedded in every generated map (FR-L1). */}
          <p className="small">
            {t("about.embedded")} <span className="mono">{info.mapCopyright}</span>
          </p>
          <p className="small">
            <a
              href="https://www.swisstopo.admin.ch/en/terms-of-use-free-geodata-and-geoservices"
              target="_blank"
              rel="noreferrer"
            >
              {t("about.termsLink")}
            </a>
          </p>

          <h3>{t("about.useTitle")}</h3>
          {/* FR-L3, in three sentences that each say one thing. */}
          <ul className="small">
            <li>{t("about.usePersonal")}</li>
            <li>{t("about.useRedistribution")}</li>
            <li>{t("about.useGarmin")}</li>
          </ul>

          <h3>{t("about.licenceTitle")}</h3>
          <p className="small">{t("about.appLicence")}</p>
          <table className="facts small">
            <thead>
              <tr>
                <th>{t("about.component")}</th>
                <th>{t("about.componentVersion")}</th>
                <th>{t("about.componentLicence")}</th>
              </tr>
            </thead>
            <tbody>
              {info.components.map((c) => (
                <tr key={c.name}>
                  <td>
                    <a href={c.url} target="_blank" rel="noreferrer">
                      {c.name}
                    </a>
                  </td>
                  <td className="mono">{c.version ?? t("about.notInstalled")}</td>
                  <td>
                    {c.license}
                    {c.licensePath ? (
                      <span className="mono muted small"> {c.licensePath}</span>
                    ) : (
                      <span className="muted small"> {t("about.licenceNotBundled")}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted small">
            {t("about.processBoundary")}
          </p>
          {info.noticePath && (
            <p className="muted small">
              {t("about.notice")} <span className="mono">{info.noticePath}</span>
            </p>
          )}

          <h3>{t("about.trademarkTitle")}</h3>
          {/* FR-L4. */}
          <p className="small">{t("about.trademarks")}</p>
        </>
      )}
    </section>
  );
}

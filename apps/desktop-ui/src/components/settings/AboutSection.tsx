import type { AboutProjection } from "../../lib/hostClient";
import { SettingsRow } from "./SettingsRow";

export type AboutStatus = "loading" | "unavailable" | "ready";

/**
 * Version and installation identity, for a support request.
 *
 * This is the one section that legitimately takes a whole projection: every
 * field it renders comes from `AboutProjection`, and reducing it to five
 * scalars would only restate the projection's own shape one layer up. The panes
 * that need a single fact out of it — Skills, Updates — take that fact instead,
 * so About's shape is not their problem.
 *
 * `status` is separate from `about` being null because "loading" and
 * "unavailable" are different sentences and a null projection cannot tell them
 * apart.
 */
export interface AboutSectionProps {
  readonly about: AboutProjection | null;
  readonly status: AboutStatus;
}

export function AboutSection({ about, status }: AboutSectionProps) {
  return (
    <section aria-labelledby="settings-about-title">
      <header className="settings-pane__header">
        <h3 id="settings-about-title">About</h3>
        <p>Version and installation identity for support requests.</p>
      </header>
      {status === "ready" && about !== null ? (
        <div className="settings-list">
          {/*
            `appVersion` is the crate version, so every build this
            repository has ever produced answers "0.1.0" — a tester
            reporting a fault on "0.1.0" identifies nothing. The detail
            now says so rather than implying the value is a build
            identity, and the row below carries one for the half of the
            app that can supply it. A commit-pinned *host* identity needs
            a field on the About projection, which `assertExactKeys`
            requires to land on both sides at once.
          */}
          <SettingsRow
            detail="Product version. It does not identify which build this is."
            label="Version"
            value={about.appVersion}
          />
          <SettingsRow
            detail="Identifies this renderer bundle for support."
            label="UI build"
            value={__FORGE_UI_BUILD__}
          />
          <div className="settings-row settings-row--stacked">
            <span>
              <strong>Installation id</strong>
              <small>Identifies this local installation for support; shares no file content.</small>
            </span>
            <code className="settings-install-id">{about.installationId}</code>
          </div>
          <SettingsRow
            detail="Bundled BMAD foundation package."
            label="Foundation"
            value={`${about.foundationPackageName} ${about.foundationPackageVersion}`}
          />
          <SettingsRow
            detail="Host boot mode for this session."
            label="Boot mode"
            value={about.bootMode === "ready" ? "Ready" : "Read-only recovery"}
          />
        </div>
      ) : (
        <p className="settings-pane__copy" role="status">
          {status === "loading"
            ? "Loading version information…"
            : "Version information is unavailable in this mode."}
        </p>
      )}
    </section>
  );
}

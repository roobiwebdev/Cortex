import * as React from "react";
import { Check, Compass, Download, Monitor, Moon, RefreshCw, Sun } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../../lib/engine";
import { formatAgo } from "../../lib/format";
import type { ThemePref } from "../../lib/types";
import { APP_VERSION, checkForUpdates, installUpdate, useUpdates, type UpdateState } from "../../lib/updates";
import { ACCENT_PRESETS } from "../../lib/use-studio-theme";
import type { SurfaceId } from "../../prototype/surfaces";
import { SettingsGroup, SettingsPage, SettingsRow } from "./parts";

export { APP_VERSION };

/** General: appearance, app presence (login, Dock, HUD), help and updates. Matrix UI-04, UPD-02/03. */
export function GeneralSection({ navigate }: { navigate: (id: SurfaceId) => void }) {
  const s = useEngine().settings;
  return (
    <SettingsPage title="General" subtitle="How Cortex looks and where it lives on your Mac.">
      <SettingsGroup title="Appearance">
        <SettingsRow label="Appearance" hint="System follows your Mac's light or dark setting.">
          <Segmented<ThemePref>
            aria-label="Appearance"
            value={s.theme}
            onValueChange={(theme) => engine.update({ theme })}
            options={[
              {
                value: "system",
                label: (
                  <>
                    <Monitor /> System
                  </>
                ),
              },
              {
                value: "light",
                label: (
                  <>
                    <Sun /> Light
                  </>
                ),
              },
              {
                value: "dark",
                label: (
                  <>
                    <Moon /> Dark
                  </>
                ),
              },
            ]}
          />
        </SettingsRow>
        <SettingsRow label="Accent colour" hint="Used for selections and buttons. Red is kept for recording.">
          <div role="radiogroup" aria-label="Accent colour" className="flex gap-2">
            {ACCENT_PRESETS.map((a) => {
              const on = s.accentHue === a.hue;
              return (
                <button
                  key={a.hue}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  aria-label={a.label}
                  title={a.label}
                  onClick={() => engine.update({ accentHue: a.hue })}
                  className={cn(
                    "grid size-[22px] place-items-center rounded-full ring-offset-2 ring-offset-card transition-shadow outline-none hover:ring-2 hover:ring-border focus-visible:ring-2 focus-visible:ring-ring",
                    on && "ring-2 ring-ring hover:ring-ring",
                  )}
                  // Swatch colour is the preset's own hue: it previews the accent before it is applied.
                  style={{ background: `hsl(${a.hue} 58% 45%)` }}
                >
                  {on && <Check className="size-3 text-background" strokeWidth={3} />}
                </button>
              );
            })}
          </div>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="App">
        <SettingsRow label="Open at login" hint="Cortex starts in the menu bar when you log in." htmlFor="set-login">
          <Switch
            id="set-login"
            checked={s.openAtLogin}
            onCheckedChange={(openAtLogin) => engine.update({ openAtLogin })}
          />
        </SettingsRow>
        <SettingsRow
          label="Show in Dock"
          hint={
            s.showInDock
              ? "Cortex has a Dock icon and app menu."
              : "Menu bar only. Open windows from the menu bar icon."
          }
          htmlFor="set-dock"
        >
          <Switch
            id="set-dock"
            checked={s.showInDock}
            onCheckedChange={(showInDock) => engine.update({ showInDock })}
          />
        </SettingsRow>
        <SettingsRow
          label="Show HUD while recording"
          hint="A small floating timer with stop and pause. It never appears in the recording."
          htmlFor="set-hud"
        >
          <Switch id="set-hud" checked={s.showHud} onCheckedChange={(showHud) => engine.update({ showHud })} />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Help">
        <SettingsRow
          label="Interface sounds"
          hint="A soft click as you step through the tour. Never while recording."
          htmlFor="set-sounds"
        >
          <Switch id="set-sounds" checked={s.sounds} onCheckedChange={(sounds) => engine.update({ sounds })} />
        </SettingsRow>
        <SettingsRow label="Tour" hint="A two-minute walk through recording, the camera, shortcuts and trimming.">
          <Button variant="soft" size="sm" onClick={() => navigate("onboarding")}>
            <Compass />
            Show the tour
          </Button>
        </SettingsRow>
        <SettingsRow
          label="Tips"
          hint={
            s.seenHints.length > 0
              ? "Short tips appear the first time you use area picking, Quick trim and Go live."
              : "Every tip will show the next time you use its feature."
          }
        >
          <Button
            variant="soft"
            size="sm"
            disabled={s.seenHints.length === 0}
            onClick={() => engine.update({ seenHints: [] })}
          >
            Show tips again
          </Button>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Updates"
        footer="Updates are signed and checked before they install. Cortex never updates during a recording."
      >
        <SettingsRow
          label="Check for updates automatically"
          hint="Looks once a day and asks before installing."
          htmlFor="set-upd"
        >
          <Switch id="set-upd" checked={s.autoUpdate} onCheckedChange={(autoUpdate) => engine.update({ autoUpdate })} />
        </SettingsRow>
        <UpdateRow />
      </SettingsGroup>
    </SettingsPage>
  );
}

/** What the Updates row says under the version. */
function updateHint(u: UpdateState, busyRecording: boolean): React.ReactNode {
  const next = u.available?.version;
  if (u.installing) return `Installing Cortex ${next ?? ""}. Cortex reopens in a moment.`;
  if (u.download) {
    const pct = u.download.total ? Math.round((u.download.downloaded / u.download.total) * 100) : null;
    return pct === null ? `Downloading Cortex ${next ?? ""}…` : `Downloading Cortex ${next ?? ""}… ${pct}%`;
  }
  if (u.checking) return "Checking…";
  if (u.error) return u.error;
  if (next) {
    const when = busyRecording ? "Stop recording to install it." : u.ready ? "Downloaded and ready to install." : null;
    return (
      <>
        Cortex {next} is available.{when && ` ${when}`}
        {u.available?.notes && <span className="mt-1 line-clamp-3 block whitespace-pre-line">{u.available.notes}</span>}
      </>
    );
  }
  if (u.lastChecked) return `You're up to date. Checked ${formatAgo(u.lastChecked).toLowerCase()}.`;
  return "Not checked yet.";
}

function UpdateRow() {
  const u = useUpdates();
  // Installing relaunches Cortex, so the shell refuses it while anything is being captured.
  const busyRecording = useEngine((s) => s.status !== "idle" || s.live);
  const busy = u.checking || u.download !== null || u.installing;
  const canInstall = u.available !== null;
  return (
    <SettingsRow
      label={
        <span>
          Version <span className="tabular">{u.currentVersion}</span>
        </span>
      }
      hint={updateHint(u, busyRecording)}
    >
      {canInstall ? (
        <Button variant="soft" size="sm" disabled={busy || busyRecording} onClick={installUpdate}>
          <Download />
          Install and reopen
        </Button>
      ) : (
        <Button variant="soft" size="sm" disabled={busy} onClick={checkForUpdates}>
          <RefreshCw className={cn(u.checking && "animate-spin")} />
          Check now
        </Button>
      )}
    </SettingsRow>
  );
}

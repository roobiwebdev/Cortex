import * as React from "react";
import { Check, Monitor, Moon, RefreshCw, Sun } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Segmented } from "@starter/ui/components/segmented";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../../lib/mock-engine";
import type { ThemePref } from "../../lib/types";
import { ACCENT_PRESETS } from "../../lib/use-studio-theme";
import { SettingsGroup, SettingsPage, SettingsRow } from "./parts";

export const APP_VERSION = "0.1.0";

/** General: appearance, app presence (login, Dock, HUD) and updates. Matrix UI-04, UPD-02/03. */
export function GeneralSection() {
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

function UpdateRow() {
  // prototype-local: a fake check that always finds the current version.
  const [state, setState] = React.useState<"idle" | "checking" | "latest">("idle");
  React.useEffect(() => {
    if (state !== "checking") return;
    const t = setTimeout(() => setState("latest"), 1200);
    return () => clearTimeout(t);
  }, [state]);

  return (
    <SettingsRow
      label={
        <span>
          Version <span className="tabular">{APP_VERSION}</span>
        </span>
      }
      hint={
        state === "latest" ? "You're up to date." : state === "checking" ? "Checking…" : "Last checked today at 09:12."
      }
    >
      <Button variant="soft" size="sm" disabled={state === "checking"} onClick={() => setState("checking")}>
        <RefreshCw className={cn(state === "checking" && "animate-spin")} />
        Check now
      </Button>
    </SettingsRow>
  );
}

import * as React from "react";
import { Check, ClipboardCopy, Download } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@starter/ui/components/alert-dialog";
import { Button } from "@starter/ui/components/button";
import { Switch } from "@starter/ui/components/switch";

import { engine, useEngine } from "../../lib/engine";
import type { EngineState, RecorderSettings } from "../../lib/types";
import { SettingsGroup, SettingsPage, SettingsRow } from "./parts";
import { APP_VERSION } from "./general";

const ENV_VARS: ReadonlyArray<{ name: string; does: string }> = [
  { name: "REC_DEBUG", does: "Log audio offsets, frame timing and encoder retries" },
  { name: "REC_SETTINGS", does: "Read settings from another file" },
  { name: "REC_OUTPUT_DIR", does: "Save recordings to another folder" },
  { name: "REC_STREAM_KEY", does: "Use this stream key instead of Keychain" },
  { name: "REC_INSECURE_TLS", does: "Accept self-signed RTMPS/WHIP servers" },
];

// prototype-local: the engine has no defaults getter yet, so remember the first settings this window saw.
let firstSeen: RecorderSettings | null = null;

function deviceReport(e: EngineState) {
  const lines = [
    `Cortex ${APP_VERSION} · ${navigator.userAgent}`,
    ...e.displays.map((d) => `Display ${d.id}: ${d.name} ${d.width}×${d.height}`),
    ...e.cameras.map((c) => `Camera: ${c.name}${c.connected ? "" : " (not connected)"}`),
    ...e.mics.map((m) => `Mic: ${m.name}${m.connected ? "" : " (not connected)"}`),
    `Permissions: ${Object.entries(e.permissions)
      .map(([k, v]) => `${k}=${v}`)
      .join(", ")}`,
    `Free disk: ${e.freeDiskBytes} bytes`,
  ];
  return lines.join("\n");
}

/** Advanced: preview power, diagnostics, resets, env overrides, Rec import. Matrix SET-K29, PERF-*, ENV-01..06, SET-03 (import). */
export function AdvancedSection() {
  const e = useEngine();
  firstSeen ??= e.settings;
  const [copied, setCopied] = React.useState(false);
  const [permsReset, setPermsReset] = React.useState(false);
  const [imported, setImported] = React.useState(false);

  async function copyReport() {
    try {
      await navigator.clipboard.writeText(deviceReport(e));
    } catch {
      // Clipboard can be blocked in a preview; the confirmation still reflects intent.
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  }

  return (
    <SettingsPage title="Advanced" subtitle="Performance, troubleshooting and resets. Most people never need these.">
      <SettingsGroup title="Performance">
        <SettingsRow
          label="Low-power preview"
          htmlFor="set-lowpower"
          hint="Updates the live preview less often to save battery. Recordings are not affected."
        >
          <Switch
            id="set-lowpower"
            checked={e.settings.previewLowPower}
            onCheckedChange={(previewLowPower) => engine.update({ previewLowPower })}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Migrate">
        <SettingsRow
          label="Import settings from Rec"
          badge="new"
          hint={
            imported
              ? "Imported. Stream keys stay in Keychain; re-enter them if Cortex asks."
              : "Found ~/Library/Application Support/rec-rs/settings.json"
          }
        >
          <Button variant="soft" size="sm" disabled={imported} onClick={() => setImported(true)}>
            {imported ? <Check /> : <Download />}
            {imported ? "Imported" : "Import…"}
          </Button>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Troubleshooting">
        <SettingsRow
          label="Device report"
          hint="Copies displays, cameras, mics and permission states. No recordings or keys."
        >
          <Button variant="soft" size="sm" onClick={copyReport}>
            {copied ? <Check /> : <ClipboardCopy />}
            {copied ? "Copied" : "Copy"}
          </Button>
        </SettingsRow>
        <SettingsRow
          label="Reset permissions"
          hint={
            permsReset
              ? "Done. Cortex will ask again next time it records."
              : "Asks again for screen, microphone and camera access."
          }
        >
          <Button
            variant="soft"
            size="sm"
            onClick={() => {
              engine.setPermission("screen", "not-determined");
              engine.setPermission("microphone", "not-determined");
              engine.setPermission("camera", "not-determined");
              setPermsReset(true);
            }}
          >
            Reset…
          </Button>
        </SettingsRow>
        <SettingsRow label="Reset all settings" hint="Back to defaults. Recordings and stream keys are kept.">
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="soft" size="sm">
                Reset…
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent className="rounded-[18px] bg-card">
              <AlertDialogHeader>
                <AlertDialogTitle>Reset all settings?</AlertDialogTitle>
                <AlertDialogDescription>
                  Every option goes back to its default. Your recordings and Keychain stream keys are not touched.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  variant="tactile"
                  onClick={() => {
                    if (firstSeen) engine.update(firstSeen);
                  }}
                >
                  Reset
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Environment overrides"
        footer="For debugging. Set these before launching Cortex from Terminal; they win over the settings above."
      >
        <dl className="divide-y">
          {ENV_VARS.map((v) => (
            <div key={v.name} className="flex items-baseline gap-4 px-4 py-2">
              <dt className="w-36 shrink-0 font-mono text-[11.5px] text-accent-text">{v.name}</dt>
              <dd className="text-xs text-muted-foreground">{v.does}</dd>
            </div>
          ))}
        </dl>
      </SettingsGroup>
    </SettingsPage>
  );
}

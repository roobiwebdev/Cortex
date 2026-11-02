import * as React from "react";
import { FolderOpen } from "lucide-react";

import { Button } from "@starter/ui/components/button";

import { engine, useEngine } from "../../lib/engine";
import { ChoiceSelect, SettingsRow } from "./parts";

/** Folders offered by the popup; "other" stands in for the native folder picker. */
const FOLDERS = ["~/Movies/Cortex", "~/Movies", "~/Desktop", "~/Documents/Recordings"] as const;

/**
 * SOURCE OF TRUTH KEYWORDS: OutputFolderRow, output folder, outputDir, choose folder, save location
 * WHAT:  The "Save to" row: current folder path plus a Choose… button (native folder picker later).
 * WHY:   Shown in both Recording and Storage; one component keeps them identical.
 * WHERE: settings Recording and Storage sections.
 */
export function OutputFolderRow() {
  const dir = useEngine().settings.outputDir;
  function choose() {
    // prototype-local: cycle through common folders instead of opening NSOpenPanel.
    const i = FOLDERS.findIndex((f) => f === dir);
    engine.update({ outputDir: FOLDERS[(i + 1) % FOLDERS.length] ?? FOLDERS[0] });
  }
  return (
    <SettingsRow label="Save recordings to" hint="New recordings and saved replays go here.">
      <span className="flex max-w-56 items-center gap-1.5 truncate rounded-[8px] bg-muted px-2 py-1 font-mono text-xs text-muted-foreground">
        <FolderOpen className="size-3.5 shrink-0" />
        <span className="truncate">{dir}</span>
      </span>
      <Button variant="soft" size="sm" onClick={choose}>
        Choose…
      </Button>
    </SettingsRow>
  );
}

type NamingPattern = "dated" | "plain" | "iso";
const NAMING: ReadonlyArray<{ value: NamingPattern; label: string }> = [
  { value: "dated", label: "Recording {date} at {time}" },
  { value: "plain", label: "Cortex {date} {time}" },
  { value: "iso", label: "{yyyy-mm-dd}_{hh-mm-ss}" },
];

// prototype-local store so Recording and Storage show the same pattern. Needs a `namingPattern` setting.
let naming: NamingPattern = "dated";
const namingListeners = new Set<() => void>();
function useNaming(): [NamingPattern, (p: NamingPattern) => void] {
  const value = React.useSyncExternalStore(
    (l) => {
      namingListeners.add(l);
      return () => namingListeners.delete(l);
    },
    () => naming,
  );
  return [
    value,
    (p) => {
      naming = p;
      for (const l of namingListeners) l();
    },
  ];
}

const pad = (n: number) => String(n).padStart(2, "0");

function previewName(p: NamingPattern, ext: string, d = new Date()) {
  const date = d.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
  const time = `${pad(d.getHours())}.${pad(d.getMinutes())}.${pad(d.getSeconds())}`;
  if (p === "dated") return `Recording ${date} at ${time}.${ext}`;
  if (p === "plain") return `Cortex ${date} ${time}.${ext}`;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${time.replaceAll(".", "-")}.${ext}`;
}

/**
 * SOURCE OF TRUTH KEYWORDS: NamingRow, file naming, filename pattern, preview name
 * WHAT:  File-name pattern picker with a live preview of the next file name (uses the current container).
 * WHY:   People search Finder by name; seeing the exact result beats reading token syntax.
 * WHERE: settings Recording and Storage sections.
 */
export function NamingRow() {
  const container = useEngine().settings.container;
  const [pattern, setPattern] = useNaming();
  return (
    <SettingsRow
      label="File name"
      badge="prototype"
      hint={
        <>
          Next file:
          <span className="block font-mono text-[11px] text-foreground/80">{previewName(pattern, container)}</span>
        </>
      }
    >
      <ChoiceSelect<NamingPattern>
        aria-label="File name pattern"
        value={pattern}
        onChange={setPattern}
        options={NAMING}
      />
    </SettingsRow>
  );
}

import * as React from "react";
import { FolderSearch } from "lucide-react";

import { Button } from "@starter/ui/components/button";

import { formatBytes } from "../../lib/format";
import { useEngine } from "../../lib/mock-engine";
import { NamingRow, OutputFolderRow } from "./files";
import { ChoiceSelect, SettingsGroup, SettingsPage, SettingsRow } from "./parts";
import { autoBitrate } from "./recording";

/** Rough size of a 1 TB-class internal disk, for the usage bar. */
const DISK_BYTES = 494_000_000_000;
type Keep = "forever" | "90" | "30";

/** Storage: where files go, free space, how long recordings are kept. Matrix SET-K01, SET-07, FLG-01. */
export function StorageSection() {
  const e = useEngine();
  const s = e.settings;
  // prototype-local: needs a `keepRecordings` setting and a cleanup job (moves to Trash).
  const [keep, setKeep] = React.useState<Keep>("forever");
  const [revealed, setRevealed] = React.useState(false);

  const used = e.recordings.reduce((n, r) => n + r.bytes, 0);
  const mbps = s.bitrateMbps ?? autoBitrate(s.resolution, s.fps);
  const hoursLeft = e.freeDiskBytes / ((mbps * 1_000_000) / 8) / 3600;
  const usedPct = Math.min(100, ((DISK_BYTES - e.freeDiskBytes) / DISK_BYTES) * 100);
  const cortexPct = Math.max(0.6, (used / DISK_BYTES) * 100);

  return (
    <SettingsPage title="Storage" subtitle="Where recordings are saved and how much room is left.">
      <SettingsGroup title="Disk">
        <div className="px-4 py-4">
          <div className="flex items-baseline justify-between">
            <span className="text-[13px] font-medium">Macintosh HD</span>
            <span className="tabular text-xs text-muted-foreground">
              <span className="font-display text-xl font-semibold text-foreground">{formatBytes(e.freeDiskBytes)}</span>{" "}
              free of {formatBytes(DISK_BYTES)}
            </span>
          </div>
          <div
            className="mt-3 flex h-2 overflow-hidden rounded-full bg-muted"
            role="img"
            aria-label={`${Math.round(usedPct)}% of the disk used`}
          >
            <div className="h-full bg-muted-foreground/45" style={{ width: `${usedPct - cortexPct}%` }} />
            <div className="h-full bg-primary" style={{ width: `${cortexPct}%` }} />
          </div>
          <div className="mt-2.5 flex items-center gap-4 text-xs text-muted-foreground">
            <span className="inline-flex items-center gap-1.5">
              <span className="size-2 rounded-full bg-primary" />
              <span>
                Cortex · <span className="tabular">{formatBytes(used)}</span> in {e.recordings.length} recordings
              </span>
            </span>
            <span className="inline-flex items-center gap-1.5">
              <span className="size-2 rounded-full bg-muted-foreground/45" /> Other
            </span>
          </div>
        </div>
        <SettingsRow
          label="Recording time left"
          hint={`At ${mbps} Mb/s (current Recording settings). Cortex warns you below 5 GB.`}
        >
          <span className="tabular text-[13px] font-medium">
            ≈ {hoursLeft >= 10 ? Math.round(hoursLeft) : hoursLeft.toFixed(1)} h
          </span>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Files">
        <OutputFolderRow />
        <NamingRow />
        <SettingsRow
          label="Keep recordings"
          badge="prototype"
          hint={
            keep === "forever"
              ? "Cortex never deletes anything."
              : `Older recordings move to the Trash after ${keep} days.`
          }
        >
          <ChoiceSelect<Keep>
            aria-label="Keep recordings"
            value={keep}
            onChange={setKeep}
            options={[
              { value: "forever", label: "Forever" },
              { value: "90", label: "90 days" },
              { value: "30", label: "30 days" },
            ]}
          />
        </SettingsRow>
        <SettingsRow label="Recordings folder" hint={revealed ? "Opened in Finder." : `Open ${s.outputDir} in Finder.`}>
          <Button variant="soft" size="sm" onClick={() => setRevealed(true)}>
            <FolderSearch /> Reveal in Finder
          </Button>
        </SettingsRow>
      </SettingsGroup>
    </SettingsPage>
  );
}

import * as React from "react";
import { ExternalLink } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@starter/ui/components/dialog";

import { BrandMark } from "../../components/brand-mark";
import { engine } from "../../lib/engine";
import { usePlatformWords } from "../../lib/platform";
import { APP_VERSION } from "./general";
import { SettingsGroup, SettingsRow } from "./parts";

const NOTICES: ReadonlyArray<{ name: string; license: string; use: string }> = [
  { name: "Rec", license: "MIT or Apache-2.0", use: "Recording engine design" },
  { name: "Tauri", license: "MIT or Apache-2.0", use: "App shell and software updates" },
  { name: "Inter", license: "SIL OFL 1.1", use: "Interface font" },
  { name: "Bricolage Grotesque", license: "SIL OFL 1.1", use: "Display font" },
  { name: "JetBrains Mono", license: "SIL OFL 1.1", use: "Monospaced font" },
  { name: "Lucide", license: "ISC", use: "Icons" },
  { name: "Radix UI", license: "MIT", use: "Accessible controls" },
  { name: "React", license: "MIT", use: "Interface" },
];

/** About: name, version, credits and licences. */
export function AboutSection() {
  const words = usePlatformWords();
  const [open, setOpen] = React.useState(false);
  return (
    <div className="mx-auto w-full max-w-[580px] px-8 pb-10">
      <div className="flex flex-col items-center pt-4 pb-8 text-center">
        <div className="grid size-[76px] place-items-center rounded-[18px] border bg-card shadow-ledge">
          <BrandMark className="size-12" />
        </div>
        <h1 className="mt-4 font-display text-[30px] leading-none font-bold tracking-[-0.015em]">Cortex</h1>
        <p className="tabular mt-2 text-[13px] text-muted-foreground">
          Version {APP_VERSION} · {words.requirement}
        </p>
        <p className="mt-3 text-[13px] text-muted-foreground">
          Built by{" "}
          <button
            type="button"
            onClick={() => engine.openLink("author")}
            className="font-medium text-foreground underline decoration-muted-foreground/50 underline-offset-2 transition-colors hover:text-primary hover:decoration-primary focus-visible:rounded-sm focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
          >
            Roobi
          </button>
          .
        </p>
      </div>

      <div className="space-y-6">
        <SettingsGroup title="Credits">
          <SettingsRow label="Built by Roobi" hint="Designer and developer of Cortex. roobi.dev">
            <Button variant="soft" size="sm" onClick={() => engine.openLink("author")}>
              Visit <ExternalLink />
            </Button>
          </SettingsRow>
          <SettingsRow label="Cortex on GitHub" hint="Downloads, release notes and updates.">
            <Button variant="ghost" size="sm" onClick={() => engine.openLink("releases")}>
              Open <ExternalLink />
            </Button>
          </SettingsRow>
          <SettingsRow label="Third-party notices" hint="Fonts, icons and libraries that ship inside Cortex.">
            <Button variant="soft" size="sm" onClick={() => setOpen(true)}>
              View
            </Button>
          </SettingsRow>
        </SettingsGroup>
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="rounded-[18px] bg-card sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="font-display text-lg">Third-party notices</DialogTitle>
            <DialogDescription>
              Cortex includes work from these projects. Full licence texts ship in the app bundle.
            </DialogDescription>
          </DialogHeader>
          <ul className="divide-y rounded-[14px] border">
            {NOTICES.map((n) => (
              <li key={n.name} className="flex items-baseline justify-between gap-4 px-3 py-2">
                <span>
                  <span className="text-[13px] font-medium">{n.name}</span>
                  <span className="ml-2 text-xs text-muted-foreground">{n.use}</span>
                </span>
                <span className="shrink-0 font-mono text-[11px] text-muted-foreground">{n.license}</span>
              </li>
            ))}
          </ul>
        </DialogContent>
      </Dialog>
    </div>
  );
}

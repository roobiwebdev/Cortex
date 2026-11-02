import * as React from "react";
import {
  AudioLines,
  Camera,
  HardDrive,
  Info,
  Keyboard,
  type LucideIcon,
  Radio,
  Search,
  Settings2,
  SlidersHorizontal,
  Video,
} from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import { TITLEBAR_H } from "../prototype/window-frame";
import type { SurfaceProps } from "../prototype/surfaces";
import { AboutSection } from "./settings/about";
import { AdvancedSection } from "./settings/advanced";
import { AudioSection } from "./settings/audio";
import { CameraSection } from "./settings/camera";
import { GeneralSection } from "./settings/general";
import { LiveSection } from "./settings/live";
import { RecordingSection } from "./settings/recording";
import { ShortcutsSection } from "./settings/shortcuts";
import { StorageSection } from "./settings/storage";

type SectionId = "general" | "recording" | "audio" | "camera" | "live" | "shortcuts" | "storage" | "advanced" | "about";

/** Sidebar entries. Tile colours use theme tokens only; red is never used (it means "recording"). */
const SECTIONS: ReadonlyArray<{
  id: SectionId;
  label: string;
  icon: LucideIcon;
  tile: string;
  /** Extra words the sidebar search matches. */
  keywords: string;
  group: 0 | 1 | 2;
}> = [
  {
    id: "general",
    label: "General",
    icon: Settings2,
    tile: "bg-muted-foreground text-background",
    keywords: "appearance theme dark light accent colour login dock menu bar hud update version",
    group: 0,
  },
  {
    id: "recording",
    label: "Recording",
    icon: Video,
    tile: "bg-primary text-primary-foreground",
    keywords:
      "video resolution fps frame rate codec hevc h264 prores container mov mp4 bitrate cursor clicks countdown folder name",
    group: 1,
  },
  {
    id: "audio",
    label: "Audio",
    icon: AudioLines,
    tile: "bg-warn text-background",
    keywords: "system audio microphone mic tracks mix stems gain db offset sync latency bluetooth backend",
    group: 1,
  },
  {
    id: "camera",
    label: "Camera",
    icon: Camera,
    tile: "bg-ok text-background",
    keywords: "webcam overlay pip shape size border shadow mirror track 1080p",
    group: 1,
  },
  {
    id: "live",
    label: "Live & Replay",
    icon: Radio,
    tile: "bg-primary text-primary-foreground",
    keywords: "stream youtube twitch kick rtmp whip key keychain replay buffer bitrate keyframe cbr",
    group: 1,
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    icon: Keyboard,
    tile: "bg-muted-foreground text-background",
    keywords: "hotkey keyboard keys",
    group: 2,
  },
  {
    id: "storage",
    label: "Storage",
    icon: HardDrive,
    tile: "bg-ok text-background",
    keywords: "disk space folder finder keep delete",
    group: 2,
  },
  {
    id: "advanced",
    label: "Advanced",
    icon: SlidersHorizontal,
    tile: "bg-muted-foreground text-background",
    keywords: "low power preview diagnostics report reset permissions environment debug import rec",
    group: 2,
  },
  {
    id: "about",
    label: "About",
    icon: Info,
    tile: "bg-muted-foreground text-background",
    keywords: "version licence license credits notices rec",
    group: 2,
  },
];

/**
 * SOURCE OF TRUTH KEYWORDS: SettingsWindow, settings, preferences, sidebar, sections, settings window
 * WHAT:  The Settings window: a full-height sidebar of sections (traffic lights sit over it, like
 *        System Settings) and a scrolling right pane that shows one section.
 * WHY:   Every Rec option needs a findable home; sections + search keep ~60 settings scannable.
 *        All values read and write the engine, so other surfaces update live.
 * WHERE: prototype surface "settings"; later the Tauri settings window.
 */
export function SettingsWindow({ cameraStream }: SurfaceProps) {
  const [active, setActive] = React.useState<SectionId>("general");
  const [query, setQuery] = React.useState("");
  const [scrolled, setScrolled] = React.useState(false);
  const paneRef = React.useRef<HTMLDivElement>(null);

  const q = query.trim().toLowerCase();
  const visible = q ? SECTIONS.filter((s) => `${s.label} ${s.keywords}`.toLowerCase().includes(q)) : SECTIONS;
  const current = SECTIONS.find((s) => s.id === active) ?? SECTIONS[0];

  function open(id: SectionId) {
    setActive(id);
    paneRef.current?.scrollTo({ top: 0 });
    setScrolled(false);
  }

  return (
    <div className="flex size-full bg-background text-[13px] text-foreground">
      <aside className="flex w-[210px] shrink-0 flex-col border-r bg-sidebar" style={{ paddingTop: TITLEBAR_H }}>
        <div className="px-3 pb-3">
          <label className="flex h-7 items-center gap-1.5 rounded-[8px] bg-background/70 px-2 text-muted-foreground ring-1 ring-border transition-shadow focus-within:ring-2 focus-within:ring-ring/50">
            <Search className="size-3.5 shrink-0" />
            <input
              type="text"
              aria-label="Search settings"
              placeholder="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                const first = visible[0];
                if (e.key === "Enter" && first) open(first.id);
                if (e.key === "Escape") setQuery("");
              }}
              className="w-full min-w-0 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </label>
        </div>
        <nav aria-label="Settings sections" className="flex-1 overflow-y-auto px-2 pb-3">
          {visible.length === 0 && <p className="px-2 py-1 text-xs text-muted-foreground">No matches.</p>}
          {visible.map((s, i) => {
            const prev = visible[i - 1];
            const on = s.id === active;
            const Icon = s.icon;
            return (
              <React.Fragment key={s.id}>
                {prev && prev.group !== s.group && <div className="mx-2 my-1.5 h-px bg-border" />}
                <button
                  type="button"
                  aria-current={on ? "page" : undefined}
                  onClick={() => open(s.id)}
                  className={cn(
                    "flex h-8 w-full items-center gap-2.5 rounded-[8px] px-1.5 text-left text-[13px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50",
                    on ? "bg-card font-medium shadow-ledge" : "hover:bg-card/60",
                  )}
                >
                  <span className={cn("grid size-[22px] shrink-0 place-items-center rounded-[6px]", s.tile)}>
                    <Icon className="size-3.5" strokeWidth={2.2} />
                  </span>
                  {s.label}
                </button>
              </React.Fragment>
            );
          })}
        </nav>
      </aside>

      <div
        ref={paneRef}
        onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 40)}
        className="relative min-w-0 flex-1 overflow-y-auto"
      >
        {/* Inset title bar zone: stays put and shows the section name once the big title scrolls away. */}
        <div
          className={cn(
            "sticky top-0 z-10 flex items-center justify-center border-b border-transparent bg-background/85 backdrop-blur-md transition-colors",
            scrolled && "border-border",
          )}
          style={{ height: TITLEBAR_H }}
        >
          <span
            className={cn(
              "text-[13px] font-semibold transition-opacity duration-150",
              scrolled ? "opacity-100" : "opacity-0",
            )}
            aria-hidden
          >
            {current?.label}
          </span>
        </div>
        <div className="pt-1">
          {active === "general" && <GeneralSection />}
          {active === "recording" && <RecordingSection />}
          {active === "audio" && <AudioSection />}
          {active === "camera" && <CameraSection cameraStream={cameraStream} />}
          {active === "live" && <LiveSection />}
          {active === "shortcuts" && <ShortcutsSection />}
          {active === "storage" && <StorageSection />}
          {active === "advanced" && <AdvancedSection />}
          {active === "about" && <AboutSection />}
        </div>
      </div>
    </div>
  );
}

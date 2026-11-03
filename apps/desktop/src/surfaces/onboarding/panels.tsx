import type * as React from "react";
import {
  Check,
  Download,
  ExternalLink,
  HardDrive,
  Info,
  Layers,
  MenuSquare,
  Mic,
  Monitor,
  RotateCcw,
  ShieldCheck,
  Video,
} from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { cn } from "@starter/ui/lib/utils";

import type { Permission } from "../../lib/types";
import { PermissionChip, type ChipState } from "./permission-chip";
import { NEED_LABEL, PERMISSION_STEPS } from "./steps";

const PERMISSION_ICON: Record<Permission, React.ReactNode> = {
  screen: <Monitor className="size-4" />,
  microphone: <Mic className="size-4" />,
  camera: <Video className="size-4" />,
};

/** Step kicker, e.g. "STEP 2 OF 5 · REQUIRED". */
function Kicker({ children }: { children: React.ReactNode }) {
  return <p className="text-[10.5px] font-semibold tracking-[0.08em] text-accent-text uppercase">{children}</p>;
}

function Headline({ children, id }: { children: React.ReactNode; id: string }) {
  return (
    <h1 id={id} className="mt-2 font-display text-[30px] leading-[1.08] font-bold tracking-[-0.01em] text-balance">
      {children}
    </h1>
  );
}

function Lede({ children }: { children: React.ReactNode }) {
  return <p className="mt-3 text-[13.5px] leading-relaxed text-pretty text-muted-foreground">{children}</p>;
}

/**
 * SOURCE OF TRUTH KEYWORDS: WelcomePanel, onboarding welcome, value bullets, move to Applications
 * WHAT:  Step 1 copy: what Cortex does, three value points and the install-location note.
 * WHY:   Sets expectations before asking for any permission.
 * WHERE: OnboardingWindow, first step.
 */
export function WelcomePanel({ kicker, titleId }: { kicker: string; titleId: string }) {
  const points: Array<{ icon: React.ReactNode; text: string }> = [
    {
      icon: <Monitor className="size-3.5" />,
      text: "Record a display, a window, an app or any area, up to 4K at 60 fps.",
    },
    { icon: <Layers className="size-3.5" />, text: "Camera, microphone and system audio each keep their own track." },
    {
      icon: <HardDrive className="size-3.5" />,
      text: "Files save straight to your Mac. No account, nothing uploaded.",
    },
  ];
  return (
    <div>
      <Kicker>{kicker}</Kicker>
      <Headline id={titleId}>Record your screen, camera and voice</Headline>
      <Lede>Cortex is a quiet recorder that lives in your menu bar. Setup takes about a minute.</Lede>
      <ul className="mt-5 space-y-2.5">
        {points.map((p) => (
          <li key={p.text} className="flex items-start gap-3 text-[13px]">
            <span className="mt-px grid size-6 shrink-0 place-items-center rounded-[6px] bg-accent-tint text-accent-text">
              {p.icon}
            </span>
            <span className="pt-0.5">{p.text}</span>
          </li>
        ))}
      </ul>
      <Note icon={<Download className="size-3.5" />} className="mt-6">
        Opened Cortex from Downloads or a disk image? It will offer to move itself to Applications so updates can
        install.
      </Note>
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: PermissionPanel, permission step, request access, System Settings deep link, denied, TCC
 * WHAT:  One permission step: headline, lede, the request button, a live status chip,
 *        a System Settings shortcut and the denied explanation.
 * WHY:   The user grants each permission where it is explained, and sees the result
 *        immediately (Rec's PRM-03/04 flows), instead of hunting through System Settings.
 * WHERE: OnboardingWindow steps 2–4 (screen, microphone, camera).
 */
export function PermissionPanel({
  permission,
  kicker,
  titleId,
  chip,
  onRequest,
  onOpenSettings,
  onRelaunch,
}: {
  permission: Permission;
  kicker: string;
  titleId: string;
  chip: ChipState;
  onRequest: () => void;
  onOpenSettings: () => void;
  /** Quit and reopen Cortex (shown when a Screen Recording grant waits for it). */
  onRelaunch: () => void;
}) {
  const p = PERMISSION_STEPS[permission];
  const granted = chip === "granted";
  const waiting = chip === "waiting";
  const denied = chip === "denied";

  return (
    <div>
      <Kicker>
        {kicker} · {NEED_LABEL[p.need]}
      </Kicker>
      <Headline id={titleId}>{p.title}</Headline>
      <Lede>{p.lede}</Lede>

      {chip === "relaunch" ? (
        <Button variant="tactile" size="lg" className="mt-5 h-10 rounded-[var(--radius)] px-4" onClick={onRelaunch}>
          <RotateCcw />
          Reopen Cortex
        </Button>
      ) : (
        <Button
          variant={granted ? "soft" : "tactile"}
          size="lg"
          className="mt-5 h-10 rounded-[var(--radius)] px-4"
          disabled={granted || waiting}
          onClick={onRequest}
        >
          {granted ? <Check /> : denied ? <RotateCcw /> : <ShieldCheck />}
          {granted ? "Allowed" : denied ? "Ask again" : p.allowLabel}
        </Button>
      )}

      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1">
        <PermissionChip state={chip} />
        {p.need === "required" && chip !== "granted" && chip !== "waiting" && chip !== "relaunch" && (
          <span className="text-[12px] text-muted-foreground">Needed to continue</span>
        )}
        <Button
          variant="link"
          size="sm"
          className="ml-auto h-6 px-0 text-[12.5px] text-muted-foreground hover:text-foreground"
          onClick={onOpenSettings}
          disabled={waiting}
        >
          Open System Settings
          <ExternalLink data-icon="inline-end" />
        </Button>
      </div>

      {denied ? (
        <div className="mt-4 rounded-[10px] bg-warn-tint px-3 py-2.5 text-[12.5px] leading-relaxed text-foreground">
          macOS has {p.name} turned off for Cortex. Click <b className="font-semibold">Ask again</b>, or open{" "}
          <span className="font-medium">System Settings › Privacy &amp; Security › {p.name}</span> and switch Cortex on.
        </div>
      ) : (
        p.notes.length > 0 && (
          <div className="mt-4 space-y-1.5">
            {p.notes.map((n) => (
              <Note key={n} icon={<Info className="size-3.5" />}>
                {n}
              </Note>
            ))}
          </div>
        )
      )}
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: ReadyPanel, onboarding summary, granted, skipped, global shortcut, menu bar
 * WHAT:  The last step: what was granted or skipped, the global shortcut and where Cortex lives.
 * WHY:   Confirms the setup and tells the user how to start, so the first recording is obvious.
 * WHERE: OnboardingWindow, final step.
 */
export function ReadyPanel({
  kicker,
  titleId,
  chips,
}: {
  kicker: string;
  titleId: string;
  chips: Record<Permission, ChipState>;
}) {
  const order: Permission[] = ["screen", "microphone", "camera"];
  return (
    <div>
      <Kicker>{kicker}</Kicker>
      <Headline id={titleId}>You’re ready to record</Headline>
      <ul className="mt-5 divide-y rounded-[14px] border bg-card">
        {order.map((perm) => {
          const p = PERMISSION_STEPS[perm];
          const state = chips[perm];
          const ok = state === "granted";
          return (
            <li key={perm} className="flex items-center gap-3 px-3 py-2">
              <span
                className={cn(
                  "grid size-7 shrink-0 place-items-center rounded-[8px]",
                  ok ? "bg-accent-tint text-accent-text" : "bg-muted text-muted-foreground",
                )}
              >
                {PERMISSION_ICON[perm]}
              </span>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium">{p.name}</div>
                {!ok && <div className="text-[11.5px] text-muted-foreground">{p.skippedNote}</div>}
              </div>
              {ok ? (
                <PermissionChip state="granted" />
              ) : state === "denied" ? (
                <span className="text-[12px] font-medium text-warn">Denied</span>
              ) : (
                <span className="text-[12px] text-muted-foreground">Skipped</span>
              )}
            </li>
          );
        })}
      </ul>
      <div className="mt-4 space-y-2">
        <div className="flex items-center gap-2 text-[13px]">
          <span className="text-muted-foreground">Start or stop from anywhere</span>
          <span className="flex gap-0.5" aria-label="Shift Command R">
            <Kbd>⇧</Kbd>
            <Kbd>⌘</Kbd>
            <Kbd>R</Kbd>
          </span>
        </div>
        <Note icon={<MenuSquare className="size-3.5" />}>
          Cortex lives in your menu bar. Closing a window keeps it running. Change any of this later in Settings.
        </Note>
      </div>
    </div>
  );
}

function Note({ icon, children, className }: { icon: React.ReactNode; children: React.ReactNode; className?: string }) {
  return (
    <p className={cn("flex items-start gap-2 text-[12px] leading-relaxed text-muted-foreground", className)}>
      <span className="mt-[3px] shrink-0">{icon}</span>
      <span>{children}</span>
    </p>
  );
}

import type * as React from "react";

import { CountdownSurface } from "../surfaces/countdown";
import { HudSurface } from "../surfaces/hud";
import { MainWindow } from "../surfaces/main-window";
import { MenuBarPanel } from "../surfaces/menubar-panel";
import { OnboardingWindow } from "../surfaces/onboarding";
import { RegionPicker } from "../surfaces/region-picker";
import { SettingsWindow } from "../surfaces/settings";
import { ToastStack } from "../surfaces/toasts";
import { TrimWindow } from "../surfaces/trim";

/** Props every surface receives from the prototype shell. */
export interface SurfaceProps {
  /** Live webcam stream when the reviewer turns on "Use my camera"; otherwise a placeholder. */
  cameraStream: MediaStream | null;
  /** Ask the shell to show another surface (e.g. main window → region picker). */
  navigate: (id: SurfaceId) => void;
}

export type SurfaceId =
  | "main"
  | "menubar"
  | "hud"
  | "countdown"
  | "toasts"
  | "region"
  | "settings"
  | "onboarding"
  | "trim";

/**
 * How each surface is presented in the browser:
 * - "window": inside a WindowFrame of the given size, centred on the backdrop.
 * - "overlay": full-bleed over a fake desktop (floating panels, pickers, toasts).
 */
export const SURFACES: ReadonlyArray<{
  id: SurfaceId;
  label: string;
  group: "Windows" | "Floating" | "Flows";
  frame: { kind: "window"; width: number; height: number } | { kind: "overlay" };
  Component: (p: SurfaceProps) => React.ReactNode;
}> = [
  {
    id: "main",
    label: "Main window",
    group: "Windows",
    frame: { kind: "window", width: 1180, height: 760 },
    Component: MainWindow,
  },
  {
    id: "settings",
    label: "Settings",
    group: "Windows",
    frame: { kind: "window", width: 860, height: 620 },
    Component: SettingsWindow,
  },
  {
    id: "trim",
    label: "Quick trim",
    group: "Windows",
    frame: { kind: "window", width: 1000, height: 680 },
    Component: TrimWindow,
  },
  { id: "menubar", label: "Menu bar panel", group: "Floating", frame: { kind: "overlay" }, Component: MenuBarPanel },
  { id: "hud", label: "Recording HUD", group: "Floating", frame: { kind: "overlay" }, Component: HudSurface },
  { id: "toasts", label: "Toasts", group: "Floating", frame: { kind: "overlay" }, Component: ToastStack },
  { id: "countdown", label: "Countdown", group: "Floating", frame: { kind: "overlay" }, Component: CountdownSurface },
  {
    id: "region",
    label: "Region & window picker",
    group: "Flows",
    frame: { kind: "overlay" },
    Component: RegionPicker,
  },
  {
    id: "onboarding",
    label: "Onboarding",
    group: "Flows",
    frame: { kind: "window", width: 880, height: 560 },
    Component: OnboardingWindow,
  },
];

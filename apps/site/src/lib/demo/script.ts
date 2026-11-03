import { along, type Keyframe } from "#lib/ease.js";
import type { Sound } from "./sound.js";

/** The scene is laid out at this size and scaled to fit. */
export const STAGE = { w: 1200, h: 760 };

/**
 * The Cortex window on the stage. The app opens at 1180×760 (tauri.conf.json); the mock keeps its
 * layout (sources sidebar, preview, mixer, record bar) at a size that fits the stage.
 */
export const WINDOW = { x: 100, y: 50, w: 1000, h: 640 };

/** The sources sidebar's width, and the main column's preview frame in window coordinates. */
export const SIDEBAR_W = 272;
export const FRAME_H = 340;
/** The whole display (3456×2234), then the 16:9 area picked in chapter 1. */
export const DISPLAY_W = Math.round((FRAME_H * 3456) / 2234);
export const AREA_W = Math.round((FRAME_H * 16) / 9);
/** The frame is centred in the main column, under the 44 pt title bar and 20 pt of stage padding. */
export const FRAME_CX = SIDEBAR_W + (WINDOW.w - SIDEBAR_W) / 2;
export const FRAME_Y = 72;

/** The camera overlay at 22% of the frame's width, 2% in from the edges (Cortex's defaults). */
export const PIP = Math.round(AREA_W * 0.22);
export const PIP_MARGIN = 12;

/** The file card that opens after the recording, in stage coordinates. */
export const FILE = { x: 353, y: 160, w: 494 };
/** Where the track lanes start inside it: the card's padding, header and gap, then the ruler. */
const LANES_TOP = FILE.y + 20 + 40 + 14 + 26;
const LANE_X = FILE.x + 20 + 110;
/** The lanes' width (`WIDTH` in mock/waves.ts). */
export const LANE_W = 326;

export const DURATION = 29_500;
/** With reduced motion the scene holds here, where everything it animates is on screen. */
export const SETTLED = 13_400;

export const T = {
  areaTab: 1300,
  selectArea: 2100,
  marquee: [2600, 3500],
  source: [3700, 4300],
  scene: 5500,
  cam: [5600, 6100],
  shape: [6800, 7200],
  grab: 8000,
  drop: 8950,
  snap: [8950, 9250],
  keys: [
    { show: [9850, 11_000], at: [10_000, 10_120, 10_260] },
    { show: [17_550, 18_500], at: [17_700, 17_820, 17_960] },
  ],
  recording: [
    [10_400, 15_100],
    [16_600, 18_100],
  ],
  saving: [18_100, 18_700],
  file: [19_300, 28_900],
  trimStart: [23_900, 24_600],
  trimEnd: [24_900, 25_600],
  save: 26_300,
  exported: 27_100,
} as const;

/** How much Quick trim cuts from each end, as a fraction of the recording. */
export const CUT = { start: 0.18, end: 0.14 };

export const CLICKS = [T.areaTab, T.selectArea, T.scene, T.shape[0], T.recording[0][1], T.recording[1][0], T.save];

/** Cursor targets on the stage, over the controls that CortexWindow draws. */
const AREA_TAB = { x: WINDOW.x + 226, y: WINDOW.y + 86 };
const SELECT_AREA = { x: WINDOW.x + 136, y: WINDOW.y + 190 };
const SCREEN_CAM = { x: WINDOW.x + 136, y: WINDOW.y + 274 };
const CIRCLE = { x: WINDOW.x + 74, y: WINDOW.y + 397 };
const PAUSE = { x: WINDOW.x + FRAME_CX + 108, y: WINDOW.y + 604 };

/** The slides window on the previewed desktop, which the marquee selects (frame coordinates). */
export const SLIDES = { x: 24, y: 40, w: 300, h: 169 };
const DESK_X = WINDOW.x + FRAME_CX - DISPLAY_W / 2;
const DESK_Y = WINDOW.y + FRAME_Y;
export const MARQUEE_A = { x: DESK_X + SLIDES.x, y: DESK_Y + SLIDES.y };
export const MARQUEE_B = { x: MARQUEE_A.x + SLIDES.w, y: MARQUEE_A.y + SLIDES.h };

/** Where the overlay's centre is when grabbed, and where the cursor lets go of it. */
const AREA_X = WINDOW.x + FRAME_CX - AREA_W / 2;
export const GRAB = {
  x: AREA_X + AREA_W - PIP_MARGIN - PIP / 2,
  y: WINDOW.y + FRAME_Y + FRAME_H - PIP_MARGIN - PIP / 2,
};
export const DROP = { x: AREA_X + 92, y: GRAB.y - 6 };

const HANDLE_A = { x: LANE_X, y: LANES_TOP + 117 };
const HANDLE_B = { x: LANE_X + LANE_W, y: HANDLE_A.y };
export const TRIM_A = { x: HANDLE_A.x + CUT.start * LANE_W, y: HANDLE_A.y };
export const TRIM_B = { x: HANDLE_B.x - CUT.end * LANE_W, y: HANDLE_B.y };
const SAVE = { x: FILE.x + FILE.w - 66, y: LANES_TOP + 267 };

const CURSOR: Keyframe[] = [
  { t: 0, x: 600, y: 430 },
  { t: 500, x: 600, y: 430 },
  { t: 1200, ...AREA_TAB },
  { t: 1500, ...AREA_TAB },
  { t: 2000, ...SELECT_AREA },
  { t: 2300, ...SELECT_AREA },
  { t: T.marquee[0], ...MARQUEE_A },
  { t: T.marquee[1], ...MARQUEE_B },
  { t: 3900, x: MARQUEE_B.x + 6, y: MARQUEE_B.y + 8 },
  { t: 4700, x: 420, y: 330 },
  { t: 5400, ...SCREEN_CAM },
  { t: 6100, ...SCREEN_CAM },
  { t: 6700, ...CIRCLE },
  { t: 7300, ...CIRCLE },
  { t: 7900, ...GRAB },
  { t: T.grab, ...GRAB },
  { t: T.drop - 50, ...DROP },
  { t: 9500, x: 700, y: 330 },
  { t: 10_200, x: 760, y: 300 },
  { t: 14_400, x: 770, y: 310 },
  { t: 15_000, ...PAUSE },
  { t: 16_700, ...PAUSE },
  { t: 17_400, x: 820, y: 560 },
  { t: 18_800, x: 830, y: 566 },
  { t: 23_500, x: 520, y: 640 },
  { t: T.trimStart[0] - 100, ...HANDLE_A },
  { t: T.trimStart[0], ...HANDLE_A },
  { t: T.trimStart[1], ...TRIM_A },
  { t: T.trimEnd[0], ...HANDLE_B },
  { t: T.trimEnd[1], ...TRIM_B },
  { t: T.save - 200, ...SAVE },
  { t: T.save + 300, ...SAVE },
  { t: 27_400, x: 1060, y: 712 },
  { t: DURATION, x: 1060, y: 712 },
];

export const cursorAt = (t: number) => along(CURSOR, t);

export type Status = "preview" | "recording" | "paused" | "saving";

export function status(t: number): Status {
  if (t >= T.saving[0] && t < T.saving[1]) return "saving";
  if (T.recording.some(([a, b]) => t >= a && t < b)) return "recording";
  if (t >= T.recording[0][1] && t < T.recording[1][0]) return "paused";
  return "preview";
}

/** Milliseconds recorded by `t`, leaving out the pause. */
export function recorded(t: number) {
  return T.recording.reduce((sum, [a, b]) => sum + Math.min(Math.max(t - a, 0), b - a), 0);
}

export const clock = (ms: number) => {
  const s = Math.floor(ms / 1000);
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
};

export const CHAPTERS = [
  { label: "Pick a source", t: 0 },
  { label: "Add your camera", t: 4600 },
  { label: "Record", t: 9600 },
  { label: "Pause", t: 14_400 },
  { label: "One file", t: 18_800 },
  { label: "Quick trim", t: 23_400 },
] as const;

export type Caption = { start: number; end: number; words: { text: string; at: number }[] };

/** Spreads the words across [start, end], giving longer words more time. */
function caption(text: string, start: number, end: number): Caption {
  const parts = text.split(" ");
  const weights = parts.map((w) => w.length + 2);
  const total = weights.reduce((a, b) => a + b, 0);
  let at = start;
  const words = parts.map((w, i) => {
    const word = { text: w, at };
    at += ((end - start) * (weights[i] ?? 0)) / total;
    return word;
  });
  return { start, end, words };
}

export const CAPTIONS = [
  caption("Record a display, a window, an app, or just an area.", 700, 4000),
  caption("Turn on your camera, and drag it anywhere.", 5000, 8600),
  caption("Press ⇧⌘R in any app, and you're recording.", 10_500, 13_600),
  caption("Pause and resume, with no gap in the file.", 15_300, 17_400),
  caption("Stop, and the camera has its own track too.", 19_900, 22_600),
  caption("Trim the ends, with no re-encode.", 24_000, 26_800),
];

export type Cue = { t: number; sound: Sound; variant?: number };

const cue = (t: number, sound: Sound, variant?: number): Cue => ({ t, sound, variant });

export const CUES: Cue[] = [
  ...CLICKS.map((t) => cue(t, "switch")),
  cue(T.selectArea + 40, "pop"),
  cue(T.marquee[0], "tick"),
  cue(T.marquee[1], "tick"),
  cue(T.source[0] + 50, "sweep"),
  cue(T.cam[0] + 60, "pop"),
  cue(T.shape[0] + 80, "whoosh"),
  cue(T.grab, "tick"),
  cue(T.grab + 100, "whoosh"),
  cue(T.snap[0] + 40, "pop"),
  ...T.keys.flatMap((k) => k.at.map((t, i) => cue(t, "key", i))),
  cue(T.recording[0][0], "start"),
  cue(T.saving[0], "stop"),
  cue(T.saving[1], "chime"),
  cue(T.file[0] + 80, "sweep"),
  cue(T.trimStart[0], "tick"),
  cue(T.trimStart[1], "tick"),
  cue(T.trimEnd[0], "tick"),
  cue(T.trimEnd[1], "tick"),
  cue(T.exported, "chime"),
].toSorted((a, b) => a.t - b.t);

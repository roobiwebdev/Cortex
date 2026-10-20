import { along, type Keyframe } from '#lib/ease.js';
import type { Sound } from './sound.js';

/** The scene is laid out at this size and scaled to fit. */
export const STAGE = { w: 1200, h: 760 };

/** The Rec window on the stage, at the app's own sizes in points (src/app/ui.rs). */
export const WINDOW = { x: 100, y: 50, w: 1000, h: 640 };

/** The preview area inside the window, after its 16 pt padding. */
export const AREA = { x: 16, y: 56, w: 664, h: 500 };

/** The previewed frame: the whole 16:9 display, then a 16:10 window. */
export const DISPLAY_H = 374;
export const WINDOW_H = 415;

/** The camera overlay at 22% of the frame's width, 2% in from the edges (src/compose.rs). */
export const PIP = 146;
export const PIP_MARGIN = 13;

export const DURATION = 24_000;
/** With reduced motion the scene holds here, where everything it animates is on screen. */
export const SETTLED = 13_400;

export const T = {
	popup: [1350, 2550],
	source: [2600, 3150],
	scene: 5500,
	cam: [5600, 6100],
	shape: [6800, 7200],
	grab: 8000,
	drop: 8950,
	snap: [8950, 9250],
	keys: [
		{ show: [9850, 11_000], at: [10_000, 10_120, 10_260] },
		{ show: [17_550, 18_500], at: [17_700, 17_820, 17_960] }
	],
	recording: [
		[10_400, 15_100],
		[16_600, 18_100]
	],
	saving: [18_100, 18_700],
	file: [19_600, 23_300]
} as const;

export const CLICKS = [T.popup[0], T.popup[1], T.scene, T.shape[0], T.recording[0][1], T.recording[1][0], T.file[0]];

/** Cursor targets on the stage, over the controls that RecWindow draws. */
const WINDOW_COMBO = { x: 932, y: 244 };
export const POPUP_ROWS = { x: 825, y: 262, w: 215, h: 22 };
const SLIDES_ROW = { x: 932, y: POPUP_ROWS.y + POPUP_ROWS.h * 3.5 };
const SCREEN_CAM = { x: 948, y: 142 };
const SQUIRCLE = { x: 1050, y: 412 };
const PAUSE = { x: 208, y: 656 };
const REVEAL = { x: 953, y: 656 };
/** Where the overlay's center is when grabbed, and where the cursor lets go of it. */
export const GRAB = { x: 202, y: 478 };
export const DROP = { x: 680, y: 470 };

const CURSOR: Keyframe[] = [
	{ t: 0, x: 560, y: 430 },
	{ t: 500, x: 560, y: 430 },
	{ t: 1300, ...WINDOW_COMBO },
	{ t: 1700, ...WINDOW_COMBO },
	{ t: 2400, ...SLIDES_ROW },
	{ t: 4700, x: 940, y: 330 },
	{ t: 5400, ...SCREEN_CAM },
	{ t: 6100, ...SCREEN_CAM },
	{ t: 6700, ...SQUIRCLE },
	{ t: 7300, ...SQUIRCLE },
	{ t: 7900, ...GRAB },
	{ t: T.grab, ...GRAB },
	{ t: T.drop - 50, ...DROP },
	{ t: 9500, x: 700, y: 452 },
	{ t: 10_200, x: 380, y: 500 },
	{ t: 14_400, x: 386, y: 506 },
	{ t: 15_000, ...PAUSE },
	{ t: 16_700, ...PAUSE },
	{ t: 17_400, x: 430, y: 560 },
	{ t: 18_800, x: 440, y: 566 },
	{ t: 19_500, ...REVEAL },
	{ t: 20_000, ...REVEAL },
	{ t: 20_800, x: 1060, y: 712 },
	{ t: DURATION, x: 1060, y: 712 }
];

export const cursorAt = (t: number) => along(CURSOR, t);

export type Status = 'preview' | 'recording' | 'paused' | 'saving';

export function status(t: number): Status {
	if (t >= T.saving[0] && t < T.saving[1]) return 'saving';
	if (T.recording.some(([a, b]) => t >= a && t < b)) return 'recording';
	if (t >= T.recording[0][1] && t < T.recording[1][0]) return 'paused';
	return 'preview';
}

/** Milliseconds recorded by `t`, leaving out the pause. */
export function recorded(t: number) {
	return T.recording.reduce((sum, [a, b]) => sum + Math.min(Math.max(t - a, 0), b - a), 0);
}

export const clock = (ms: number) => {
	const s = Math.floor(ms / 1000);
	return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
};

export const CHAPTERS = [
	{ label: 'Pick a source', t: 0 },
	{ label: 'Add your camera', t: 4600 },
	{ label: 'Record', t: 9600 },
	{ label: 'Pause', t: 14_400 },
	{ label: 'One file', t: 18_800 }
] as const;

export type Caption = { start: number; end: number; words: { text: string; at: number }[] };

/** Spreads the words across [start, end], giving longer words more time. */
function caption(text: string, start: number, end: number): Caption {
	const parts = text.split(' ');
	const weights = parts.map((w) => w.length + 2);
	const total = weights.reduce((a, b) => a + b, 0);
	let at = start;
	const words = parts.map((w, i) => {
		const word = { text: w, at };
		at += ((end - start) * weights[i]) / total;
		return word;
	});
	return { start, end, words };
}

export const CAPTIONS = [
	caption('Record a whole display, or just one window.', 700, 3400),
	caption('Add your camera, and drag it anywhere.', 5000, 8600),
	caption("Press ⇧⌘R in any app, and you're recording.", 10_500, 13_600),
	caption('Pause and resume, with no gap in the file.', 15_300, 17_400),
	caption("Stop, and it's one file, ready to edit.", 19_900, 22_600)
];

export type Cue = { t: number; sound: Sound; variant?: number };

const cue = (t: number, sound: Sound, variant?: number): Cue => ({ t, sound, variant });

export const CUES: Cue[] = [
	...CLICKS.map((t) => cue(t, 'switch')),
	cue(T.popup[0] + 40, 'pop'),
	...[1900, 2100, 2300].map((t) => cue(t, 'tick')),
	cue(T.source[0] + 50, 'sweep'),
	cue(T.cam[0] + 60, 'pop'),
	cue(T.shape[0] + 80, 'whoosh'),
	cue(T.grab, 'tick'),
	cue(T.grab + 100, 'whoosh'),
	cue(T.snap[0] + 40, 'pop'),
	...T.keys.flatMap((k) => k.at.map((t, i) => cue(t, 'key', i))),
	cue(T.recording[0][0], 'start'),
	cue(T.saving[0], 'stop'),
	cue(T.saving[1], 'chime'),
	cue(T.file[0] + 80, 'sweep')
].sort((a, b) => a.t - b.t);

import { clamp, random } from '#lib/ease.js';

/** Bars across a track lane: 2 px wide, 1 px apart. */
export const BARS = 109;
const STEP = 3;

function levels(seed: number, shape: (i: number) => number) {
	const r = random(seed);
	return Array.from({ length: BARS }, (_, i) => clamp(shape(i) * (0.5 + 0.5 * r())));
}

/** System audio runs on; the mic is speech, with pauses between phrases. */
export const SYSTEM = levels(3, (i) => 0.5 + 0.2 * Math.sin(i / 6));
export const MIC = levels(7, (i) => (Math.sin(i / 4.4) + Math.sin(i / 1.9) * 0.3 > -0.35 ? 0.9 : 0.04));
export const MIX = SYSTEM.map((s, i) => clamp(0.15 + Math.max(s * 0.8, MIC[i])));

/** An SVG path of mirrored bars `height` tall. */
export function bars(values: number[], height: number) {
	const mid = height / 2;
	return values
		.map((v, i) => {
			const half = Math.max(0.5, (v * height) / 2);
			return `M${i * STEP} ${(mid - half).toFixed(2)}h2v${(half * 2).toFixed(2)}h-2z`;
		})
		.join('');
}

export const WIDTH = BARS * STEP - 1;

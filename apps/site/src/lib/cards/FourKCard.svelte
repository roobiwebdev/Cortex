<script lang="ts">
	import { easeInOut, lerp, span } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Fit from './Fit.svelte';

	/**
	 * The resolution picker on a 3456×2234 MacBook Pro display, with the frame each choice writes
	 * (docs/engineering/desktop.md, "Resolution and bitrate"; crates/rec-core/src/output.rs).
	 */
	const CHOICES = [
		{ label: '720p', w: 1114, h: 720 },
		{ label: '1080p', w: 1670, h: 1080 },
		{ label: '1440p', w: 2228, h: 1440 },
		{ label: '4K', w: 3840, h: 2482 },
		{ label: 'Native', w: 3456, h: 2234 }
	];
	const SOURCE = { w: 3456, h: 2234 };
	const STEP = 1800;
	/** Drawn at this many card pixels per frame pixel, so the 4K frame is 300 wide. */
	const SCALE = 300 / 3840;

	const clock = loop(CHOICES.length * STEP, 3 * STEP + 900);
	let t = $derived(clock.t);
	let i = $derived(Math.floor(t / STEP) % CHOICES.length);
	let choice = $derived(CHOICES[i] ?? CHOICES[3]);
	let prev = $derived(CHOICES[(i + CHOICES.length - 1) % CHOICES.length] ?? CHOICES[3]);
	let p = $derived(easeInOut(span(t % STEP, 0, 420)));
	let w = $derived(lerp((prev?.w ?? 0) * SCALE, (choice?.w ?? 0) * SCALE, p));
	let h = $derived(lerp((prev?.h ?? 0) * SCALE, (choice?.h ?? 0) * SCALE, p));
</script>

<div class="card-visual app" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="picker">
			<span class="thumb" style:transform="translateX({i * 100}%)"></span>
			{#each CHOICES as c, j (c.label)}
				<span class={{ on: j === i }}>{c.label}</span>
			{/each}
		</div>

		<div class="frames" aria-hidden="true">
			<div class="out" style:width="{w}px" style:height="{h}px"></div>
			<div class="source" style:width="{SOURCE.w * SCALE}px" style:height="{SOURCE.h * SCALE}px"></div>
		</div>

		<div class="size">
			<b>{choice?.w} × {choice?.h}</b>
			<span>in the file, at the screen's shape</span>
			<span class="from">Screen: {SOURCE.w} × {SOURCE.h}</span>
		</div>

		<div class="legend">
			<span><i class="key-out"></i>File</span>
			<span><i class="key-source"></i>Screen</span>
		</div>
	</Fit>
</div>

<style>
	.card-visual {
		background: radial-gradient(ellipse 70% 60% at 50% 0%, hsl(188 60% 55% / 0.09), transparent);
		color: var(--fg);
	}

	.picker {
		position: absolute;
		left: 28px;
		top: 26px;
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		width: 300px;
		height: 30px;
		padding: 2px;
		border-radius: 8px;
		background: var(--segments);
		color: var(--secondary);
		font-size: 12px;
	}

	.picker span {
		position: relative;
		display: grid;
		place-items: center;
	}

	.picker .on {
		color: var(--text);
		font-weight: 600;
	}

	.picker .thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: calc((100% - 4px) / 5);
		height: 26px;
		border-radius: 6px;
		background: var(--segment);
		box-shadow: 0 1px 0 rgba(0, 0, 0, 0.18);
		transition: transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1);
	}

	.frames {
		position: absolute;
		left: 28px;
		top: 76px;
		width: 300px;
		height: 196px;
	}

	.out,
	.source {
		position: absolute;
		left: 0;
		bottom: 0;
		border-radius: 4px;
	}

	.out {
		background: linear-gradient(160deg, hsl(188 45% 40% / 0.55), hsl(188 45% 22% / 0.55));
		box-shadow: inset 0 0 0 1.5px var(--accent);
	}

	.source {
		border: 1.5px dashed color-mix(in srgb, var(--fg) 55%, transparent);
	}

	.size {
		position: absolute;
		left: 350px;
		top: 140px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 140px;
	}

	.size b {
		font: 700 24px/28px var(--display);
		letter-spacing: -0.03em;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.size span {
		color: var(--muted);
		font-size: 12px;
		line-height: 16px;
	}

	.size .from {
		margin-top: 8px;
		font-variant-numeric: tabular-nums;
	}

	.legend {
		position: absolute;
		left: 350px;
		top: 252px;
		display: flex;
		gap: 14px;
		color: var(--muted);
		font-size: 11px;
	}

	.legend span {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.legend i {
		width: 12px;
		height: 9px;
		border-radius: 2px;
	}

	.key-out {
		background: hsl(188 45% 32% / 0.7);
		box-shadow: inset 0 0 0 1.5px var(--accent);
	}

	.key-source {
		border: 1.5px dashed color-mix(in srgb, var(--fg) 55%, transparent);
	}
</style>

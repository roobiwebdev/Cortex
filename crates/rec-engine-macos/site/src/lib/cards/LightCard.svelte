<script lang="ts">
	import { random } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Fit from './Fit.svelte';

	/** Sparkline y positions: a low, steady load with an occasional blip, scrolling left. Decorative only. */
	const N = 96;
	const STEP = 4;
	const rand = random(11);
	const LOAD = Array.from({ length: N }, (_, i) => 34 + (rand() - 0.5) * 9 + (i % 31 === 17 ? -12 : 0));

	const clock = loop(N * 250, 0);
	let shift = $derived((clock.t / 250) % N);
	let line = $derived.by(() => {
		const offset = shift % 1;
		const start = Math.floor(shift);
		return Array.from({ length: 49 }, (_, i) => {
			const x = (i - offset) * STEP;
			return `${i ? 'L' : 'M'}${x.toFixed(1)} ${LOAD[(start + i) % N].toFixed(1)}`;
		}).join(' ');
	});

	const BARS = [
		{ label: 'Screen and system audio, 4K60', value: 6 },
		{ label: 'With the webcam overlay, 4K60', value: 34 }
	];
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="stat">
			<b>6%</b>
			<span>of one core, recording a 4K60 screen</span>
		</div>
		<svg class="history" width="190" height="64" viewBox="0 0 190 64" aria-hidden="true">
			<path d="{line} L190 64 L0 64 Z" class="area" />
			<path d={line} class="line" />
		</svg>
		<div class="bars">
			{#each BARS as bar (bar.label)}
				<div class="bar">
					<div class="bar-label"><span>{bar.label}</span><span class="value">{bar.value}%</span></div>
					<div class="meter"><i style:width="{bar.value}%"></i></div>
				</div>
			{/each}
		</div>
		<div class="chips">
			<span>22 MB app</span>
			<span>108,000 of 108,000 frames</span>
			<span>Crash-safe files</span>
		</div>
	</Fit>
</div>

<style>
	.stat {
		position: absolute;
		left: 28px;
		top: 30px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.stat b {
		font: 700 56px/60px var(--display);
		letter-spacing: -0.04em;
	}

	.stat span,
	.value {
		color: var(--muted);
		font-size: 13px;
		line-height: 18px;
	}

	.history {
		position: absolute;
		top: 34px;
		right: 28px;
		overflow: hidden;
	}

	.area {
		fill: rgba(48, 209, 88, 0.16);
	}

	.line {
		fill: none;
		stroke: #30d158;
		stroke-width: 1.5;
		stroke-linejoin: round;
	}

	.bars {
		position: absolute;
		left: 28px;
		top: 136px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		width: 450px;
	}

	.bar {
		display: flex;
		flex-direction: column;
		gap: 7px;
	}

	.bar-label {
		display: flex;
		justify-content: space-between;
		font-size: 13px;
		line-height: 16px;
	}

	.value {
		font-variant-numeric: tabular-nums;
	}

	.meter {
		height: 6px;
		border-radius: 3px;
		background: color-mix(in srgb, var(--fg) 8%, transparent);
	}

	.meter i {
		display: block;
		height: 100%;
		border-radius: 3px;
		background: #30d158;
	}

	:global(:root[data-theme='light']) .line {
		stroke: #1c9640;
	}

	:global(:root[data-theme='light']) .area {
		fill: rgba(28, 150, 64, 0.12);
	}

	:global(:root[data-theme='light']) .meter i {
		background: #1c9640;
	}

	.chips {
		position: absolute;
		left: 28px;
		top: 246px;
		display: flex;
		gap: 8px;
		font-size: 12px;
		line-height: 16px;
	}

	.chips span {
		padding: 5px 10px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--fg) 6%, transparent);
		color: color-mix(in srgb, var(--fg) 78%, transparent);
	}
</style>

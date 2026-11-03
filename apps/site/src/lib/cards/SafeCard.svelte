<script lang="ts">
	import { span } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Fit from './Fit.svelte';

	/**
	 * The file is written in 2-second fragments (crates/rec-engine-macos writer), so when the app
	 * stops without warning, everything up to the last finished fragment still plays. Decorative.
	 */
	const FRAGMENTS = 14;
	const WRITE = 6200;
	const CRASH = 6200;

	const clock = loop(9000, 8000);
	let t = $derived(clock.t);
	let written = $derived(span(t, 0, WRITE) * (FRAGMENTS - 0.6));
	let crashed = $derived(t >= CRASH);
	let kept = $derived(Math.floor(written));
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="stat">
			<b>36,000 of 36,000</b>
			<span>frames kept in a 10-minute 4K60 HEVC test</span>
		</div>

		<div class="file" aria-hidden="true">
			<div class="fragments">
				{#each { length: FRAGMENTS }, i (i)}
					<i
						class={{ done: i < kept, open: i === kept && !crashed, lost: crashed && i === kept }}
						style:--fill="{Math.min(1, Math.max(0, written - i)) * 100}%"
					></i>
				{/each}
			</div>
			<div class="scale"><span>0 s</span><span>2 s fragments</span><span>{FRAGMENTS * 2} s</span></div>
			<p class={['verdict', { shown: crashed }]}>
				<svg viewBox="0 0 14 14" aria-hidden="true"><path d="M3 7.4 5.8 10 11 4.2" /></svg>
				Crash at {(written * 2).toFixed(0)} s: the file still plays to {kept * 2} s
			</p>
		</div>

		<div class="chips">
			<!-- `du -sh /Applications/Cortex.app` on the 0.1.0 build: 22M. -->
			<span>22 MB app</span>
			<span>Crash-safe files</span>
			<span>Quit saves the file first</span>
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
		font: 700 44px/50px var(--display);
		letter-spacing: -0.04em;
		font-variant-numeric: tabular-nums;
	}

	.stat span {
		color: var(--muted);
		font-size: 13px;
		line-height: 18px;
	}

	.file {
		position: absolute;
		left: 28px;
		top: 130px;
		width: 450px;
	}

	.fragments {
		display: flex;
		gap: 3px;
	}

	.fragments i {
		position: relative;
		flex: 1;
		height: 30px;
		overflow: hidden;
		border-radius: 4px;
		background: color-mix(in srgb, var(--fg) 7%, transparent);
	}

	.fragments i::after {
		content: '';
		position: absolute;
		inset: 0 auto 0 0;
		width: var(--fill);
		background: hsl(188 58% 58% / 0.45);
	}

	.fragments .done::after {
		background: var(--brand);
	}

	.fragments .lost::after {
		background: repeating-linear-gradient(135deg, rgba(240, 86, 91, 0.55) 0 4px, transparent 4px 8px);
	}

	.scale {
		display: flex;
		justify-content: space-between;
		margin-top: 8px;
		color: var(--muted);
		font: 11px/14px var(--mono);
	}

	.verdict {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 14px 0 0;
		color: var(--fg);
		font-size: 13px;
		opacity: 0;
		transition: opacity 0.3s;
	}

	.verdict.shown {
		opacity: 1;
	}

	.verdict svg {
		width: 14px;
		height: 14px;
		fill: none;
		stroke: var(--brand);
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
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

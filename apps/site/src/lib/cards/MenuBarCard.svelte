<script lang="ts">
	import { envelope } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import { clock as format } from '#lib/demo/script.js';
	import Fit from './Fit.svelte';

	/**
	 * ⇧⌘R pressed over another app starts a recording, and the floating HUD shows the time. Then the
	 * menu bar panel opens: the same Stop button, the sources, and the app's links. Decorative only.
	 */
	const PRESSES = [
		{ at: [600, 740, 880], until: 1300 },
		{ at: [7800, 7940, 8080], until: 8500 }
	];
	const START = 1000;
	const STOP = 8200;

	const clock = loop(9000, 5000);
	let t = $derived(clock.t);
	let recording = $derived(t >= START && t < STOP);
	let hud = $derived(envelope(t, START, START + 260, STOP, STOP + 240));
	let panel = $derived(envelope(t, 2600, 2780, 7000, 7200));
	let press = $derived(PRESSES.find((p) => t >= p.at[0] && t < p.until) ?? null);
	let elapsed = $derived(format(Math.max(0, t - START)));

	const SOURCES = [
		{ label: 'Camera', detail: 'FaceTime HD Camera', on: true },
		{ label: 'Microphone', detail: 'MacBook Pro Microphone', on: true },
		{ label: 'System audio', detail: 'Sound from apps', on: true }
	];
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="backdrop"></div>
		<div class="menubar">
			<span class={['status', { open: panel > 0.5 }]}>
				<svg viewBox="0 0 14 14" aria-hidden="true">
					<circle cx="7" cy="7" r="5.9" class="ring" />
					<circle cx="7" cy="7" r="2.6" class="fill" />
				</svg>
			</span>
			<span>Tue 9:41 AM</span>
		</div>

		<div class="keys">
			{#each ['⇧', '⌘', 'R'] as key, i (key)}
				<kbd class={{ down: press !== null && t >= (press.at[i] ?? 0), r: key === 'R' }}>{key}</kbd>
			{/each}
		</div>
		<p class="hint">from any app</p>

		{#if hud > 0}
			<div class="hud" style:opacity={hud} style:transform="translateY({(1 - hud) * 8}px)">
				<span class="hud-stop"><i></i></span>
				<span class="hud-pause"><i></i><i></i></span>
				<span class="hud-time"><small>REC</small>{elapsed}</span>
			</div>
		{/if}

		{#if panel > 0}
			<div class="panel" style:opacity={panel} style:transform="translateY({(1 - panel) * -4}px)">
				<div class="panel-head">
					<b>Cortex</b>
					<span class={['state', { rec: recording }]}>{recording ? 'RECORDING' : 'READY'}</span>
				</div>
				<div class="sources">
					{#each SOURCES as s (s.label)}
						<div class="source">
							<div><b>{s.label}</b><span>{s.detail}</span></div>
							<span class={['switch', { on: s.on }]}><i></i></span>
						</div>
					{/each}
				</div>
				<div class={['start', { rec: recording }]}>
					<span>{recording ? 'Stop recording' : 'Start recording'}</span>
					<kbd>⇧⌘R</kbd>
				</div>
				<div class="links"><span class="accent">Open Cortex</span><span>Settings ⌘,</span><span>Quit</span></div>
			</div>
		{/if}
	</Fit>
</div>

<style>
	.backdrop {
		position: absolute;
		inset: 0;
		background-color: #0b1214;
		background-image:
			radial-gradient(circle farthest-corner at 10% 70%, hsl(188 55% 40% / 0.45), transparent 60%),
			radial-gradient(circle farthest-corner at 100% 100%, hsl(196 50% 30% / 0.35), transparent 60%),
			linear-gradient(160deg, #10262b, #0a1215 55%, #0c1a1f);
	}

	.menubar {
		position: absolute;
		inset: 0 0 auto;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 12px;
		height: 26px;
		padding: 0 14px;
		background: rgba(10, 10, 14, 0.36);
		color: #f4f4f5;
		font-size: 13px;
		font-variant-numeric: tabular-nums;
	}

	.status {
		display: flex;
		align-items: center;
		height: 20px;
		padding: 0 7px;
		border-radius: 5px;
	}

	.status.open {
		background: rgba(255, 255, 255, 0.16);
	}

	.status svg {
		width: 14px;
		height: 14px;
	}

	.status .fill {
		fill: currentColor;
	}

	.status .ring {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.2;
	}

	.keys {
		position: absolute;
		left: 40px;
		top: 92px;
		display: flex;
		gap: 8px;
	}

	kbd {
		display: grid;
		place-items: center;
		width: 46px;
		height: 46px;
		border-radius: 11px;
		background: linear-gradient(#3a3a3e, #2a2a2e);
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.14),
			0 3px 0 #101012,
			0 8px 20px rgba(0, 0, 0, 0.4);
		color: #f4f4f5;
		font: 500 18px var(--sans);
		transition:
			translate 0.08s,
			box-shadow 0.08s;
	}

	kbd.r {
		background: linear-gradient(#fff, #f2f2f6 55%, #e6e6ec);
		color: #1c1c1e;
		font-weight: 700;
	}

	kbd.down {
		translate: 0 2px;
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.14),
			0 1px 0 #101012,
			0 4px 12px rgba(0, 0, 0, 0.4);
	}

	.hint {
		position: absolute;
		left: 40px;
		top: 152px;
		margin: 0;
		color: rgba(244, 244, 245, 0.62);
		font-size: 12px;
	}

	/* The recording HUD: a capsule that floats over every app and stays out of the recording. */
	.hud {
		position: absolute;
		left: 40px;
		top: 214px;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 7px 14px 7px 7px;
		border-radius: 999px;
		background: rgba(22, 24, 27, 0.92);
		box-shadow:
			inset 0 0 0 1px rgba(255, 255, 255, 0.1),
			0 12px 30px rgba(0, 0, 0, 0.45);
		color: #ecedee;
	}

	.hud-stop,
	.hud-pause {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		border-radius: 50%;
	}

	.hud-stop {
		background: #f0565b;
	}

	.hud-stop i {
		width: 11px;
		height: 11px;
		border-radius: 2.5px;
		background: #fff;
	}

	.hud-pause {
		display: flex;
		justify-content: center;
		gap: 3px;
		background: rgba(255, 255, 255, 0.08);
	}

	.hud-pause i {
		width: 3px;
		height: 11px;
		border-radius: 1px;
		background: #ecedee;
	}

	.hud-time {
		display: flex;
		flex-direction: column;
		font: 500 17px/19px var(--mono);
		font-variant-numeric: tabular-nums;
	}

	.hud-time small {
		color: #f0565b;
		font: 700 9px/11px var(--sans);
		letter-spacing: 0.08em;
	}

	.panel {
		position: absolute;
		top: 30px;
		right: 16px;
		width: 232px;
		padding: 10px;
		border-radius: 14px;
		background: rgba(22, 24, 27, 0.94);
		box-shadow:
			inset 0 0 0 1px rgba(255, 255, 255, 0.08),
			0 18px 40px rgba(0, 0, 0, 0.5);
		color: #ecedee;
		font-size: 12px;
		line-height: 14px;
	}

	.panel-head {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 2px 2px 8px;
	}

	.state {
		color: #9a9da3;
		font-size: 9.5px;
		font-weight: 600;
		letter-spacing: 0.08em;
	}

	.state.rec {
		color: #f0565b;
	}

	.sources {
		border-radius: 9px;
		background: #1d2024;
	}

	.source {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 34px;
		padding: 0 9px;
	}

	.source + .source {
		border-top: 1px solid #2a2d32;
	}

	.source div {
		display: flex;
		flex-direction: column;
	}

	.source b {
		font-weight: 500;
	}

	.source span:not(.switch) {
		color: #9a9da3;
		font-size: 10px;
		line-height: 12px;
	}

	.switch {
		position: relative;
		width: 26px;
		height: 15px;
		border-radius: 8px;
		background: rgba(255, 255, 255, 0.15);
	}

	.switch.on {
		background: hsl(188 58% 58%);
	}

	.switch i {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 11px;
		height: 11px;
		border-radius: 50%;
		background: #fff;
	}

	.switch.on i {
		left: 13px;
		background: hsl(188 60% 7%);
	}

	.start {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 34px;
		margin-top: 8px;
		padding: 0 10px;
		border-radius: 9px;
		background: #f0565b;
		box-shadow: 0 2px 0 #b33237;
		color: #fff;
		font-weight: 600;
	}

	.start kbd {
		width: auto;
		height: auto;
		padding: 1px 5px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.22);
		box-shadow: none;
		color: #fff;
		font: 600 9.5px/13px var(--sans);
	}

	.links {
		display: flex;
		justify-content: space-between;
		padding: 10px 2px 0;
		color: #9a9da3;
		font-size: 11px;
	}

	.links .accent {
		color: hsl(188 58% 66%);
	}
</style>

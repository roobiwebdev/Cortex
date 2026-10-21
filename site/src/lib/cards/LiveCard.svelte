<script lang="ts">
	import { loop } from '#lib/motion.svelte.js';
	import { clock as format } from '#lib/demo/script.js';
	import Fit from './Fit.svelte';

	const SAVE = 2400;
	/** The app shows a notice for four seconds. */
	const NOTICE = 4000;

	const clock = loop(8000, 3500);
	let t = $derived(clock.t);
	let saved = $derived(t >= SAVE + 60 && t < SAVE + NOTICE);
	let bitrate = $derived((6 + Math.sin(Math.floor(t / 500) * 2.1) * 0.12).toFixed(1));
</script>

<div class="card-visual app" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="toolbar">
			<span class="pill">LIVE</span>
			{#if saved}<span class="notice">Replay saved: replay-2026-10-20-094530.mp4</span>{/if}
		</div>

		<div class="columns">
			<section>
				<div class="header">
					<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
						<path d="M2.6 7a4.4 4.4 0 1 0 1.3-3.1M2.4 1.8v2.6H5" />
					</svg>
					REPLAY BUFFER
				</div>
				<div class="group">
					<div class="row"><span>Keep recent footage</span><span class="switch"><i></i></span></div>
					<div class="row slider">
						<span class="label">Length</span>
						<span class="track"><i style:width="16%"></i><b style:left="16%"></b></span>
						<span class="value">60 s</span>
					</div>
					<i class="divider"></i>
					<div class="save">
						<span class={['button', { pressed: t >= SAVE && t < SAVE + 150 }]}>Save Replay</span>
					</div>
					<span class="small">60 s · 45 MB in memory</span>
				</div>
			</section>

			<section>
				<div class="header">
					<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
						<path d="M7 6.2v.1M4.8 4.2a3 3 0 0 0 0 4.2M9.2 4.2a3 3 0 0 1 0 4.2M3.1 2.6a5.3 5.3 0 0 0 0 7.6M10.9 2.6a5.3 5.3 0 0 1 0 7.6" />
					</svg>
					STREAM
				</div>
				<div class="group">
					<div class="combo off">
						<span>Custom (RTMP or WHIP URL)</span>
						<svg width="8" height="10" viewBox="0 0 8 10" aria-hidden="true"><path d="M1 3.4 4 1.2l3 2.2M1 6.6l3 2.2 3-2.2" /></svg>
					</div>
					<div class="field off">rtmps://live.example.com/app</div>
					<div class="row"><span>Stream key</span><span class="small">In Keychain</span></div>
					<i class="divider"></i>
					<span class="stop">Stop Stream</span>
					<span class="small live">{format(42_000 + t)}&nbsp;&nbsp;{bitrate} Mb/s&nbsp;&nbsp;0 dropped</span>
				</div>
			</section>
		</div>
	</Fit>
</div>

<style>
	.card-visual {
		background: var(--sidebar);
		color: var(--text);
		font-size: 13px;
		line-height: 16px;
	}

	:global(:root:not([data-theme='light'])) .card-visual {
		background: #202022;
	}

	.toolbar {
		position: absolute;
		left: 22px;
		top: 40px;
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.pill {
		padding: 3px 8px;
		border-radius: 9px;
		background: var(--rec);
		color: #fff;
		font-size: 11px;
		font-weight: 600;
		line-height: 13px;
	}

	.notice {
		color: var(--secondary);
		font-size: 11px;
	}

	.columns {
		position: absolute;
		left: 22px;
		top: 70px;
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		width: 462px;
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.header {
		display: flex;
		align-items: center;
		gap: 6px;
		padding-left: 2px;
		color: var(--secondary);
		font-size: 10.5px;
		font-weight: 600;
		letter-spacing: 0.06em;
		line-height: 14px;
	}

	svg {
		flex-shrink: 0;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.3;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 12px;
		border-radius: 10px;
		background: var(--group);
		box-shadow: inset 0 0 0 1px var(--group-edge);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 28px;
	}

	.switch {
		position: relative;
		width: 30px;
		height: 18px;
		border-radius: 9px;
		background: var(--accent);
	}

	.switch i {
		position: absolute;
		top: 2px;
		left: 14px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.3);
	}

	.slider {
		justify-content: flex-start;
		gap: 8px;
		color: var(--secondary);
		font-size: 12px;
	}

	.label {
		width: 48px;
	}

	.track {
		position: relative;
		flex: 1;
		height: 4px;
		border-radius: 2px;
		background: var(--track);
	}

	.track i {
		position: absolute;
		inset: 0 auto 0 0;
		border-radius: 2px;
		background: var(--accent);
	}

	.track b {
		position: absolute;
		top: -5px;
		width: 14px;
		height: 14px;
		margin-left: -7px;
		border-radius: 50%;
		background: #ececee;
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
	}

	:global(:root[data-theme='light']) .track b {
		background: #fff;
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.12),
			0 1px 2px rgba(0, 0, 0, 0.3);
	}

	.value {
		width: 32px;
		text-align: right;
	}

	.divider {
		display: block;
		height: 1px;
		background: color-mix(in srgb, var(--separator) 70%, transparent);
	}

	.save {
		display: flex;
	}

	.button {
		display: inline-flex;
		align-items: center;
		height: 22px;
		padding: 0 9px;
		border-radius: 6px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
		transition: filter 0.1s;
	}

	.pressed {
		filter: brightness(1.35);
	}

	:global(:root[data-theme='light']) .pressed {
		filter: brightness(0.85);
	}

	.small {
		color: var(--secondary);
		font-size: 11px;
		line-height: 14px;
	}

	.small.live {
		color: var(--text);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.combo,
	.field {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		height: 24px;
		padding: 0 8px;
		border-radius: 6px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
		white-space: nowrap;
	}

	.combo span {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.combo svg {
		stroke: var(--chevrons);
	}

	.field {
		overflow: hidden;
		font-size: 12px;
	}

	.off {
		opacity: 0.45;
	}

	.stop {
		display: grid;
		place-items: center;
		height: 28px;
		border-radius: 7px;
		background: var(--rec);
		color: #fff;
		font-weight: 600;
	}
</style>

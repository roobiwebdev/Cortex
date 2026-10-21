<script lang="ts">
	import { envelope } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import { clock as format } from '#lib/demo/script.js';
	import Fit from './Fit.svelte';

	const PRESSES = [
		{ at: [600, 740, 880], until: 1300 },
		{ at: [7800, 7940, 8080], until: 8500 }
	];
	const START = 1000;
	const STOP = 8200;

	const clock = loop(9000, 5000);
	let t = $derived(clock.t);
	let recording = $derived(t >= START && t < STOP);
	let menu = $derived(envelope(t, 2600, 2780, 7400, 7600));
	let highlight = $derived(t >= 3000 && t < 7400);
	let press = $derived(PRESSES.find((p) => t >= p.at[0] && t < p.until) ?? null);

	let items = $derived([
		[
			{ label: recording ? 'Stop Recording' : 'Start Recording', key: '⇧⌘R' },
			{ label: 'Pause', off: !recording },
			{ label: 'Save Replay', key: '⇧⌘S' },
			{ label: 'Go Live' }
		],
		[{ label: 'Show Rec Window' }, { label: 'Settings…', key: '⌘,' }],
		[{ label: 'Reveal Last Recording' }, { label: 'Open Recordings Folder' }],
		[
			{ label: 'Global hotkey: ⇧⌘R', off: true },
			{ label: 'Quit Rec', key: '⌘Q' }
		]
	]);
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="backdrop"></div>
		<div class="menubar">
			<span class={['status', { rec: recording, open: menu > 0.5 }]}>
				<svg viewBox="0 0 14 14" aria-hidden="true">
					{#if recording}
						<circle cx="7" cy="7" r="6.5" class="fill" />
						<circle cx="7" cy="7" r="2.6" class="hole" />
					{:else}
						<circle cx="7" cy="7" r="5.9" class="ring" />
						<circle cx="7" cy="7" r="2.6" class="fill" />
					{/if}
				</svg>
				{#if recording}<span>{format(t - START)}</span>{/if}
			</span>
			<span>Tue 9:41 AM</span>
		</div>

		<div class="keys">
			{#each ['⇧', '⌘', 'R'] as key, i (key)}
				<kbd class={{ down: press !== null && t >= press.at[i], r: key === 'R' }}>{key}</kbd>
			{/each}
		</div>
		<p class="hint">from any app</p>

		{#if menu > 0}
			<div class="menu" style:opacity={menu} style:transform="translateY({(1 - menu) * -4}px)">
				{#each items as group, g (g)}
					{#if g > 0}<i class="separator"></i>{/if}
					{#each group as item, i (item.label)}
						<div class={['item', { off: item.off, selected: highlight && g === 0 && i === 0 }]}>
							<span>{item.label}</span>
							{#if item.key}<span class="key">{item.key}</span>{/if}
						</div>
					{/each}
				{/each}
			</div>
		{/if}
	</Fit>
</div>

<style>
	.backdrop {
		position: absolute;
		inset: 0;
		background-color: #0c0b14;
		background-image:
			radial-gradient(circle farthest-corner at 10% 70%, rgba(98, 84, 220, 0.42), transparent 60%),
			radial-gradient(circle farthest-corner at 100% 100%, rgba(40, 150, 190, 0.32), transparent 60%),
			linear-gradient(160deg, #15132a, #0b0a14 55%, #0d1220);
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
		gap: 5px;
		height: 20px;
		padding: 0 7px;
		border-radius: 5px;
	}

	.status.open {
		background: rgba(255, 255, 255, 0.16);
	}

	.status.rec {
		color: #ff453a;
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

	.status .hole {
		fill: #1a1013;
	}

	.keys {
		position: absolute;
		left: 56px;
		top: 112px;
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
		left: 56px;
		top: 172px;
		margin: 0;
		color: rgba(244, 244, 245, 0.62);
		font-size: 12px;
	}

	.menu {
		position: absolute;
		top: 30px;
		right: 26px;
		width: 216px;
		padding: 5px;
		border-radius: 10px;
		background: rgba(36, 36, 42, 0.72);
		backdrop-filter: blur(24px) saturate(180%);
		box-shadow:
			inset 0 0.5px 0 rgba(255, 255, 255, 0.24),
			0 0 0 0.5px rgba(0, 0, 0, 0.6),
			0 18px 40px rgba(0, 0, 0, 0.5);
		color: #f4f4f5;
		font-size: 12.5px;
	}

	.item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 20px;
		padding: 0 9px;
		border-radius: 5px;
	}

	.item.off {
		color: rgba(244, 244, 245, 0.35);
	}

	.item.selected {
		background: #0a64d8;
	}

	.key {
		color: rgba(244, 244, 245, 0.55);
		letter-spacing: 0.04em;
	}

	.selected .key {
		color: rgba(255, 255, 255, 0.85);
	}

	.separator {
		display: block;
		height: 1px;
		margin: 4px 9px;
		background: rgba(255, 255, 255, 0.12);
	}
</style>

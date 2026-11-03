<script lang="ts">
	import { envelope } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Fit from './Fit.svelte';

	/**
	 * Cortex's four capture targets (rec-core CaptureTarget) on one desktop, each showing what ends
	 * up in the file. Decorative only.
	 */
	const MODES = [
		{ label: 'Display', note: 'The whole screen.' },
		{ label: 'Window', note: 'Just one window.' },
		{ label: 'App', note: 'Every window of one app, new ones too. The rest stays black.' },
		{ label: 'Area', note: 'A rectangle you drag, which snaps to window edges.' }
	];
	const STEP = 2600;

	const clock = loop(MODES.length * STEP, 3 * STEP + 1400);
	let t = $derived(clock.t);
	let i = $derived(Math.floor(t / STEP) % MODES.length);
	let mode = $derived(MODES[i]?.label ?? 'Area');
	let shown = $derived(envelope(t % STEP, 0, 300, STEP - 200, STEP));

	/** Window frames on the 450×260 desktop. Two belong to the same app (the browser). */
	const EDITOR = { x: 26, y: 30, w: 220, h: 170 };
	const BROWSER = { x: 214, y: 58, w: 210, h: 150 };
	const BROWSER_2 = { x: 286, y: 22, w: 140, h: 64 };
	const AREA = { x: 60, y: 54, w: 160, h: 90 };
</script>

{#snippet win(f: { x: number; y: number; w: number; h: number }, kind: string)}
	<div class="win {kind}" style:left="{f.x}px" style:top="{f.y}px" style:width="{f.w}px" style:height="{f.h}px">
		<div class="bar"><i></i><i></i><i></i></div>
		<div class="lines">{#each [80, 62, 90, 54, 72] as w, k (k)}<i style:width="{w}%"></i>{/each}</div>
	</div>
{/snippet}

<div class="card-visual app" {@attach clock.observe}>
	<Fit width={506} height={430}>
		<div class="tabs">
			<span class="thumb" style:transform="translateX({i * 100}%)"></span>
			{#each MODES as m, j (m.label)}
				<span class={{ on: j === i }}>{m.label}</span>
			{/each}
		</div>

		<div class="desktop" aria-hidden="true">
			{@render win(EDITOR, 'editor')}
			{@render win(BROWSER_2, 'browser')}
			{@render win(BROWSER, 'browser')}

			{#if mode === 'Display'}
				<div class="pick whole" style:opacity={shown}></div>
			{:else if mode === 'Window'}
				<div
					class="pick dim"
					style:opacity={shown}
					style:left="{EDITOR.x}px"
					style:top="{EDITOR.y}px"
					style:width="{EDITOR.w}px"
					style:height="{EDITOR.h}px"
				></div>
			{:else if mode === 'App'}
				<div class="black" style:opacity={shown}>
					{@render win(BROWSER_2, 'browser')}
					{@render win(BROWSER, 'browser')}
				</div>
			{:else}
				<div
					class="pick dim dashed"
					style:opacity={shown}
					style:left="{AREA.x}px"
					style:top="{AREA.y}px"
					style:width="{AREA.w}px"
					style:height="{AREA.h}px"
				>
					<span class="size">1280 × 720</span>
				</div>
			{/if}
		</div>

		<p class="note">{MODES[i]?.note}</p>
	</Fit>
</div>

<style>
	.card-visual {
		background: radial-gradient(ellipse 70% 60% at 50% 0%, hsl(188 60% 55% / 0.09), transparent);
		color: var(--fg);
	}

	.tabs {
		position: absolute;
		left: 103px;
		top: 30px;
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		width: 300px;
		height: 32px;
		padding: 2px;
		border-radius: 9px;
		background: var(--segments);
		color: var(--secondary);
		font-size: 12.5px;
	}

	.tabs span {
		position: relative;
		display: grid;
		place-items: center;
	}

	.tabs .on {
		color: var(--text);
		font-weight: 600;
	}

	.tabs .thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: calc((100% - 4px) / 4);
		height: 28px;
		border-radius: 7px;
		background: var(--segment);
		box-shadow: 0 1px 0 rgba(0, 0, 0, 0.18);
		transition: transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1);
	}

	.desktop {
		position: absolute;
		left: 28px;
		top: 86px;
		width: 450px;
		height: 260px;
		overflow: hidden;
		border-radius: 10px;
		background:
			radial-gradient(circle farthest-corner at 18% 16%, hsl(188 50% 40% / 0.55), transparent 58%),
			radial-gradient(circle farthest-corner at 96% 100%, hsl(196 45% 30% / 0.4), transparent 60%),
			linear-gradient(160deg, #12282d, #0a1215 55%, #0c1a1f);
		box-shadow:
			0 0 0 1px rgba(255, 255, 255, 0.08),
			0 20px 44px -16px rgba(0, 0, 0, 0.6);
	}

	.win {
		position: absolute;
		overflow: hidden;
		border-radius: 6px;
		box-shadow:
			0 0 0 0.5px rgba(255, 255, 255, 0.2),
			0 10px 24px rgba(0, 0, 0, 0.45);
	}

	.editor {
		background: #1d2024;
	}

	.browser {
		background: #f2f2f0;
	}

	.bar {
		display: flex;
		gap: 4px;
		height: 14px;
		padding: 4px 6px;
		background: rgba(0, 0, 0, 0.18);
	}

	.bar i {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.45);
	}

	.lines {
		display: flex;
		flex-direction: column;
		gap: 7px;
		padding: 10px;
	}

	.lines i {
		height: 4px;
		border-radius: 2px;
		background: rgba(255, 255, 255, 0.22);
	}

	.browser .lines i {
		background: #c9c9ce;
	}

	.pick {
		position: absolute;
		border-radius: 6px;
		box-shadow: inset 0 0 0 2px hsl(188 58% 58%);
		pointer-events: none;
	}

	.pick.whole {
		inset: 0;
		border-radius: 10px;
	}

	.pick.dim {
		box-shadow:
			inset 0 0 0 2px hsl(188 58% 58%),
			0 0 0 999px rgba(0, 0, 0, 0.55);
	}

	.pick.dashed {
		border-radius: 2px;
		outline: 1.5px dashed hsl(188 58% 70%);
		outline-offset: -1px;
		box-shadow: 0 0 0 999px rgba(0, 0, 0, 0.55);
	}

	.size {
		position: absolute;
		right: 4px;
		bottom: -22px;
		padding: 2px 6px;
		border-radius: 5px;
		background: hsl(188 58% 58%);
		color: hsl(188 60% 7%);
		font: 600 10px/14px var(--mono);
	}

	.black {
		position: absolute;
		inset: 0;
		background: #000;
	}

	.note {
		position: absolute;
		left: 28px;
		top: 366px;
		width: 450px;
		margin: 0;
		color: var(--muted);
		font-size: 13px;
		line-height: 18px;
		text-align: center;
	}
</style>

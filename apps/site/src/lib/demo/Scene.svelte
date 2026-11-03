<script lang="ts">
	import { easeInOut, easeOutBack, envelope, span } from '#lib/ease.js';
	import Cursor from '#lib/mock/Cursor.svelte';
	import Tracks from '#lib/mock/Tracks.svelte';
	import CortexWindow from './CortexWindow.svelte';
	import { CAPTIONS, CLICKS, CUT, DURATION, FILE, T, WINDOW, clock, cursorAt, recorded, status } from './script.js';

	let { t }: { t: number } = $props();

	let phase = $derived(status(t));
	let time = $derived(clock(recorded(t)));

	let cursor = $derived(cursorAt(t));
	let hand = $derived(t >= 7850 && t < T.drop + 300);
	let resizing = $derived(
		(t >= T.trimStart[0] - 150 && t < T.trimStart[1] + 150) || (t >= T.trimEnd[0] - 150 && t < T.trimEnd[1] + 150)
	);
	let clicking = $derived(CLICKS.some((c) => t >= c - 60 && t < c + 120));

	let keys = $derived(T.keys.find((k) => t >= k.show[0] && t < k.show[1] + 300) ?? null);
	let keysShown = $derived(keys ? envelope(t, keys.show[0], keys.show[0] + 200, keys.show[1], keys.show[1] + 260) : 0);
	const KEYS = ['⇧', '⌘', 'R'];

	let file = $derived(envelope(t, T.file[0], T.file[0] + 450, T.file[1], T.file[1] + 350));
	let lanes = $derived(easeInOut(span(t, T.file[0] + 250, T.file[0] + 1500)));
	let playhead = $derived(span(t, T.file[0] + 1500, T.trimStart[0] - 400));
	let trimming = $derived(t >= T.trimStart[0] - 400);
	let trim = $derived({
		start: CUT.start * easeInOut(span(t, T.trimStart[0], T.trimStart[1])),
		end: 1 - CUT.end * easeInOut(span(t, T.trimEnd[0], T.trimEnd[1]))
	});
	let exporting = $derived(span(t, T.save, T.exported));
	let exported = $derived(t >= T.exported);
	const LENGTH = clock(recorded(DURATION));
	const NAME = 'rec-2026-11-03-094112';

	let caption = $derived(CAPTIONS.find((c) => t >= c.start - 200 && t < c.end + 900) ?? null);
	let captionShown = $derived(caption ? envelope(t, caption.start - 200, caption.start, caption.end + 600, caption.end + 900) : 0);
</script>

<div class="scene">
	<div class="wallpaper"></div>

	<div class="menubar">
		<svg class="apple" viewBox="0 0 17 21" aria-hidden="true">
			<path
				d="M11.2 4.3c.6-.8 1-1.8.9-2.8-.9 0-2 .6-2.6 1.4-.6.7-1.1 1.7-.9 2.7 1 .1 2-.5 2.6-1.3zM12.1 5.8c-1.4-.1-2.7.8-3.3.8-.7 0-1.7-.8-2.9-.8C4.4 5.9 3 6.8 2.2 8.1c-1.6 2.7-.4 6.8 1.1 9 .8 1.1 1.7 2.3 2.8 2.3 1.1 0 1.6-.7 2.9-.7 1.4 0 1.8.7 2.9.7 1.2 0 2-1.1 2.7-2.2.9-1.3 1.2-2.5 1.2-2.6 0 0-2.4-.9-2.4-3.6 0-2.2 1.8-3.3 1.9-3.4-1-1.5-2.6-1.7-3.2-1.7z"
			/>
		</svg>
		<b>Cortex</b>
		<span>Window</span>
		<span class="spacer"></span>
		<span class="status">
			<svg viewBox="0 0 14 14" aria-hidden="true">
				<circle cx="7" cy="7" r="5.9" class="ring" />
				<circle cx="7" cy="7" r="2.6" class="fill" />
			</svg>
		</span>
		<span>Tue Nov 3&nbsp;&nbsp;9:41 AM</span>
	</div>

	<div class="window" style:left="{WINDOW.x}px" style:top="{WINDOW.y}px" style:width="{WINDOW.w}px" style:height="{WINDOW.h}px">
		<CortexWindow {t} />
	</div>

	{#if keys && keysShown > 0}
		<div class="keys glass" style:opacity={keysShown} style:transform="scale({0.94 + keysShown * 0.06})">
			{#each KEYS as key, i (key)}
				<span class={['key', { down: t >= keys.at[i] && t < keys.show[1] - 150, r: key === 'R' }]}>{key}</span>
			{/each}
		</div>
	{/if}

	{#if file > 0}
		<div
		class="file glass"
		style:left="{FILE.x}px"
		style:top="{FILE.y}px"
		style:width="{FILE.w}px"
		style:opacity={file} style:transform="translateY({(1 - easeOutBack(file)) * 24}px)">
			<div class="file-head">
				<svg width="30" height="37" viewBox="0 0 34 42" aria-hidden="true">
					<path d="M4 1h18l11 11v25a4 4 0 0 1-4 4H4a4 4 0 0 1-4-4V5a4 4 0 0 1 4-4z" class="doc" />
					<path d="M22 1v8a3 3 0 0 0 3 3h8" class="fold" />
					<path d="M13 19.5v10l8.5-5z" class="play" />
				</svg>
				<div>
					<b>{NAME}.mov</b>
					<span>{LENGTH} · 3840 × 2160 · 60 fps · 5 tracks</span>
				</div>
			</div>
			<Tracks
				dark
				camera
				ruler={['0:00', '0:02', '0:04', '0:06']}
				reveal={lanes}
				playhead={lanes >= 1 && !trimming ? playhead : null}
				trim={trimming ? trim : null}
			/>
			<div class={['trimbar', { shown: trimming }]}>
				<span class="modes"><span class="on">Fast (no re-encode)</span><span>Exact</span></span>
				<span class="spacer"></span>
				{#if exported}
					<span class="saved">
						<svg viewBox="0 0 14 14" aria-hidden="true"><path d="M3 7.4 5.8 10 11 4.2" /></svg>
						{NAME}-trim.mov
					</span>
				{:else}
					<span class={['save', { pressed: t >= T.save && t < T.save + 150 }]}>
						<i style:width="{exporting * 100}%"></i>
						<b>{exporting > 0 ? 'Saving…' : 'Save as copy'}</b>
					</span>
				{/if}
			</div>
		</div>
	{/if}

	<Cursor
		x={cursor.x}
		y={cursor.y}
		kind={resizing ? 'resize' : hand ? (t >= T.grab && t < T.drop ? 'grabbing' : 'grab') : 'arrow'}
		pressed={clicking}
	/>

	{#if caption && captionShown > 0}
		<div class="caption" style:opacity={captionShown} style:transform="translateY({(1 - captionShown) * 6}px)">
			{#each caption.words as w, i (i)}
				<span style:opacity={0.32 + 0.68 * span(t, w.at - 60, w.at + 140)}>{w.text}</span>
			{/each}
		</div>
	{/if}
</div>

<style>
	.scene {
		position: relative;
		width: 1200px;
		height: 760px;
		overflow: hidden;
		color: #f4f4f5;
		font-family: var(--sans);
		user-select: none;
	}

	.wallpaper {
		position: absolute;
		inset: 0;
		background-color: #0b1214;
		background-image:
			radial-gradient(circle farthest-corner at 18% 16%, hsl(188 55% 42% / 0.5), transparent 58%),
			radial-gradient(circle farthest-corner at 96% 100%, hsl(196 50% 30% / 0.4), transparent 60%),
			radial-gradient(circle farthest-corner at 70% 40%, hsl(176 40% 28% / 0.25), transparent 60%),
			linear-gradient(160deg, #10262b 0%, #0a1215 55%, #0c1a1f 100%);
	}

	.menubar {
		position: absolute;
		inset: 0 0 auto;
		display: flex;
		align-items: center;
		gap: 18px;
		height: 28px;
		padding: 0 18px;
		background: rgba(10, 10, 14, 0.32);
		backdrop-filter: blur(20px) saturate(180%);
		font-size: 13px;
	}

	.apple {
		width: 13px;
		height: 16px;
		fill: #f4f4f5;
	}

	.spacer {
		flex: 1;
	}

	.status {
		display: flex;
		align-items: center;
		gap: 5px;
		height: 22px;
		margin-right: -6px;
		padding: 0 7px;
		border-radius: 5px;
		font-variant-numeric: tabular-nums;
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

	.window {
		position: absolute;
	}

	.glass {
		background: rgba(28, 28, 34, 0.42);
		backdrop-filter: blur(22px) saturate(190%) brightness(106%);
		box-shadow:
			inset 0 0.5px 0 rgba(255, 255, 255, 0.32),
			inset 0 0 0 0.5px rgba(255, 255, 255, 0.14),
			0 24px 60px -12px rgba(0, 0, 0, 0.7);
	}

	.keys {
		position: absolute;
		left: 462px;
		top: 190px;
		display: flex;
		gap: 12px;
		padding: 18px;
		border-radius: 26px;
	}

	.key {
		display: grid;
		place-items: center;
		width: 72px;
		height: 72px;
		border-radius: 16px;
		background: linear-gradient(#3a3a3e, #2a2a2e);
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.14),
			0 3px 0 #101012,
			0 8px 20px rgba(0, 0, 0, 0.4);
		font-size: 28px;
		font-weight: 500;
		transition:
			translate 0.08s,
			box-shadow 0.08s;
	}

	.key.r {
		background: linear-gradient(#fff, #f2f2f6 55%, #e6e6ec);
		color: #1c1c1e;
		font-weight: 700;
	}

	.key.down {
		translate: 0 2px;
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.14),
			0 1px 0 #101012,
			0 4px 12px rgba(0, 0, 0, 0.4);
	}

	.file {
		position: absolute;
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 20px;
		border-radius: 22px;
		background: rgba(22, 24, 27, 0.9);
	}

	.trimbar {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 30px;
		font-size: 12px;
		opacity: 0;
		transition: opacity 0.3s;
	}

	.trimbar.shown {
		opacity: 1;
	}

	.trimbar .spacer {
		flex: 1;
	}

	.modes {
		display: flex;
		padding: 2px;
		border-radius: 8px;
		background: rgba(255, 255, 255, 0.06);
		color: rgba(244, 244, 245, 0.6);
	}

	.modes span {
		padding: 5px 10px;
		border-radius: 6px;
	}

	.modes .on {
		background: rgba(255, 255, 255, 0.12);
		color: #f4f4f5;
	}

	.save {
		position: relative;
		overflow: hidden;
		height: 30px;
		padding: 0 14px;
		border-radius: 8px;
		background: hsl(188 50% 30%);
		box-shadow: 0 2px 0 hsl(188 50% 20%);
		line-height: 30px;
		transition: filter 0.1s;
	}

	.save i {
		position: absolute;
		inset: 0 auto 0 0;
		background: hsl(188 58% 58%);
	}

	.save b {
		position: relative;
		font-weight: 600;
	}

	.save.pressed {
		filter: brightness(1.3);
	}

	.saved {
		display: flex;
		align-items: center;
		gap: 6px;
		color: hsl(188 58% 66%);
		font-weight: 600;
	}

	.saved svg {
		width: 14px;
		height: 14px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.file-head {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.file-head div {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.file-head b {
		font-size: 15px;
		font-weight: 600;
		line-height: 18px;
	}

	.file-head span {
		color: rgba(244, 244, 245, 0.6);
		font-size: 12px;
		line-height: 16px;
	}

	.doc {
		fill: #23272c;
		stroke: rgba(255, 255, 255, 0.18);
	}

	.fold {
		fill: #2f343a;
	}

	.play {
		fill: #f4f4f5;
	}

	.caption {
		position: absolute;
		left: 50%;
		bottom: 14px;
		display: flex;
		gap: 0 0.28em;
		padding: 9px 18px;
		border-radius: 14px;
		background: rgba(8, 8, 10, 0.62);
		backdrop-filter: blur(16px);
		color: #fff;
		font-size: 21px;
		font-weight: 600;
		letter-spacing: -0.01em;
		line-height: 28px;
		translate: -50% 0;
		white-space: nowrap;
	}
</style>

<script lang="ts">
	import { clamp, easeInOut, easeOut, easeOutBack, envelope, lerp, span } from '#lib/ease.js';
	import Person from '#lib/rec/Person.svelte';
	import ShapeIcon, { SHAPES } from '#lib/rec/ShapeIcon.svelte';
	import Slide from '#lib/rec/Slide.svelte';
	import {
		AREA,
		DISPLAY_H,
		DROP,
		GRAB,
		PIP,
		PIP_MARGIN,
		POPUP_ROWS,
		T,
		WINDOW,
		WINDOW_H,
		clock,
		cursorAt,
		recorded,
		status
	} from './script.js';

	let { t }: { t: number } = $props();

	const down = (at: number) => t >= at && t < at + 150;

	let phase = $derived(status(t));
	let live = $derived(phase === 'recording' || phase === 'paused');
	let time = $derived(clock(recorded(t)));
	let hasLast = $derived(t >= T.saving[1]);
	let cursor = $derived(cursorAt(t));

	// Chapter 1: pick a window from the popup, and the preview takes its shape.
	const WINDOWS = ['Entire display', 'Editor — main.rs', 'Notes — Episode 15', 'Slides — Episode 15'];
	let popup = $derived(envelope(t, T.popup[0], T.popup[0] + 160, T.popup[1], T.popup[1] + 120));
	let hovered = $derived(
		cursor.x > POPUP_ROWS.x && cursor.x < POPUP_ROWS.x + POPUP_ROWS.w
			? Math.floor((cursor.y - POPUP_ROWS.y) / POPUP_ROWS.h)
			: -1
	);
	let picked = $derived(t >= T.popup[1]);
	let source = $derived(easeInOut(span(t, T.source[0], T.source[1])));
	let frameH = $derived(lerp(DISPLAY_H, WINDOW_H, source));

	// Chapter 2: Screen + Cam, a new shape, and a drag to the other corner.
	let sceneAt = $derived(easeInOut(span(t, T.scene, T.scene + 220)));
	let cam = $derived(easeOut(span(t, T.cam[0], T.cam[1])));
	let camIn = $derived(easeOutBack(span(t, T.cam[0], T.cam[0] + 420)));
	let morph = $derived(easeInOut(span(t, T.shape[0], T.shape[1])));
	let shape = $derived(t >= T.shape[0] ? 'squircle' : 'rounded');
	let pipH = $derived(lerp((PIP * 9) / 16, PIP, morph));
	/** Rounded uses 60% of the short side's half for its corners; the squircle is drawn close to its superellipse. */
	let pipR = $derived(lerp(0.6 * ((PIP * 9) / 32), PIP * 0.34, morph));
	const START = { x: PIP_MARGIN, y: WINDOW_H - PIP_MARGIN - PIP };
	const END = { x: AREA.w - PIP_MARGIN - PIP, y: WINDOW_H - PIP_MARGIN - PIP };
	const LET_GO = { x: START.x + DROP.x - GRAB.x, y: START.y + DROP.y - GRAB.y };
	let pip = $derived.by(() => {
		if (t < T.grab) return { x: PIP_MARGIN, y: WINDOW_H - PIP_MARGIN - pipH };
		if (t < T.drop) return { x: START.x + cursor.x - GRAB.x, y: START.y + cursor.y - GRAB.y };
		const p = easeOut(span(t, T.snap[0], T.snap[1]));
		return { x: lerp(LET_GO.x, END.x, p), y: lerp(LET_GO.y, END.y, p) };
	});
	let dragging = $derived(t >= T.grab && t < T.drop);
	let hovering = $derived(t >= 7850 && t < T.drop + 300);
	let nearCorner = $derived(dragging && Math.hypot(pip.x - END.x, pip.y - END.y) < AREA.w * 0.05);
	let corner = $derived(t < T.grab ? 3 : t < T.drop ? -1 : 4);
	let showCam = $derived(t >= T.cam[0] && phase !== 'saving');

	// Meters move while a recording runs, from levels that repeat on every play.
	const db = (x: number) => clamp((x + 60) / 60);
	const system = (ms: number) => -16 + 5 * Math.sin(ms / 230) + 3 * Math.sin(ms / 71 + 1);
	const mic = (ms: number) => -28 + 12 * Math.max(0, Math.sin(ms / 380)) + 2 * Math.sin(ms / 53);
	const peak = (f: (ms: number) => number) => Math.max(f(t), f(t - 200), f(t - 400), f(t - 600)) + 1;
	let meters = $derived(
		live
			? [
					{ name: 'System', level: system(t), peak: peak(system) },
					{ name: 'Mic', level: mic(t), peak: peak(mic) }
				]
			: [
					{ name: 'System', level: -90, peak: -90 },
					{ name: 'Mic', level: -90, peak: -90 }
				]
	);
</script>

{#snippet chevrons()}
	<svg class="chevrons" width="8" height="10" viewBox="0 0 8 10" aria-hidden="true">
		<path d="M1 3.4 4 1.2l3 2.2M1 6.6l3 2.2 3-2.2" />
	</svg>
{/snippet}

{#snippet header(label: string, icon: string)}
	<div class="header">
		<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d={icon} /></svg>
		{label}
	</div>
{/snippet}

{#snippet switcher(on: boolean)}
	<span class={['switch', { on }]}><i></i></span>
{/snippet}

{#snippet slider(label: string, fill: number, value: string)}
	<div class="row slider">
		<span class="label">{label}</span>
		<span class="track"><i style:width="{fill * 100}%"></i><b style:left="{fill * 100}%"></b></span>
		<span class="value">{value}</span>
	</div>
{/snippet}

<div class="window app">
	<div class="toolbar">
		<div class="lights"><i></i><i></i><i></i></div>
		<b>Rec</b>
		{#if phase === 'saving'}
			<span class="pill quiet">SAVING…</span>
		{:else if phase === 'paused'}
			<span class="pill paused">PAUSED&nbsp;&nbsp;{time}</span>
		{:else if phase === 'recording'}
			<span class="pill rec">●&nbsp;REC&nbsp;&nbsp;{time}</span>
		{:else}
			<span class="pill quiet">PREVIEW</span>
		{/if}
		<span class="spacer"></span>
		<span class="button">Settings…</span>
	</div>

	<div class="body">
		<div class="area">
			<div
				class={['frame', { rec: phase === 'recording', paused: phase === 'paused', saving: phase === 'saving' }]}
				style:height="{frameH}px"
			>
				{#if phase === 'saving'}
					<span class="placeholder">Saving…</span>
				{:else}
					{#if source < 1}
						<div class="desktop" style:opacity={1 - source}>
							<div class="desk-bar"></div>
							<div class="desk-window slides">
								<div class="desk-title"></div>
								<div class="desk-slide"><Slide /></div>
							</div>
							<div class="desk-window notes">
								<div class="desk-title"></div>
								<div class="note-lines">
									{#each [86, 70, 92, 58, 80, 64, 88, 50] as w, i (i)}<i style:width="{w}%"></i>{/each}
								</div>
							</div>
							<div class="dock">{#each { length: 7 }, i (i)}<i></i>{/each}</div>
						</div>
					{/if}
					{#if source > 0}
						<div class="shot" style:opacity={source}><Slide /></div>
					{/if}
					{#if showCam}
						{#if nearCorner}
							<div class="ghost" style:left="{END.x}px" style:top="{END.y}px"></div>
						{/if}
						<div
							class="pip"
							style:left="{pip.x}px"
							style:top="{pip.y}px"
							style:width="{PIP}px"
							style:height="{pipH}px"
							style:border-radius="{pipR}px"
							style:transform="scale({camIn})"
						>
							<Person />
						</div>
						{#if hovering}
							<div
								class="outline"
								style:left="{pip.x}px"
								style:top="{pip.y}px"
								style:width="{PIP}px"
								style:height="{pipH}px"
							></div>
						{/if}
					{/if}
				{/if}
			</div>
		</div>

		<div class="sidebar">
			<section>
				{@render header('SCENE', 'M3.5 4.5h7a1.5 1.5 0 0 1 1.5 1.5v5a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 2 11V6a1.5 1.5 0 0 1 1.5-1.5zM4.5 2h6')}
				<div class="segments">
					<span class={['thumb', { pressed: down(T.scene) }]} style:transform="translateX({sceneAt * 100}%)"></span>
					{#each ['Screen', 'Screen + Cam', 'Camera'] as label, i (label)}
						<span class={{ on: Math.round(sceneAt) === i }}>{label}</span>
					{/each}
				</div>
			</section>

			<section>
				{@render header('SCREEN', 'M2.5 2h9A1.5 1.5 0 0 1 13 3.5v5A1.5 1.5 0 0 1 11.5 10h-9A1.5 1.5 0 0 1 1 8.5v-5A1.5 1.5 0 0 1 2.5 2zM7 10v2.5M4.5 12.5h5')}
				<div class="group">
					<div class="combo"><span>Display 1 — 5120×2880</span>{@render chevrons()}</div>
					<div class="pair">
						<div class={['combo', { pressed: popup > 0 }]}>
							<span>{picked ? WINDOWS[3] : WINDOWS[0]}</span>{@render chevrons()}
						</div>
						<div class="button square">↻</div>
					</div>
				</div>
			</section>

			<section>
				{@render header('CAMERA', 'M2.5 4h5A1.5 1.5 0 0 1 9 5.5v4A1.5 1.5 0 0 1 7.5 11h-5A1.5 1.5 0 0 1 1 9.5v-4A1.5 1.5 0 0 1 2.5 4zM9.5 7.5 13 5v5z')}
				<div class="group">
					<div class="combo" style:opacity={lerp(0.45, 1, cam)}><span>Default camera</span>{@render chevrons()}</div>
					<div class="grow" style:grid-template-rows="{1 - cam}fr">
						<div><p class="hint">Pick a camera scene to use the webcam.</p></div>
					</div>
				</div>
			</section>

			<div class="grow" style:grid-template-rows="{cam}fr">
				<section>
					{@render header('CAMERA OVERLAY', 'M2.5 2h9A1.5 1.5 0 0 1 13 3.5v7a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 1 10.5v-7A1.5 1.5 0 0 1 2.5 2zM9.5 6.5a2 2 0 1 1 0 4 2 2 0 0 1 0-4z')}
					<div class="group overlay">
						<div class="shapes">
							{#each SHAPES as s (s)}
								<span class={{ on: s === shape, pressed: s === 'squircle' && down(T.shape[0]) }}><ShapeIcon shape={s} /></span>
							{/each}
						</div>
						{@render slider('Size', 0.3, '22%')}
						{@render slider('Roundness', 0.6, '60%')}
						<div class="row slider">
							<span class="label">Border</span>
							<span class="track"><i style:width="15%"></i><b style:left="15%"></b></span>
							<span class="value">3 px</span>
							<span class="swatch"></span>
						</div>
						<div class="row corners">
							<span class="label">Position</span>
							{#each ['↖', '↗', '↙', '↘'] as arrow, i (arrow)}
								<span class={{ on: corner === i + 1 }}>{arrow}</span>
							{/each}
							{#if corner < 0}<span class="custom">custom</span>{/if}
						</div>
						<div class="divider"></div>
						<div class="row"><span>Shadow</span>{@render switcher(true)}</div>
						<div class="row"><span>Mirror</span>{@render switcher(true)}</div>
					</div>
				</section>
			</div>

			<section>
				{@render header('AUDIO', 'M2 5.5h2L7 3v8L4 8.5H2zM9.5 5a3 3 0 0 1 0 4M11 3.5a5 5 0 0 1 0 7')}
				<div class="group">
					<div class="row"><span>System audio</span>{@render switcher(true)}</div>
					<div class="divider"></div>
					<div class="row"><span>Microphone</span>{@render switcher(true)}</div>
					<div class="combo"><span>Default microphone</span>{@render chevrons()}</div>
				</div>
			</section>

			<section>
				{@render header('OUTPUT', 'M1.5 4A1.5 1.5 0 0 1 3 2.5h2.5L7 4h4A1.5 1.5 0 0 1 12.5 5.5v5A1.5 1.5 0 0 1 11 12H3a1.5 1.5 0 0 1-1.5-1.5z')}
				<div class="group">
					<div class="row"><span>Format</span><span class="value">4K · 60 fps</span></div>
					<div class="divider"></div>
					<div class="row"><span>Codec</span><span class="value">HEVC · .mov</span></div>
					<div class="divider"></div>
					<div class="row"><span>Folder</span><span class="value">~/Movies</span></div>
				</div>
			</section>
		</div>
	</div>

	<div class="controls">
		<span class="record">
			{#if phase === 'saving'}
				<i class="finishing"></i>
			{:else if live}
				<i class="stop"></i>
			{:else}
				<i class="start"></i>
			{/if}
		</span>
		<span class={['button pause', { off: !live, pressed: down(T.recording[0][1]) || down(T.recording[1][0]) }]}>
			{phase === 'paused' ? 'Resume' : 'Pause'}
		</span>
		<span class={['clock', phase]}>{live ? time : '00:00'}</span>
		<div class="meters">
			{#each meters as m (m.name)}
				<div class="meter">
					<span class="meter-name">{m.name}</span>
					<span class="meter-bar">
						<i class="green" style:width="{Math.min(db(m.level), db(-18)) * 100}%"></i>
						{#if m.level > -18}
							<i class="yellow" style:left="{db(-18) * 100}%" style:width="{(Math.min(db(m.level), db(-6)) - db(-18)) * 100}%"></i>
						{/if}
						{#if m.level > -6}
							<i class="red" style:left="{db(-6) * 100}%" style:width="{(db(m.level) - db(-6)) * 100}%"></i>
						{/if}
						{#if m.peak > -60}
							<b style:left="{db(m.peak) * 100}%"></b>
						{/if}
					</span>
					<span class="meter-db">{m.level <= -89 ? '-∞' : Math.round(m.level)}</span>
				</div>
			{/each}
		</div>
		<span class="spacer"></span>
		{#if hasLast}
			<span class={['button', { pressed: down(T.file[0]) }]}>Reveal Last</span>
		{/if}
		<span class="button">Open Folder</span>
	</div>

	{#if popup > 0}
		<div
			class="popup"
			style:left="{POPUP_ROWS.x - WINDOW.x - 4}px"
			style:top="{POPUP_ROWS.y - WINDOW.y - 4}px"
			style:width="{POPUP_ROWS.w + 8}px"
			style:opacity={popup}
			style:transform="translateY({(1 - popup) * -4}px)"
		>
			{#each WINDOWS as label, i (label)}
				<span class={{ hovered: i === hovered && t < T.popup[1] + 60, current: i === 0 }}>{label}</span>
			{/each}
		</div>
	{/if}
</div>

<style>
	.window {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border-radius: 12px;
		background: var(--window);
		box-shadow:
			0 0 0 0.5px rgba(255, 255, 255, 0.18),
			0 30px 80px -10px rgba(0, 0, 0, 0.7);
		color: var(--text);
		font: 13px/16px var(--sans);
	}

	:global(:root[data-theme='light']) .window {
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.22),
			0 30px 80px -10px rgba(0, 0, 0, 0.45);
	}

	.toolbar {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 6px;
		height: 40px;
		padding: 0 12px 0 14px;
		border-bottom: 1px solid var(--separator);
	}

	.lights {
		display: flex;
		gap: 8px;
		width: 68px;
	}

	.lights i {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ff5f57;
	}

	.lights i:nth-child(2) {
		background: #febc2e;
	}

	.lights i:nth-child(3) {
		background: #28c840;
	}

	.toolbar b {
		margin-right: 4px;
		font-weight: 600;
	}

	.pill {
		padding: 3px 8px;
		border-radius: 9px;
		background: var(--rec);
		color: #fff;
		font-size: 11px;
		font-weight: 600;
		line-height: 13px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.pill.paused {
		background: var(--paused);
	}

	.pill.quiet {
		background: var(--control);
		color: var(--secondary);
	}

	.spacer {
		flex: 1;
	}

	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		height: 22px;
		padding: 0 9px;
		border-radius: 6px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
		white-space: nowrap;
		transition: filter 0.1s;
	}

	.pressed {
		filter: brightness(0.82);
	}

	:global(:root:not([data-theme='light'])) .pressed {
		filter: brightness(1.35);
	}

	.body {
		display: flex;
		flex: 1;
		min-height: 0;
	}

	.area {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 696px;
		flex-shrink: 0;
		padding: 16px;
	}

	.frame {
		position: relative;
		width: 664px;
		flex-shrink: 0;
		background: #111;
		box-shadow: 0 0 0 0.5px var(--separator);
		transition: box-shadow 0.15s;
	}

	.frame.rec {
		box-shadow: 0 0 0 2px var(--rec);
	}

	.frame.paused {
		box-shadow: 0 0 0 2px var(--paused);
	}

	.frame.saving {
		display: grid;
		place-items: center;
		border-radius: 10px;
		background: var(--control);
	}

	.placeholder {
		color: #808080;
	}

	.shot,
	.desktop {
		position: absolute;
		inset: 0;
		overflow: hidden;
	}

	.desktop {
		background:
			radial-gradient(circle farthest-corner at 22% 18%, rgba(98, 84, 220, 0.4), transparent 58%),
			radial-gradient(circle farthest-corner at 96% 100%, rgba(40, 150, 190, 0.3), transparent 60%),
			linear-gradient(160deg, #15132a, #0b0a14 55%, #0d1220);
	}

	.desk-bar {
		height: 9px;
		background: rgba(10, 10, 14, 0.4);
	}

	.desk-window {
		position: absolute;
		overflow: hidden;
		border-radius: 5px;
		box-shadow:
			0 0 0 0.5px rgba(255, 255, 255, 0.2),
			0 10px 24px rgba(0, 0, 0, 0.5);
	}

	.desk-title {
		height: 11px;
		background: #2b2b2d;
	}

	.slides {
		left: 34px;
		top: 40px;
		width: 380px;
		height: 249px;
		background: #fbfbfa;
	}

	.desk-slide {
		width: 664px;
		transform: scale(0.5723);
		transform-origin: 0 0;
	}

	.notes {
		left: 438px;
		top: 64px;
		width: 190px;
		height: 214px;
		background: #f2f2f0;
	}

	.note-lines {
		display: flex;
		flex-direction: column;
		gap: 9px;
		padding: 14px 12px;
	}

	.note-lines i {
		height: 4px;
		border-radius: 2px;
		background: #c9c9ce;
	}

	.note-lines i:first-child {
		height: 7px;
		background: #48484c;
	}

	.dock {
		position: absolute;
		left: 50%;
		bottom: 6px;
		display: flex;
		gap: 5px;
		padding: 4px 6px;
		border-radius: 7px;
		background: rgba(255, 255, 255, 0.16);
		translate: -50% 0;
	}

	.dock i {
		width: 14px;
		height: 14px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.55);
	}

	.pip {
		position: absolute;
		overflow: hidden;
		border: 3px solid #fff;
		box-shadow: 0 10px 26px rgba(0, 0, 0, 0.35);
		transform-origin: 0 100%;
	}

	.outline,
	.ghost {
		position: absolute;
		border-radius: 4px;
		pointer-events: none;
	}

	.outline {
		box-shadow: 0 0 0 1.5px var(--accent);
	}

	.ghost {
		width: 146px;
		height: 146px;
		box-shadow: 0 0 0 1px rgba(0, 122, 255, 0.7);
	}

	.sidebar {
		display: flex;
		flex-direction: column;
		width: 304px;
		flex-shrink: 0;
		padding: 0 16px;
		overflow: hidden;
		border-left: 1px solid var(--separator);
		background: var(--sidebar);
	}

	section {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding-top: 16px;
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

	.header svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.3;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.segments {
		position: relative;
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		height: 28px;
		padding: 2px;
		border-radius: 8px;
		background: var(--segments);
		color: var(--secondary);
		font-size: 12px;
		font-weight: 500;
	}

	.segments span {
		position: relative;
		display: grid;
		place-items: center;
	}

	.segments .on {
		color: var(--text);
		font-weight: 600;
	}

	.segments .thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: calc((100% - 4px) / 3);
		height: 24px;
		border-radius: 6px;
		background: var(--segment);
		box-shadow: 0 0.5px 0 rgba(0, 0, 0, 0.24);
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

	.group.overlay {
		padding-top: 10px;
	}

	.combo {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		flex: 1;
		min-width: 0;
		height: 24px;
		padding: 0 8px;
		border-radius: 6px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
	}

	.combo span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevrons {
		flex-shrink: 0;
		fill: none;
		stroke: var(--chevrons);
		stroke-width: 1.3;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.pair {
		display: flex;
		gap: 6px;
	}

	.button.square {
		width: 26px;
		height: 24px;
		padding: 0;
	}

	.grow {
		display: grid;
	}

	.grow > :global(*) {
		min-height: 0;
		overflow: hidden;
	}

	.hint {
		margin: 0;
		padding-top: 2px;
		color: var(--secondary);
		font-size: 11px;
		line-height: 14px;
	}

	.shapes {
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		gap: 6px;
		padding-bottom: 2px;
	}

	.shapes span {
		display: grid;
		place-items: center;
		height: 32px;
		border-radius: 7px;
		box-shadow: inset 0 0 0 1px var(--control-edge);
		color: var(--secondary);
	}

	.shapes .on {
		background: rgba(0, 122, 255, 0.18);
		box-shadow: inset 0 0 0 1.5px var(--accent);
		color: #2d8cff;
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 28px;
	}

	.slider,
	.corners {
		justify-content: flex-start;
		gap: 8px;
		color: var(--secondary);
		font-size: 12px;
	}

	.label {
		width: 72px;
		flex-shrink: 0;
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
		flex-shrink: 0;
		color: var(--secondary);
		text-align: right;
	}

	.slider .value {
		width: 36px;
	}

	.swatch {
		width: 22px;
		height: 18px;
		flex-shrink: 0;
		border-radius: 4px;
		background: #fff;
		box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.2);
	}

	.corners {
		gap: 4px;
	}

	.corners .label {
		width: 76px;
	}

	.corners span:not(.label, .custom) {
		display: grid;
		place-items: center;
		width: 28px;
		height: 24px;
		border-radius: 6px;
		background: var(--control);
		color: var(--text);
		font-size: 13px;
	}

	.corners .on:not(.label) {
		background: var(--accent);
		color: #fff;
	}

	.custom {
		margin-left: 4px;
		font-size: 11px;
	}

	.divider {
		height: 1px;
		margin: 0 0 -1px;
		background: color-mix(in srgb, var(--separator) 70%, transparent);
	}

	.switch {
		position: relative;
		width: 30px;
		height: 18px;
		border-radius: 9px;
		background: var(--switch-off);
	}

	.switch.on {
		background: var(--accent);
	}

	.switch i {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: #fff;
		box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.3);
	}

	.switch.on i {
		left: 14px;
	}

	.controls {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 8px;
		height: 68px;
		padding: 10px 16px;
		border-top: 1px solid var(--separator);
	}

	.record {
		display: grid;
		place-items: center;
		width: 44px;
		height: 44px;
		margin-right: -2px;
		border-radius: 50%;
		box-shadow: inset 0 0 0 3px var(--secondary);
	}

	.record i {
		display: block;
		background: var(--rec);
	}

	.start {
		width: 32px;
		height: 32px;
		border-radius: 50%;
	}

	.stop {
		width: 18px;
		height: 18px;
		border-radius: 4px;
	}

	.record .finishing {
		width: 30px;
		height: 30px;
		border-radius: 50%;
		background: var(--tertiary);
	}

	.pause {
		width: 72px;
		height: 26px;
		margin-right: 4px;
	}

	.pause.off {
		color: var(--tertiary);
	}

	.clock {
		width: 66px;
		color: var(--tertiary);
		font: 22px/26px var(--mono);
		font-variant-numeric: tabular-nums;
	}

	.clock.recording {
		color: var(--rec);
	}

	.clock.paused {
		color: var(--secondary);
	}

	.meters {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin-left: 8px;
		color: var(--secondary);
		font-size: 11px;
		line-height: 14px;
	}

	.meter {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.meter-name {
		width: 44px;
		text-align: center;
	}

	.meter-bar {
		position: relative;
		width: 220px;
		height: 8px;
		overflow: hidden;
		border-radius: 3px;
		background: var(--control);
	}

	:global(:root[data-theme='light']) .meter-bar {
		background: #e3e3e6;
	}

	.meter-bar i {
		position: absolute;
		top: 0;
		bottom: 0;
		left: 0;
		border-radius: 2px;
		background: #34c759;
	}

	.meter-bar .yellow {
		background: #fc0;
	}

	.meter-bar .red {
		background: var(--rec);
	}

	.meter-bar b {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 2px;
		margin-left: -1px;
		background: var(--text);
	}

	.meter-db {
		width: 20px;
	}

	.popup {
		position: absolute;
		display: flex;
		flex-direction: column;
		padding: 4px;
		border-radius: 8px;
		background: var(--popover);
		box-shadow:
			0 0 0 0.5px var(--control-edge),
			0 12px 32px var(--drop);
	}

	.popup span {
		height: 22px;
		padding: 0 8px;
		border-radius: 5px;
		line-height: 22px;
		white-space: nowrap;
	}

	.popup .current {
		background: color-mix(in srgb, var(--accent) 22%, transparent);
	}

	.popup .hovered {
		background: var(--accent);
		color: #fff;
	}
</style>

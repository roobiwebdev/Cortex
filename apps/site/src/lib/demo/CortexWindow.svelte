<script lang="ts">
	import { clamp, easeInOut, easeOut, easeOutBack, lerp, span } from '#lib/ease.js';
	import Person from '#lib/mock/Person.svelte';
	import ShapeIcon, { SHAPES } from '#lib/mock/ShapeIcon.svelte';
	import Slide from '#lib/mock/Slide.svelte';
	import {
		AREA_W,
		DISPLAY_W,
		DROP,
		FRAME_CX,
		FRAME_H,
		FRAME_Y,
		GRAB,
		MARQUEE_A,
		PIP,
		PIP_MARGIN,
		SLIDES,
		T,
		WINDOW,
		clock,
		cursorAt,
		recorded,
		status
	} from './script.js';

	/**
	 * The Cortex main window at time `t` of the demo: the sources sidebar (capture, scene, camera),
	 * the preview with its output chips, the audio mixer and the record bar
	 * (apps/desktop/src/surfaces/main-window). Drawn in HTML and CSS, not a screenshot.
	 */
	let { t }: { t: number } = $props();

	const down = (at: number) => t >= at && t < at + 150;

	let phase = $derived(status(t));
	let live = $derived(phase === 'recording' || phase === 'paused');
	let time = $derived(clock(recorded(t)));
	let cursor = $derived(cursorAt(t));

	// Chapter 1: the Area tab, then a rectangle dragged over the slides window, and the preview takes its shape.
	let areaMode = $derived(t >= T.areaTab);
	let marqueeOn = $derived(t >= T.marquee[0] && t < T.source[1]);
	let marquee = $derived.by(() => {
		const end = t < T.marquee[1] ? cursor : { x: MARQUEE_A.x + SLIDES.w, y: MARQUEE_A.y + SLIDES.h };
		return {
			x: SLIDES.x,
			y: SLIDES.y,
			w: Math.max(0, end.x - MARQUEE_A.x),
			h: Math.max(0, end.y - MARQUEE_A.y)
		};
	});
	let picked = $derived(t >= T.marquee[1]);
	let source = $derived(easeInOut(span(t, T.source[0], T.source[1])));
	let frameW = $derived(lerp(DISPLAY_W, AREA_W, source));

	// Chapter 2: Screen + Cam, a new shape, and a drag to the other corner.
	let sceneAt = $derived(easeInOut(span(t, T.scene, T.scene + 220)));
	let cam = $derived(easeOut(span(t, T.cam[0], T.cam[1])));
	let camIn = $derived(easeOutBack(span(t, T.cam[0], T.cam[0] + 420)));
	let morph = $derived(easeInOut(span(t, T.shape[0], T.shape[1])));
	let shape = $derived(t >= T.shape[0] ? 'circle' : 'squircle');
	/** The squircle at Cortex's default 35% roundness, then a circle. */
	let pipR = $derived(lerp(PIP * 0.3, PIP / 2, morph));
	const START = { x: AREA_W - PIP_MARGIN - PIP, y: FRAME_H - PIP_MARGIN - PIP };
	const END = { x: PIP_MARGIN, y: FRAME_H - PIP_MARGIN - PIP };
	const LET_GO = { x: START.x + DROP.x - GRAB.x, y: START.y + DROP.y - GRAB.y };
	let pip = $derived.by(() => {
		if (t < T.grab) return START;
		if (t < T.drop) return { x: START.x + cursor.x - GRAB.x, y: START.y + cursor.y - GRAB.y };
		const p = easeOut(span(t, T.snap[0], T.snap[1]));
		return { x: lerp(LET_GO.x, END.x, p), y: lerp(LET_GO.y, END.y, p) };
	});
	let dragging = $derived(t >= T.grab && t < T.drop);
	let hovering = $derived(t >= 7850 && t < T.drop + 300);
	let nearCorner = $derived(dragging && Math.hypot(pip.x - END.x, pip.y - END.y) < AREA_W * 0.06);
	let showCam = $derived(t >= T.cam[0]);

	// The mixer's meters move while a recording runs, from levels that repeat on every play.
	const SEGMENTS = 24;
	const system = (ms: number) => -16 + 5 * Math.sin(ms / 230) + 3 * Math.sin(ms / 71 + 1);
	const mic = (ms: number) => -28 + 12 * Math.max(0, Math.sin(ms / 380)) + 2 * Math.sin(ms / 53);
	const lit = (db: number) => Math.round(clamp((db + 60) / 60) * SEGMENTS);
	let meters = $derived([
		{ name: 'System audio', detail: 'All sounds from this Mac', lit: live ? lit(system(t)) : 0 },
		{ name: 'Microphone', detail: 'MacBook Pro Microphone', lit: live ? lit(mic(t)) : 0, select: true }
	]);
	const segment = (i: number) => (i >= SEGMENTS - 3 ? 'high' : i >= SEGMENTS - 8 ? 'mid' : 'low');
</script>

{#snippet chevrons()}
	<svg class="chevrons" width="8" height="10" viewBox="0 0 8 10" aria-hidden="true">
		<path d="M1 3.4 4 1.2l3 2.2M1 6.6l3 2.2 3-2.2" />
	</svg>
{/snippet}

{#snippet header(label: string, top: number)}
	<div class="header" style:top="{top}px">
		{label}
		<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M2 3.5 5 6.5l3-3" /></svg>
	</div>
{/snippet}

{#snippet switcher(on: boolean)}
	<span class={['switch', { on }]}><i></i></span>
{/snippet}

{#snippet slider(label: string, value: string, fill: number, top: number)}
	<div class="slider" style:top="{top}px">
		<div class="slider-label"><span>{label}</span><span class="mono">{value}</span></div>
		<span class="track"><i style:width="{fill * 100}%"></i><b style:left="{fill * 100}%"></b></span>
	</div>
{/snippet}

{#snippet monitor()}
	<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
		<rect x="2" y="3" width="12" height="8" rx="1.5" />
		<path d="M6 13.5h4M8 11v2.5" />
	</svg>
{/snippet}

<div class="window app">
	<aside class="sidebar">
		<div class="lights"><i></i><i></i><i></i></div>

		{@render header('CAPTURE', 50)}
		<div class="segments four" style:top="72px">
			<span class="thumb" style:transform="translateX({(areaMode ? 3 : 0) * 100}%)"></span>
			{#each ['Display', 'Window', 'App', 'Area'] as label, i (label)}
				<span class={{ on: (areaMode ? 3 : 0) === i, pressed: i === 3 && down(T.areaTab) }}>{label}</span>
			{/each}
		</div>
		<div class="capture" style:top="108px">
			{#if areaMode}
				<div class="area-box">
					<span class="area-icon">
						<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
							<rect x="1.5" y="1.5" width="11" height="11" rx="1" stroke-dasharray="2 2" />
						</svg>
					</span>
					<p>
						{#if picked}
							Slides area on Built-in Retina Display. Drag again to change it.
						{:else}
							Record part of a display. Drag a rectangle, or pick a window to snap to it.
						{/if}
					</p>
					<span class={['wide-button', { pressed: down(T.selectArea) }]}>
						{picked ? 'Change area…' : 'Select area…'}
					</span>
				</div>
			{:else}
				<div class="display on">
					<span class="display-icon">{@render monitor()}</span>
					<div><b>Built-in Retina Display</b><span>Display 1 · 3456 × 2234</span></div>
					<svg class="check" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M3 7.4 5.8 10 11 4.2" /></svg>
				</div>
				<div class="display">
					<span class="display-icon">{@render monitor()}</span>
					<div><b>Studio Display</b><span>Display 2 · 5120 × 2880</span></div>
				</div>
			{/if}
		</div>

		<i class="rule" style:top="232px"></i>
		{@render header('SCENE', 244)}
		<div class="segments three" style:top="260px">
			<span class={['thumb', { pressed: down(T.scene) }]} style:transform="translateX({sceneAt * 100}%)"></span>
			{#each ['Screen', 'Screen + Cam', 'Camera'] as label, i (label)}
				<span class={{ on: Math.round(sceneAt) === i }}>{label}</span>
			{/each}
		</div>

		<i class="rule" style:top="300px"></i>
		{@render header('CAMERA', 312)}
		<div class="camera" style:opacity={lerp(0.4, 1, cam)}>
			<div class="combo" style:top="330px"><span>FaceTime HD Camera</span>{@render chevrons()}</div>
			<div class="overline" style:top="370px">OVERLAY</div>
			<div class="shapes" style:top="382px">
				{#each SHAPES as s (s)}
					<span class={{ on: showCam && s === shape, pressed: s === 'circle' && down(T.shape[0]) }}><ShapeIcon shape={s} /></span>
				{/each}
			</div>
			{@render slider('Size', '22%', 0.3, 424)}
			{@render slider('Roundness', '35%', 0.35, 462)}
			<div class="row" style:top="502px"><span>Shadow</span>{@render switcher(true)}</div>
			<div class="row" style:top="532px"><span>Mirror</span>{@render switcher(true)}</div>
			<div class="row" style:top="562px">
				<span>Save camera as separate track</span>{@render switcher(true)}
			</div>
		</div>
	</aside>

	<div class="main">
		<div class="titlebar">
			<b>Cortex</b>
			{#if live}
				<span class={['title-time', phase]}><i></i>{time}</span>
			{:else if phase === 'saving'}
				<span class="title-time saving">Saving…</span>
			{/if}
			<span class="tools">
				<svg width="15" height="15" viewBox="0 0 16 16" aria-hidden="true">
					<circle cx="4" cy="4.5" r="2" /><circle cx="4" cy="11.5" r="2" />
					<path d="M5.6 5.6 13.5 12.5M5.6 10.4 13.5 3.5" />
				</svg>
				<svg width="15" height="15" viewBox="0 0 16 16" aria-hidden="true">
					<circle cx="8" cy="8" r="2.2" />
					<path d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1" />
				</svg>
			</span>
		</div>

		<div class="stage">
			<div
				class={['frame', { rec: phase === 'recording', paused: phase === 'paused' }]}
				style:left="{FRAME_CX - frameW / 2 - 288}px"
				style:top="{FRAME_Y - 52}px"
				style:width="{frameW}px"
				style:height="{FRAME_H}px"
			>
				{#if source < 1}
					<div class="desktop" style:opacity={1 - source}>
						<div class="desk-bar"></div>
						<div
							class="desk-window slides"
							style:left="{SLIDES.x}px"
							style:top="{SLIDES.y}px"
							style:width="{SLIDES.w}px"
							style:height="{SLIDES.h}px"
						>
							<div class="desk-title"></div>
							<div class="desk-slide" style:transform="scale({SLIDES.w / 664})"><Slide /></div>
						</div>
						<div class="desk-window notes">
							<div class="desk-title"></div>
							<div class="note-lines">
								{#each [86, 70, 92, 58, 80, 64, 88, 50] as w, i (i)}<i style:width="{w}%"></i>{/each}
							</div>
						</div>
						<div class="dock">{#each { length: 7 }, i (i)}<i></i>{/each}</div>
						{#if marqueeOn}
							<div
								class="marquee"
								style:left="{marquee.x}px"
								style:top="{marquee.y}px"
								style:width="{marquee.w}px"
								style:height="{marquee.h}px"
							></div>
						{/if}
					</div>
				{/if}
				{#if source > 0}
					<div class="shot" style:opacity={source}>
						<div style:transform="scale({AREA_W / 664})" class="shot-slide"><Slide /></div>
					</div>
				{/if}
				{#if showCam}
					{#if nearCorner}
						<div class="ghost" style:left="{END.x}px" style:top="{END.y}px" style:width="{PIP}px" style:height="{PIP}px"></div>
					{/if}
					<div
						class="pip"
						style:left="{pip.x}px"
						style:top="{pip.y}px"
						style:width="{PIP}px"
						style:height="{PIP}px"
						style:border-radius="{pipR}px"
						style:transform="scale({camIn})"
					>
						<Person />
					</div>
					{#if hovering}
						<div class="outline" style:left="{pip.x}px" style:top="{pip.y}px" style:width="{PIP}px" style:height="{PIP}px"></div>
					{/if}
				{/if}
				{#if live}
					<span class={['badge', phase]}><i></i>{phase === 'paused' ? 'PAUSED' : 'REC'}</span>
				{/if}
			</div>
		</div>

		<div class="chips">
			<span>4K · 60 fps · HEVC · MOV</span>
			{#if showCam}
				<span class="chip-cam" style:opacity={cam}>
					<svg width="11" height="11" viewBox="0 0 12 12" aria-hidden="true"><rect x="1.5" y="1.5" width="9" height="9" rx="2" /><circle cx="6" cy="5" r="1.6" /><path d="M3.4 9.6a2.8 2.8 0 0 1 5.2 0" /></svg>
					Camera track
				</span>
			{/if}
		</div>

		<div class="mixer">
			{#each meters as m (m.name)}
				<div class="strip">
					<span class="strip-icon">
						<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
							{#if m.select}
								<rect x="5" y="1.5" width="4" height="7" rx="2" /><path d="M3 7a4 4 0 0 0 8 0M7 11v1.5" />
							{:else}
								<rect x="2.5" y="1.5" width="9" height="11" rx="2" /><circle cx="7" cy="8" r="2" />
							{/if}
						</svg>
					</span>
					<span class="strip-name">{m.name}</span>
					{#if m.select}
						<span class="combo small"><span>{m.detail}</span>{@render chevrons()}</span>
					{:else}
						<span class="detail">{m.detail}</span>
					{/if}
					<span class="meter">
						{#each { length: SEGMENTS }, i (i)}
							<i class={[segment(i), { lit: i < m.lit }]}></i>
						{/each}
					</span>
					<span class="track gain"><i style:width="62%"></i><b style:left="62%"></b></span>
					<span class="mono db">0.0 dB</span>
				</div>
			{/each}
		</div>

		<div class="recordbar">
			<div class="clock-wrap">
				<span class={['clock', phase]}>{live ? time : '00:00'}</span>
				<span class="saves">Saves to ~/Movies/Cortex</span>
			</div>
			<span
				class={['record', { active: live, saving: phase === 'saving' }]}
				style:left="{FRAME_CX - 272 - 78}px"
			>
				{#if live}
					<i class="stop"></i>Stop
				{:else if phase === 'saving'}
					Saving…
				{:else}
					<i class="dot"></i>Record
				{/if}
				<kbd>⇧⌘R</kbd>
			</span>
			<span
				class={['square-button', { off: !live, pressed: down(T.recording[0][1]) || down(T.recording[1][0]) }]}
				style:left="{FRAME_CX - 272 + 86}px"
			>
				{#if phase === 'paused'}
					<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M4 2.5v9l7.5-4.5z" class="solid" /></svg>
				{:else}
					<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M4.5 2.5v9M9.5 2.5v9" /></svg>
				{/if}
			</span>
			<span class="go-live">
				<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
					<circle cx="7" cy="7" r="1.2" class="solid" />
					<path d="M4.6 4.6a3.4 3.4 0 0 0 0 4.8M9.4 4.6a3.4 3.4 0 0 1 0 4.8M2.8 2.8a5.9 5.9 0 0 0 0 8.4M11.2 2.8a5.9 5.9 0 0 1 0 8.4" />
				</svg>
				Go live
			</span>
		</div>
	</div>
</div>

<style>
	.window {
		position: absolute;
		inset: 0;
		display: flex;
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

	svg {
		flex-shrink: 0;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	svg .solid {
		fill: currentColor;
		stroke: none;
	}

	.mono {
		font-family: var(--mono);
		font-size: 11px;
	}

	.pressed {
		filter: brightness(0.86);
	}

	:global(:root:not([data-theme='light'])) .pressed {
		filter: brightness(1.35);
	}

	/* Sidebar */

	.sidebar {
		position: relative;
		width: 272px;
		flex-shrink: 0;
		border-right: 1px solid var(--separator);
		background: var(--sidebar);
	}

	.sidebar > *,
	.camera > * {
		position: absolute;
		left: 16px;
		right: 16px;
	}

	.camera {
		position: absolute;
		inset: 0;
	}

	.lights {
		top: 18px;
		display: flex;
		gap: 8px;
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

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		color: var(--secondary);
		font-size: 10.5px;
		font-weight: 600;
		letter-spacing: 0.08em;
		line-height: 14px;
	}

	.overline {
		color: var(--secondary);
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.08em;
	}

	.rule {
		left: 0 !important;
		right: 0 !important;
		height: 1px;
		background: var(--separator);
	}

	.segments {
		display: grid;
		height: 28px;
		padding: 2px;
		border-radius: 8px;
		background: var(--segments);
		color: var(--secondary);
		font-size: 11.5px;
	}

	.segments.four {
		grid-template-columns: repeat(4, 1fr);
	}

	.segments.three {
		grid-template-columns: repeat(3, 1fr);
	}

	.segments span {
		position: relative;
		display: grid;
		place-items: center;
		white-space: nowrap;
	}

	.segments .on {
		color: var(--text);
		font-weight: 500;
	}

	.segments .thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		height: 24px;
		border-radius: 6px;
		background: var(--segment);
		box-shadow: 0 1px 0 rgba(0, 0, 0, 0.18);
		transition: transform 0.2s cubic-bezier(0.2, 0.9, 0.3, 1);
	}

	.segments.four .thumb {
		width: calc((100% - 4px) / 4);
	}

	.segments.three .thumb {
		width: calc((100% - 4px) / 3);
	}

	.capture {
		display: flex;
		flex-direction: column;
		gap: 6px;
		height: 112px;
	}

	.display {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 50px;
		padding: 0 12px 0 8px;
		border-radius: 10px;
	}

	.display.on {
		background: var(--group);
		box-shadow: inset 0 0 0 1px var(--accent);
	}

	.display div {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 2px;
	}

	.display b {
		font-weight: 500;
	}

	.display div span {
		color: var(--secondary);
		font-size: 11px;
		line-height: 13px;
	}

	.display-icon,
	.area-icon,
	.strip-icon {
		display: grid;
		place-items: center;
		width: 30px;
		height: 30px;
		flex-shrink: 0;
		border-radius: 7px;
		background: var(--accent-tint);
		color: var(--accent-text);
	}

	.check {
		color: var(--accent);
		stroke-width: 1.8;
	}

	.area-box {
		position: relative;
		height: 112px;
		padding: 10px 10px 10px 50px;
		border-radius: 10px;
		background: var(--group);
		box-shadow: inset 0 0 0 1px var(--group-edge);
	}

	.area-icon {
		position: absolute;
		left: 10px;
		top: 10px;
	}

	.area-box p {
		margin: 0;
		color: var(--secondary);
		font-size: 11.5px;
		line-height: 15px;
	}

	.wide-button {
		position: absolute;
		left: 10px;
		right: 10px;
		bottom: 8px;
		display: grid;
		place-items: center;
		height: 28px;
		border-radius: 7px;
		background: var(--control);
		box-shadow:
			inset 0 0 0 1px var(--control-edge),
			0 2px 0 var(--control-edge);
		font-weight: 500;
	}

	.combo {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		height: 28px;
		padding: 0 10px;
		border-radius: 7px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
	}

	.combo span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevrons {
		stroke: var(--chevrons);
		stroke-width: 1.3;
	}

	.shapes {
		display: grid;
		grid-template-columns: repeat(6, 1fr);
		gap: 4px;
		padding: 3px;
		border-radius: 9px;
		background: var(--segments);
	}

	.shapes span {
		display: grid;
		place-items: center;
		height: 28px;
		border-radius: 6px;
		color: var(--secondary);
	}

	.shapes .on {
		background: var(--segment);
		box-shadow: 0 1px 0 rgba(0, 0, 0, 0.18);
		color: var(--text);
	}

	.slider {
		display: flex;
		flex-direction: column;
		gap: 9px;
	}

	.slider-label {
		display: flex;
		justify-content: space-between;
		color: var(--secondary);
		font-size: 12px;
		line-height: 14px;
	}

	.track {
		position: relative;
		display: block;
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
		background: #2a2e33;
		box-shadow:
			inset 0 0 0 1px rgba(255, 255, 255, 0.14),
			0 1px 2px rgba(0, 0, 0, 0.4);
	}

	:global(:root[data-theme='light']) .track b {
		background: #fff;
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.16),
			0 1px 2px rgba(0, 0, 0, 0.25);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 24px;
	}

	.switch {
		position: relative;
		width: 32px;
		height: 18px;
		flex-shrink: 0;
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
		left: 16px;
		background: var(--on-accent);
	}

	:global(:root[data-theme='light']) .switch.on i {
		background: #fff;
	}

	/* Main column */

	.main {
		position: relative;
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
	}

	.titlebar {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
		height: 44px;
		flex-shrink: 0;
	}

	.titlebar b {
		font-weight: 600;
	}

	.title-time {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--rec);
		font: 12px var(--mono);
	}

	.title-time i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: currentColor;
	}

	.title-time.paused {
		color: var(--paused);
	}

	.title-time.saving {
		color: var(--secondary);
		font-family: var(--sans);
	}

	.tools {
		position: absolute;
		right: 16px;
		display: flex;
		gap: 14px;
		color: var(--secondary);
	}

	.stage {
		position: relative;
		height: 380px;
		margin: 8px 16px 0;
		border-radius: 12px;
		background: var(--stage);
		flex-shrink: 0;
	}

	.frame {
		position: absolute;
		overflow: hidden;
		border-radius: 4px;
		background: #111;
		transition: box-shadow 0.15s;
	}

	.frame.rec {
		box-shadow: 0 0 0 2px var(--rec);
	}

	.frame.paused {
		box-shadow: 0 0 0 2px var(--paused);
	}

	.badge {
		position: absolute;
		left: 10px;
		top: 10px;
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 3px 8px;
		border-radius: 999px;
		background: rgba(10, 10, 12, 0.72);
		color: #fff;
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.06em;
	}

	.badge i {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: #f0565b;
	}

	.badge.paused i {
		background: #f2b04a;
	}

	.shot,
	.desktop {
		position: absolute;
		inset: 0;
		overflow: hidden;
	}

	.desktop {
		background:
			radial-gradient(circle farthest-corner at 20% 16%, hsl(188 50% 40% / 0.55), transparent 58%),
			radial-gradient(circle farthest-corner at 96% 100%, hsl(196 45% 30% / 0.4), transparent 60%),
			linear-gradient(160deg, #12282d, #0a1215 55%, #0c1a1f);
	}

	.shot {
		background: #fbfbfa;
	}

	.shot-slide,
	.desk-slide {
		width: 664px;
		transform-origin: 0 0;
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
		background: #fbfbfa;
	}

	.notes {
		left: 344px;
		top: 56px;
		width: 160px;
		height: 180px;
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

	/* The region picker's selection: everything outside it is dimmed. */
	.marquee {
		position: absolute;
		border: 1.5px solid hsl(188 58% 58%);
		box-shadow: 0 0 0 999px rgba(0, 0, 0, 0.45);
	}

	.pip {
		position: absolute;
		overflow: hidden;
		box-shadow: 0 10px 26px rgba(0, 0, 0, 0.4);
		transform-origin: 100% 100%;
	}

	.outline,
	.ghost {
		position: absolute;
		border-radius: 6px;
		pointer-events: none;
	}

	.outline {
		box-shadow: 0 0 0 1.5px var(--accent);
	}

	.ghost {
		box-shadow: 0 0 0 1px hsl(188 58% 58% / 0.8);
		background: hsl(188 58% 58% / 0.12);
	}

	.chips {
		display: flex;
		gap: 6px;
		height: 24px;
		margin: 8px 16px 0;
	}

	.chips span {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px;
		border-radius: 6px;
		box-shadow: inset 0 0 0 1px var(--separator);
		color: var(--secondary);
		font: 11px/22px var(--mono);
	}

	.mixer {
		display: flex;
		flex-direction: column;
		height: 96px;
		margin-top: 12px;
		padding: 4px 16px;
		border-top: 1px solid var(--separator);
		background: var(--window);
	}

	.strip {
		display: flex;
		align-items: center;
		gap: 12px;
		height: 44px;
	}

	.strip-icon {
		width: 26px;
		height: 26px;
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
		color: var(--secondary);
	}

	.strip-name {
		width: 96px;
		font-weight: 500;
	}

	.detail,
	.combo.small {
		width: 172px;
		color: var(--secondary);
		font-size: 12px;
	}

	.combo.small {
		height: 26px;
		color: var(--text);
	}

	.meter {
		display: flex;
		flex: 1;
		gap: 2px;
	}

	.meter i {
		flex: 1;
		height: 7px;
		border-radius: 1.5px;
		background: color-mix(in srgb, var(--text) 10%, transparent);
	}

	.meter .low.lit {
		background: var(--meter-low);
	}

	.meter .mid.lit {
		background: var(--meter-mid);
	}

	.meter .high.lit {
		background: var(--meter-high);
	}

	.gain {
		width: 90px;
		flex-shrink: 0;
	}

	.db {
		width: 44px;
		color: var(--secondary);
		text-align: right;
	}

	.recordbar {
		position: relative;
		flex: 1;
		border-top: 1px solid var(--separator);
	}

	.clock-wrap {
		position: absolute;
		left: 16px;
		top: 14px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.clock {
		color: var(--tertiary);
		font: 500 24px/26px var(--mono);
		font-variant-numeric: tabular-nums;
	}

	.clock.recording {
		color: var(--rec);
	}

	.clock.paused {
		color: var(--paused);
	}

	.saves {
		color: var(--secondary);
		font-size: 11px;
		line-height: 13px;
	}

	.record {
		position: absolute;
		top: 13px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		width: 156px;
		height: 44px;
		border-radius: 11px;
		background: var(--rec);
		box-shadow: 0 3px 0 var(--rec-ledge);
		color: #fff;
		font-size: 15px;
		font-weight: 600;
	}

	.record.saving {
		background: var(--control);
		box-shadow: inset 0 0 0 1px var(--control-edge);
		color: var(--secondary);
	}

	.record .dot {
		width: 13px;
		height: 13px;
		border-radius: 50%;
		background: #fff;
	}

	.record .stop {
		width: 12px;
		height: 12px;
		border-radius: 3px;
		background: #fff;
	}

	.record kbd {
		padding: 1px 5px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.22);
		font: 600 10px/14px var(--sans);
	}

	.record.saving kbd {
		background: var(--track);
	}

	.square-button,
	.go-live {
		position: absolute;
		top: 13px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 7px;
		height: 44px;
		border-radius: 11px;
		background: var(--control);
		box-shadow:
			inset 0 0 0 1px var(--control-edge),
			0 2px 0 var(--control-edge);
		transition: filter 0.1s;
	}

	.square-button {
		width: 44px;
	}

	.square-button.off {
		color: var(--tertiary);
	}

	.go-live {
		right: 16px;
		padding: 0 14px;
		font-weight: 500;
	}
</style>

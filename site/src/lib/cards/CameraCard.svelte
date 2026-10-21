<script lang="ts">
	import { along, easeInOut, easeOut, lerp, span } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Cursor from '#lib/rec/Cursor.svelte';
	import Person from '#lib/rec/Person.svelte';
	import ShapeIcon, { SHAPES, type Shape } from '#lib/rec/ShapeIcon.svelte';
	import Fit from './Fit.svelte';

	/** The preview inside the mini window, in card coordinates. */
	const PREVIEW = { x: 63, y: 43, w: 380, h: 238 };
	const MARGIN = 8;
	const SIZE: Record<Shape, { w: number; h: number; r: number }> = {
		circle: { w: 78, h: 78, r: 39 },
		squircle: { w: 78, h: 78, r: 27 },
		square: { w: 78, h: 78, r: 23 },
		pill: { w: 78, h: 44, r: 22 },
		rounded: { w: 78, h: 44, r: 13 }
	};
	const PICKS: { at: number; shape: Shape }[] = [
		{ at: 0, shape: 'circle' },
		{ at: 3000, shape: 'squircle' },
		{ at: 4000, shape: 'pill' },
		{ at: 7200, shape: 'rounded' },
		{ at: 8100, shape: 'circle' }
	];
	const DRAGS = [
		{ grab: 900, drop: 2300, to: 'left' },
		{ grab: 5100, drop: 6500, to: 'right' }
	] as const;
	const CURSOR = [
		{ t: 0, x: 440, y: 300 },
		{ t: 600, x: 396, y: 234 },
		{ t: 900, x: 396, y: 234 },
		{ t: 2200, x: 120, y: 226 },
		{ t: 2400, x: 120, y: 226 },
		{ t: 2900, x: 369, y: 343 },
		{ t: 3100, x: 369, y: 343 },
		{ t: 3800, x: 311, y: 343 },
		{ t: 4100, x: 311, y: 343 },
		{ t: 4800, x: 110, y: 251 },
		{ t: 5100, x: 110, y: 251 },
		{ t: 6400, x: 386, y: 246 },
		{ t: 6600, x: 386, y: 246 },
		{ t: 7100, x: 195, y: 343 },
		{ t: 7300, x: 195, y: 343 },
		{ t: 7900, x: 137, y: 343 },
		{ t: 8200, x: 137, y: 343 },
		{ t: 9300, x: 440, y: 300 },
		{ t: 10_000, x: 440, y: 300 }
	];

	const clock = loop(10_000, 4300);
	let t = $derived(clock.t);
	let cursor = $derived(along(CURSOR, t));

	let pick = $derived(PICKS.findLast((p) => t >= p.at) ?? PICKS[0]);
	let size = $derived.by(() => {
		const i = PICKS.indexOf(pick);
		const from = SIZE[PICKS[Math.max(0, i - 1)].shape];
		const to = SIZE[pick.shape];
		const p = easeInOut(span(t, pick.at, pick.at + 300));
		return { w: lerp(from.w, to.w, p), h: lerp(from.h, to.h, p), r: lerp(from.r, to.r, p) };
	});

	const rest = (side: 'left' | 'right', h: number) => ({
		x: side === 'left' ? MARGIN : PREVIEW.w - MARGIN - 78,
		y: PREVIEW.h - MARGIN - h
	});
	let drag = $derived(DRAGS.findLast((d) => t >= d.grab) ?? null);
	let dragging = $derived(drag !== null && t < drag.drop);
	let pos = $derived.by(() => {
		if (!drag) return rest('right', size.h);
		const start = rest(drag.to === 'left' ? 'right' : 'left', size.h);
		const grab = along(CURSOR, drag.grab);
		const held = (at: number) => {
			const c = along(CURSOR, at);
			return { x: start.x + c.x - grab.x, y: start.y + c.y - grab.y };
		};
		if (t < drag.drop) return held(t);
		const end = rest(drag.to, size.h);
		const from = held(drag.drop);
		const p = easeOut(span(t, drag.drop, drag.drop + 300));
		return { x: lerp(from.x, end.x, p), y: lerp(from.y, end.y, p) };
	});
	let target = $derived(drag ? rest(drag.to, size.h) : null);
	let nearCorner = $derived(dragging && target !== null && Math.hypot(pos.x - target.x, pos.y - target.y) < 34);
	let hovering = $derived((t >= 600 && t < 2600) || (t >= 4800 && t < 6800));
	let clicking = $derived(PICKS.some((p) => p.at > 0 && t >= p.at - 60 && t < p.at + 120));
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={395}>
		<div class="window app">
			<div class="preview">
				<i class="kicker"></i>
				<i class="title"></i>
				<i class="title short"></i>
				<div class="lines"><i></i><i></i><i></i></div>
				{#if nearCorner && target}
					<div class="ghost" style:left="{target.x}px" style:top="{target.y}px" style:width="{size.w}px" style:height="{size.h}px"></div>
				{/if}
				<div
					class="pip"
					style:left="{pos.x}px"
					style:top="{pos.y}px"
					style:width="{size.w}px"
					style:height="{size.h}px"
					style:border-radius="{size.r}px"
				>
					<Person />
				</div>
				{#if hovering}
					<div class="outline" style:left="{pos.x}px" style:top="{pos.y}px" style:width="{size.w}px" style:height="{size.h}px"></div>
				{/if}
			</div>
		</div>
		<div class="picker app">
			{#each SHAPES as s (s)}
				<span class={{ on: s === pick.shape, pressed: clicking && s === pick.shape }}><ShapeIcon shape={s} /></span>
			{/each}
		</div>
		<Cursor x={cursor.x} y={cursor.y} kind={dragging ? 'grabbing' : hovering ? 'grab' : 'arrow'} pressed={clicking} />
	</Fit>
</div>

<style>
	.card-visual {
		background: radial-gradient(ellipse 70% 60% at 50% 0%, rgba(140, 150, 255, 0.09), transparent);
	}

	.window {
		position: absolute;
		left: 47px;
		top: 28px;
		width: 412px;
		height: 268px;
		padding: 15px 16px;
		border-radius: 12px;
		background: var(--window);
		box-shadow:
			0 0 0 0.5px rgba(255, 255, 255, 0.16),
			0 24px 50px -12px rgba(0, 0, 0, 0.6);
	}

	:global(:root[data-theme='light']) .window {
		box-shadow:
			0 0 0 0.5px rgba(0, 0, 0, 0.16),
			0 24px 50px -12px rgba(0, 0, 0, 0.25);
	}

	.preview {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 9px;
		width: 380px;
		height: 238px;
		padding: 34px 28px;
		background: #fbfbfa;
	}

	.preview > i {
		display: block;
		height: 16px;
		border-radius: 4px;
		background: #2a2a2e;
	}

	.preview .kicker {
		width: 92px;
		height: 5px;
		border-radius: 3px;
		background: #c9c9ce;
	}

	.preview .title {
		width: 196px;
	}

	.preview .title.short {
		width: 138px;
	}

	.lines {
		display: flex;
		flex-direction: column;
		gap: 7px;
		padding-top: 8px;
	}

	.lines i {
		width: 150px;
		height: 6px;
		border-radius: 3px;
		background: #d3d3d8;
	}

	.lines i:nth-child(2) {
		width: 118px;
	}

	.lines i:nth-child(3) {
		width: 134px;
	}

	.pip {
		position: absolute;
		overflow: hidden;
		border: 2px solid #fff;
		box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3);
	}

	.outline,
	.ghost {
		position: absolute;
		margin: -2px;
		padding: 2px;
		border-radius: 4px;
		box-sizing: content-box;
	}

	.outline {
		box-shadow: 0 0 0 1.5px var(--accent);
	}

	.ghost {
		box-shadow: 0 0 0 1px rgba(0, 122, 255, 0.7);
	}

	.picker {
		position: absolute;
		left: 103px;
		top: 318px;
		display: flex;
		gap: 6px;
		padding: 8px;
		border-radius: 12px;
		background: var(--group);
		box-shadow:
			inset 0 0 0 1px var(--group-edge),
			0 12px 30px -10px rgba(0, 0, 0, 0.6);
	}

	:global(:root[data-theme='light']) .picker {
		box-shadow:
			0 0 0 1px rgba(0, 0, 0, 0.08),
			0 12px 30px -10px rgba(0, 0, 0, 0.2);
	}

	.picker span {
		display: grid;
		place-items: center;
		width: 52px;
		height: 34px;
		border-radius: 7px;
		box-shadow: inset 0 0 0 1px var(--control-edge);
		color: var(--secondary);
		transition:
			background-color 0.15s,
			color 0.15s;
	}

	.picker .on {
		background: rgba(0, 122, 255, 0.18);
		box-shadow: inset 0 0 0 1.5px var(--accent);
		color: #2d8cff;
	}

	.picker .pressed {
		background: rgba(0, 122, 255, 0.32);
	}
</style>

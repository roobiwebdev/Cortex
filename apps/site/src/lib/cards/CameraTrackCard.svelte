<script lang="ts">
	import { along, easeInOut, lerp, span } from '#lib/ease.js';
	import { loop } from '#lib/motion.svelte.js';
	import Person from '#lib/mock/Person.svelte';
	import Fit from './Fit.svelte';

	/**
	 * With "Save camera as separate track" on, the file gets the screen with the overlay burned in,
	 * plus the raw camera on a second video track (docs/engineering/desktop.md, "Camera track").
	 * The overlay moves and changes size above; the camera track below stays whole. Decorative only.
	 */
	const FRAME = { w: 232, h: 130 };
	const MOVES = [
		{ t: 0, x: 168, y: 66 },
		{ t: 1600, x: 168, y: 66 },
		{ t: 2400, x: 10, y: 66 },
		{ t: 4400, x: 10, y: 66 },
		{ t: 5200, x: 10, y: 10 },
		{ t: 7200, x: 10, y: 10 },
		{ t: 8000, x: 168, y: 66 },
		{ t: 9000, x: 168, y: 66 }
	];

	const clock = loop(9000, 1000);
	let t = $derived(clock.t);
	let pos = $derived(along(MOVES, t));
	let size = $derived(lerp(54, 70, easeInOut(span(t, 4400, 5200)) - easeInOut(span(t, 7200, 8000))));
</script>

<div class="card-visual app" {@attach clock.observe}>
	<Fit width={506} height={430}>
		<div class="setting">
			<span>Save camera as separate track</span>
			<span class="switch"><i></i></span>
		</div>

		<div class="track one">
			<div class="frame screen">
				<i class="kicker"></i>
				<i class="title"></i>
				<div class="lines"><i></i><i></i><i></i></div>
				<div class="pip" style:left="{pos.x}px" style:top="{pos.y}px" style:width="{size}px" style:height="{size}px">
					<span class="mirror"><Person /></span>
				</div>
			</div>
			<div class="label">
				<b>Track 1 · Screen</b>
				<span>The camera overlay is burned in. This is what players show.</span>
			</div>
		</div>

		<div class="track two">
			<div class="frame cam"><Person /></div>
			<div class="label">
				<b>Track 2 · Camera</b>
				<span>The raw camera, unmirrored, for your editor to reframe.</span>
			</div>
		</div>

		<div class="chips">
			<span>.mov: a second video track</span>
			<span>.mp4: a .camera.mov beside it</span>
		</div>
	</Fit>
</div>

<style>
	.card-visual {
		background: radial-gradient(ellipse 70% 60% at 50% 0%, hsl(188 60% 55% / 0.09), transparent);
		color: var(--fg);
	}

	.setting {
		position: absolute;
		left: 28px;
		top: 28px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 450px;
		height: 40px;
		padding: 0 14px;
		border-radius: 10px;
		background: var(--group);
		box-shadow: inset 0 0 0 1px var(--group-edge);
		color: var(--text);
		font-size: 13px;
	}

	.switch {
		position: relative;
		width: 32px;
		height: 18px;
		border-radius: 9px;
		background: var(--accent);
	}

	.switch i {
		position: absolute;
		top: 2px;
		left: 16px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--on-accent);
	}

	:global(:root[data-theme='light']) .switch i {
		background: #fff;
	}

	.track {
		position: absolute;
		left: 28px;
		display: flex;
		align-items: center;
		gap: 18px;
		width: 450px;
	}

	.one {
		top: 88px;
	}

	.two {
		top: 236px;
	}

	.frame {
		position: relative;
		width: 232px;
		height: 130px;
		flex-shrink: 0;
		overflow: hidden;
		border-radius: 8px;
		box-shadow:
			0 0 0 1px rgba(255, 255, 255, 0.08),
			0 16px 34px -14px rgba(0, 0, 0, 0.6);
	}

	:global(:root[data-theme='light']) .frame {
		box-shadow:
			0 0 0 1px rgba(0, 0, 0, 0.08),
			0 16px 34px -14px rgba(0, 0, 0, 0.25);
	}

	.screen {
		display: flex;
		flex-direction: column;
		gap: 7px;
		padding: 22px 20px;
		background: #fbfbfa;
	}

	.screen > i {
		display: block;
		height: 12px;
		border-radius: 3px;
		background: #2a2a2e;
	}

	.screen .kicker {
		width: 70px;
		height: 4px;
		background: #c9c9ce;
	}

	.screen .title {
		width: 140px;
	}

	.lines {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding-top: 6px;
	}

	.lines i {
		width: 110px;
		height: 5px;
		border-radius: 3px;
		background: #d3d3d8;
	}

	.pip {
		position: absolute;
		overflow: hidden;
		border-radius: 50%;
		box-shadow: 0 6px 16px rgba(0, 0, 0, 0.35);
	}

	/* The overlay is mirrored like the preview; the camera track keeps the camera's own frames. */
	.mirror {
		display: block;
		width: 100%;
		height: 100%;
		transform: scaleX(-1);
	}

	.label {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.label b {
		font-size: 13px;
		font-weight: 600;
	}

	.label span {
		color: var(--muted);
		font-size: 12.5px;
		line-height: 17px;
	}

	.chips {
		position: absolute;
		left: 28px;
		top: 388px;
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

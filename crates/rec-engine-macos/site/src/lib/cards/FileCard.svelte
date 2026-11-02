<script lang="ts">
	import { loop } from '#lib/motion.svelte.js';
	import Tracks from '#lib/rec/Tracks.svelte';
	import Fit from './Fit.svelte';

	const clock = loop(9000, 5580);
</script>

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={395}>
		<div class="head">
			<svg width="34" height="42" viewBox="0 0 34 42" aria-hidden="true">
				<path d="M4 1h18l11 11v25a4 4 0 0 1-4 4H4a4 4 0 0 1-4-4V5a4 4 0 0 1 4-4z" class="doc" />
				<path d="M22 1v8a3 3 0 0 0 3 3h8" class="fold" />
				<path d="M13 19.5v10l8.5-5z" class="play" />
			</svg>
			<div>
				<b>rec-2026-10-20-094112.mov</b>
				<span>03:12 · 3840 × 2160 · 60 fps · 4 tracks</span>
			</div>
		</div>
		<div class="tracks">
			<Tracks ruler={['0:00', '0:48', '1:36', '2:24', '3:12']} playhead={clock.t / 9000} />
		</div>
		<div class="chips">
			<span>.mov or .mp4</span>
			<span>H.264 · HEVC · ProRes 422</span>
			<span>Constant frame rate</span>
		</div>
	</Fit>
</div>

<style>
	.card-visual {
		background: radial-gradient(ellipse 70% 60% at 50% 0%, rgba(140, 150, 255, 0.09), transparent);
	}

	.head {
		position: absolute;
		left: 28px;
		top: 30px;
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.head div {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}

	.head b {
		font-size: 15px;
		font-weight: 600;
		line-height: 18px;
	}

	.head span {
		color: var(--muted);
		font-size: 12px;
		line-height: 16px;
	}

	.doc {
		fill: #2c2c2e;
		stroke: rgba(255, 255, 255, 0.14);
	}

	.fold {
		fill: #3a3a3c;
	}

	.play {
		fill: #f4f4f5;
	}

	:global(:root[data-theme='light']) .doc {
		fill: #fff;
		stroke: rgba(0, 0, 0, 0.14);
	}

	:global(:root[data-theme='light']) .fold {
		fill: #e5e5ea;
	}

	:global(:root[data-theme='light']) .play {
		fill: #1d1d1f;
	}

	.tracks {
		position: absolute;
		left: 26px;
		top: 98px;
	}

	.chips {
		position: absolute;
		left: 28px;
		top: 342px;
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

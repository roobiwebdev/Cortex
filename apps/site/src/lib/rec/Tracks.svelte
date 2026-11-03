<script lang="ts">
	import { MIC, MIX, SYSTEM, WIDTH, bars } from './waves.js';

	/** A recording's four tracks, 454 px wide. `reveal` draws them in from the left; `dark` ignores the page theme. */
	let {
		ruler,
		reveal = 1,
		playhead = null,
		dark = false
	}: { ruler: string[]; reveal?: number; playhead?: number | null; dark?: boolean } = $props();

	const H = 26;
	const audio = [
		{ name: 'Mix', kind: 'AAC · stereo', path: bars(MIX, H), mix: true },
		{ name: 'System', kind: 'AAC · stereo', path: bars(SYSTEM, H), mix: false },
		{ name: 'Mic', kind: 'AAC · mono', path: bars(MIC, H), mix: false }
	];
	let clip = $derived(`inset(0 ${((1 - reveal) * 100).toFixed(2)}% 0 0)`);
</script>

<div class={['tracks', { dark }]}>
	<div class="ruler">
		{#each ruler as mark (mark)}<span>{mark}</span>{/each}
	</div>
	<div class="lane">
		<div class="name"><b>Video</b><span>HEVC · 4K60</span></div>
		<div class="film" style:clip-path={clip}>
			{#each { length: 7 }, i (i)}
				<div class="frame"><i></i><i></i><i></i><em></em></div>
			{/each}
		</div>
	</div>
	{#each audio as track (track.name)}
		<div class="lane">
			<div class="name"><b>{track.name}</b><span>{track.kind}</span></div>
			<svg
				class={{ wave: true, mix: track.mix }}
				width={WIDTH}
				height={H}
				viewBox="0 0 {WIDTH} {H}"
				style:clip-path={clip}
				aria-hidden="true"
			>
				<path d={track.path} />
			</svg>
		</div>
	{/each}
	{#if playhead !== null}
		<div class="playhead" style:left="{110 + playhead * WIDTH}px"></div>
	{/if}
</div>

<style>
	.tracks {
		position: relative;
		width: 454px;
		padding: 6px 14px 4px;
		border-radius: 12px;
		background: #1c1c1e;
		box-shadow:
			inset 0 0 0 1px rgba(255, 255, 255, 0.07),
			0 20px 44px -16px rgba(0, 0, 0, 0.7);
		color: #f4f4f5;
		font-family: var(--sans);
	}

	.ruler {
		display: flex;
		justify-content: space-between;
		align-items: center;
		height: 20px;
		padding-left: 96px;
		color: #636366;
		font: 10px/12px var(--mono);
	}

	.lane {
		display: flex;
		align-items: center;
		gap: 14px;
		height: 46px;
	}

	.lane + .lane {
		border-top: 1px solid rgba(255, 255, 255, 0.06);
	}

	.name {
		display: flex;
		flex-direction: column;
		gap: 1px;
		width: 82px;
		flex-shrink: 0;
	}

	.name b {
		font-size: 12px;
		font-weight: 600;
		line-height: 15px;
	}

	.name span {
		color: #8e8e93;
		font-size: 11px;
		line-height: 14px;
	}

	.film {
		display: flex;
		gap: 4px;
		width: 326px;
		overflow: hidden;
	}

	.frame {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 3px;
		width: 44px;
		height: 28px;
		padding: 6px 5px;
		flex-shrink: 0;
		border-radius: 3px;
		background: #fbfbfa;
	}

	.frame i {
		width: 22px;
		height: 3px;
		border-radius: 1px;
		background: #2a2a2e;
	}

	.frame i:nth-child(2) {
		width: 16px;
	}

	.frame i:nth-child(3) {
		width: 20px;
		height: 2px;
		background: #cfcfd4;
	}

	/* The camera overlay is burned into the video track. */
	.frame em {
		position: absolute;
		right: 3px;
		bottom: 3px;
		width: 9px;
		height: 9px;
		border-radius: 3px;
		background: #4a5568;
		box-shadow: 0 0 0 1px #fff;
	}

	.wave {
		display: block;
		fill: rgba(235, 235, 245, 0.42);
	}

	.wave.mix {
		fill: #0a84ff;
	}

	.playhead {
		position: absolute;
		top: 10px;
		bottom: 6px;
		width: 1.5px;
		background: #fff;
		box-shadow: 0 0 6px rgba(0, 0, 0, 0.5);
	}

	:global(:root[data-theme='light']) .tracks:not(.dark) {
		background: #fff;
		box-shadow:
			0 0 0 1px rgba(0, 0, 0, 0.08),
			0 20px 44px -18px rgba(0, 0, 0, 0.25);
		color: #1d1d1f;

		.ruler {
			color: #aeaeb2;
		}

		.name span {
			color: #6e6e73;
		}

		.lane + .lane {
			border-color: rgba(0, 0, 0, 0.07);
		}

		.wave {
			fill: rgba(60, 60, 67, 0.32);
		}

		.wave.mix {
			fill: #007aff;
		}

		.frame {
			background: #f2f2f4;
			box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.1);
		}

		.playhead,
		.playhead::before {
			background: #1d1d1f;
			box-shadow: none;
		}
	}

	.playhead::before {
		content: '';
		position: absolute;
		top: -3px;
		left: -3.25px;
		width: 8px;
		height: 8px;
		border-radius: 2px;
		background: #fff;
	}
</style>

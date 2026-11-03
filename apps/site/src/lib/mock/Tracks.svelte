<script lang="ts">
	import { MIC, MIX, SYSTEM, WIDTH, bars } from './waves.js';

	/**
	 * A recording's tracks, 454 px wide: the screen, the camera track when `camera` is on, then the
	 * mix, system and mic audio (the writer's layout). `reveal` draws them in from the left, `trim`
	 * shows Quick trim's kept range (0..1), and `dark` ignores the page theme.
	 */
	let {
		ruler,
		reveal = 1,
		playhead = null,
		dark = false,
		camera = false,
		trim = null
	}: {
		ruler: string[];
		reveal?: number;
		playhead?: number | null;
		dark?: boolean;
		camera?: boolean;
		trim?: { start: number; end: number } | null;
	} = $props();

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
		<div class="name"><b>Screen</b><span>HEVC · 4K60</span></div>
		<div class="film" style:clip-path={clip}>
			{#each { length: 7 }, i (i)}
				<div class="frame"><i></i><i></i><i></i><em></em></div>
			{/each}
		</div>
	</div>
	{#if camera}
		<div class="lane">
			<div class="name"><b>Camera</b><span>HEVC · 1080p</span></div>
			<div class="film" style:clip-path={clip}>
				{#each { length: 7 }, i (i)}
					<div class="frame cam"><u></u></div>
				{/each}
			</div>
		</div>
	{/if}
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
	{#if trim}
		<div class="trim" style:left="{110 + trim.start * WIDTH}px" style:width="{(trim.end - trim.start) * WIDTH}px">
			<i class="handle start"></i><i class="handle end"></i>
		</div>
		<div class="cut" style:left="110px" style:width="{trim.start * WIDTH}px"></div>
		<div class="cut" style:left="{110 + trim.end * WIDTH}px" style:width="{(1 - trim.end) * WIDTH}px"></div>
	{/if}
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

	/* The camera track holds the raw camera, so its frames show the person, not the screen. */
	.frame.cam {
		overflow: hidden;
		padding: 0;
		background: linear-gradient(#55617a, #262b38);
	}

	.frame.cam u {
		position: absolute;
		left: 50%;
		bottom: -8px;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: #1d2742;
		box-shadow: 0 -12px 0 -4px #c48f74;
		translate: -50% 0;
	}

	.trim,
	.cut {
		position: absolute;
		top: 28px;
		bottom: 4px;
		pointer-events: none;
	}

	.trim {
		border: 2px solid hsl(188 58% 58%);
		border-radius: 4px;
	}

	.cut {
		background: rgba(0, 0, 0, 0.55);
	}

	.handle {
		position: absolute;
		top: 50%;
		width: 8px;
		height: 30px;
		border-radius: 3px;
		background: hsl(188 58% 58%);
		translate: 0 -50%;
	}

	.handle.start {
		left: -5px;
	}

	.handle.end {
		right: -5px;
	}

	.wave {
		display: block;
		fill: rgba(235, 235, 245, 0.42);
	}

	.wave.mix {
		fill: hsl(188 58% 58%);
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
			fill: hsl(188 62% 33%);
		}

		.trim {
			border-color: hsl(188 62% 33%);
		}

		.handle {
			background: hsl(188 62% 33%);
		}

		.cut {
			background: rgba(255, 255, 255, 0.6);
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

<script lang="ts">
	import { loop } from '#lib/motion.svelte.js';
	import Fit from './Fit.svelte';

	const W = 458;
	const clock = loop(7000, 0);
	let split = $derived(W * (0.5 + 0.17 * Math.sin((clock.t / 7000) * Math.PI * 2)));
</script>

{#snippet sample()}
	<div class="sample">
		<div class="code">fn emit_until(&amp;mut self, t: Tick)</div>
		<div class="heading">Settings, Recording, Pause / Resume</div>
		<div class="body">The quick brown fox jumps over 0123456789</div>
		<div class="chips"><span class="blue">Colored text 13 pt</span><span class="green">Colored text 13 pt</span></div>
	</div>
{/snippet}

<div class="card-visual" {@attach clock.observe}>
	<Fit width={506} height={304}>
		<div class="compare" aria-hidden="true">
			{@render sample()}
			<div class="scaled" style:clip-path="inset(0 {W - split}px 0 0)">{@render sample()}</div>
			<div class="divider" style:left="{split - 1}px"></div>
			<div class="handle" style:left="{split - 14}px">
				<svg width="14" height="10" viewBox="0 0 14 10"><path d="M4 1 .8 5 4 9M10 1l3.2 4L10 9" /></svg>
			</div>
			<span class="tag left">System scaler</span>
			<span class="tag right">Rec</span>
		</div>
		<p class="note">5K display to a 4K file: VMAF 100, up from 90 with the system scaler</p>
	</Fit>
</div>

<style>
	.compare {
		position: absolute;
		left: 24px;
		top: 26px;
		width: 458px;
		height: 226px;
		overflow: hidden;
		border-radius: 12px;
		background: #fbfbfa;
		box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.7);
	}

	:global(:root[data-theme='light']) .compare {
		box-shadow:
			0 0 0 1px rgba(0, 0, 0, 0.08),
			0 18px 40px -16px rgba(0, 0, 0, 0.25);
	}

	.sample {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 458px;
		padding: 34px 30px;
		color: #1c1c1e;
		white-space: nowrap;
	}

	.scaled {
		position: absolute;
		inset: 0;
		background: #fbfbfa;
	}

	.scaled .sample {
		filter: blur(1.1px);
	}

	.code {
		font: 17px/22px var(--mono);
	}

	.heading {
		font-size: 19px;
		font-weight: 600;
		line-height: 24px;
	}

	.body {
		color: #3a3a3e;
		font-size: 15px;
		line-height: 20px;
	}

	.chips {
		display: flex;
		gap: 8px;
		padding-top: 6px;
		font-size: 13px;
		line-height: 16px;
	}

	.chips span {
		padding: 4px 10px;
		border-radius: 6px;
	}

	.blue {
		background: #007aff;
		color: #fff;
	}

	.green {
		background: #1c1c1e;
		color: #30d158;
	}

	.divider {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 2px;
		background: #1c1c1e;
	}

	.handle {
		position: absolute;
		top: 99px;
		display: grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: #1c1c1e;
		box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
	}

	.handle svg {
		fill: none;
		stroke: #fff;
		stroke-width: 1.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.tag {
		position: absolute;
		bottom: 12px;
		padding: 3px 8px;
		border-radius: 6px;
		background: rgba(28, 28, 30, 0.8);
		color: #f4f4f5;
		font-size: 11px;
		font-weight: 600;
		line-height: 14px;
	}

	.left {
		left: 12px;
	}

	.right {
		right: 12px;
	}

	.note {
		position: absolute;
		top: 266px;
		width: 506px;
		margin: 0;
		color: var(--muted);
		font-size: 12px;
		line-height: 16px;
		text-align: center;
	}
</style>

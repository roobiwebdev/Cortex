<script lang="ts" module>
	export type Shape = 'circle' | 'rounded' | 'square' | 'pill' | 'squircle';

	/** The overlay shape picker's order (src/compose.rs). */
	export const SHAPES: Shape[] = ['circle', 'rounded', 'square', 'pill', 'squircle'];

	/** A superellipse at the default 60% roundness: exponent 2 + 8 × 0.4² (src/compose.rs). */
	const N = 3.28;
	const SQUIRCLE =
		Array.from({ length: 64 }, (_, i) => {
			const a = (i / 64) * Math.PI * 2;
			const [c, s] = [Math.cos(a), Math.sin(a)];
			const x = 9 + Math.sign(c) * Math.abs(c) ** (2 / N) * 8;
			const y = 9 + Math.sign(s) * Math.abs(s) ** (2 / N) * 8;
			return `${i ? 'L' : 'M'}${x.toFixed(2)} ${y.toFixed(2)}`;
		}).join('') + 'Z';
</script>

<script lang="ts">
	let { shape }: { shape: Shape } = $props();
</script>

{#if shape === 'circle' || shape === 'square' || shape === 'squircle'}
	<svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
		{#if shape === 'circle'}
			<circle cx="9" cy="9" r="8" />
		{:else if shape === 'square'}
			<rect x="1" y="1" width="16" height="16" rx="4.8" />
		{:else}
			<path d={SQUIRCLE} />
		{/if}
	</svg>
{:else}
	<svg width="24" height="15" viewBox="0 0 24 15" aria-hidden="true">
		<rect x="1" y="1" width="22" height="13" rx={shape === 'pill' ? 6.5 : 3.9} />
	</svg>
{/if}

<style>
	svg {
		display: block;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.5;
	}
</style>

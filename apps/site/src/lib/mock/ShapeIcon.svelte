<script lang="ts" module>
	export type Shape = 'rounded' | 'circle' | 'square' | 'pill' | 'squircle' | 'rect';

	/** The overlay shape picker's order (apps/desktop/src/surfaces/main-window/shape-glyph.tsx). */
	export const SHAPES: Shape[] = ['rounded', 'circle', 'square', 'pill', 'squircle', 'rect'];
</script>

<script lang="ts">
	/** The app's 20×20 line glyph for each camera overlay shape. */
	let { shape }: { shape: Shape } = $props();
</script>

<svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
	{#if shape === 'rounded'}
		<rect x="2" y="5" width="16" height="10" rx="3" />
	{:else if shape === 'circle'}
		<circle cx="10" cy="10" r="7" />
	{:else if shape === 'square'}
		<rect x="3.5" y="3.5" width="13" height="13" rx="1" />
	{:else if shape === 'pill'}
		<rect x="2" y="5.5" width="16" height="9" rx="4.5" />
	{:else if shape === 'squircle'}
		<path d="M10 3c5.6 0 7 1.4 7 7s-1.4 7-7 7-7-1.4-7-7 1.4-7 7-7Z" />
	{:else}
		<rect x="2" y="5" width="16" height="10" rx="0.5" />
	{/if}
</svg>

<style>
	svg {
		display: block;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.6;
	}
</style>

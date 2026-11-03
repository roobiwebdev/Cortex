<script lang="ts">
	/**
	 * The macOS pointer at (`x`, `y`): the arrow, the open and closed hands shown over the camera
	 * overlay, or the left-right resize pointer over a trim handle.
	 */
	let {
		x,
		y,
		kind = 'arrow',
		pressed = false
	}: { x: number; y: number; kind?: 'arrow' | 'grab' | 'grabbing' | 'resize'; pressed?: boolean } = $props();

	const OPEN =
		'M6.5 9.2V7.4c0-.9.7-1.5 1.5-1.5s1.5.6 1.5 1.5v-.6c0-.9.7-1.5 1.5-1.5s1.5.6 1.5 1.5v.3c0-.8.7-1.4 1.5-1.4s1.5.6 1.5 1.5v1.2c0-.8.6-1.3 1.4-1.3.8 0 1.4.6 1.4 1.4v5.2c0 3.3-2.4 5.8-5.6 5.8h-1.4c-2 0-3.4-.9-4.5-2.6L4.3 12.6c-.4-.7-.2-1.6.5-2 .6-.3 1.3-.2 1.7.3z';
	const CLOSED =
		'M7 10.2V9.4c0-.8.6-1.4 1.4-1.4.8 0 1.4.6 1.4 1.4v-.4c0-.8.6-1.4 1.4-1.4s1.4.6 1.4 1.4v.2c0-.8.6-1.3 1.4-1.3s1.4.6 1.4 1.4v.6c0-.7.6-1.2 1.3-1.2.7 0 1.3.6 1.3 1.3v4.7c0 3.1-2.3 5.4-5.3 5.4h-1.3c-1.9 0-3.2-.8-4.2-2.4l-1.9-3.4c-.4-.7-.2-1.5.5-1.9.6-.3 1.2-.2 1.6.3z';

	let hand = $derived(kind === 'grab' || kind === 'grabbing');
	let centred = $derived(kind !== 'arrow');
</script>

<svg
	class="cursor"
	viewBox="0 0 24 24"
	style:transform="translate({x - (centred ? 11 : 4)}px, {y - (centred ? 11 : 3)}px) scale({pressed ? 0.86 : 1})"
	style:transform-origin={centred ? '11px 11px' : '4px 3px'}
	aria-hidden="true"
>
	{#if kind === 'resize'}
		<path d="M2.5 11 7 6.5v2.8h3.2V4h2.6v14h-2.6v-5.3H7v2.8zM20.5 11 16 15.5v-2.8h-3.2V18h-2.6V4h2.6v5.3H16V6.5z" class="hand" />
	{:else if hand}
		<path d={kind === 'grabbing' ? CLOSED : OPEN} class="hand" />
	{:else}
		<path d="M4 3v15.2l3.9-3.7 2.6 6 2.7-1.2-2.6-5.9h5.4z" class="arrow" />
	{/if}
</svg>

<style>
	.cursor {
		position: absolute;
		top: 0;
		left: 0;
		z-index: 2;
		width: 24px;
		height: 24px;
		overflow: visible;
		filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.4));
		pointer-events: none;
	}

	.arrow {
		fill: #000;
		stroke: #fff;
		stroke-width: 1.3;
		stroke-linejoin: round;
	}

	.hand {
		fill: #fff;
		stroke: #000;
		stroke-width: 1;
		stroke-linejoin: round;
	}
</style>

<script lang="ts">
	/** The webcam picture: someone at a desk, drawn to fill whatever shape the overlay has. */
	const id = $props.id();
</script>

<svg viewBox="0 0 132 132" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
	<defs>
		<linearGradient id="{id}-room" x1="0" y1="0" x2="0.5" y2="1">
			<stop offset="0" stop-color="#55617a" />
			<stop offset="1" stop-color="#262b38" />
		</linearGradient>
		<radialGradient id="{id}-lamp" cx="0.85" cy="0.12" r="0.7">
			<stop offset="0" stop-color="#dce6fa" stop-opacity="0.5" />
			<stop offset="1" stop-color="#dce6fa" stop-opacity="0" />
		</radialGradient>
		<linearGradient id="{id}-skin" x1="0" y1="0" x2="1" y2="1">
			<stop offset="0" stop-color="#d9a88c" />
			<stop offset="1" stop-color="#b9846a" />
		</linearGradient>
	</defs>
	<rect x="-60" width="252" height="132" fill="url(#{id}-room)" />
	<rect x="-60" width="252" height="132" fill="url(#{id}-lamp)" />
	<rect x="8" y="14" width="22" height="58" rx="3" fill="#2f3546" opacity="0.7" />
	<rect x="12" y="20" width="14" height="10" rx="1.5" fill="#3b4356" opacity="0.8" />
	<path d="M10 132c3-30 22-42 56-44 34 2 53 14 56 44z" fill="#1d2742" />
	<path d="M52 86c4 7 24 7 28 0l2 8c-6 7-26 7-32 0z" fill="#16203a" />
	<path d="M55 74h22v14c-4 6-18 6-22 0z" fill="#b47d63" />
	<ellipse cx="44" cy="58" rx="4" ry="6" fill="#c48f74" />
	<ellipse cx="88" cy="58" rx="4" ry="6" fill="#c48f74" />
	<ellipse cx="66" cy="54" rx="22" ry="27" fill="url(#{id}-skin)" />
	<path d="M43 52c-2-20 9-31 24-31 15 0 25 10 22 30-2-9-7-14-13-15-8 4-20 5-31 3-1 4-2 8-2 13z" fill="#2b211d" />
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: 100%;
	}
</style>

<script lang="ts" module>
	export type Theme = 'light' | 'dark';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';

	/** The explicit choice; null follows the system appearance. */
	let { theme = $bindable(null) }: { theme?: Theme | null } = $props();

	const prefersDark = new MediaQuery('(prefers-color-scheme: dark)', false);
	let dark = $derived(theme ? theme === 'dark' : prefersDark.current);

	onMount(() => {
		const saved = document.documentElement.dataset.theme;
		if (saved === 'light' || saved === 'dark') theme = saved;
	});

	function toggle() {
		const next: Theme = dark ? 'light' : 'dark';
		theme = next === (prefersDark.current ? 'dark' : 'light') ? null : next;
		const root = document.documentElement;
		if (theme) root.dataset.theme = theme;
		else delete root.dataset.theme;
		try {
			if (theme) localStorage.setItem('theme', theme);
			else localStorage.removeItem('theme');
		} catch {}
	}
</script>

<button
	type="button"
	class="toggle"
	aria-label={dark ? 'Switch to light mode' : 'Switch to dark mode'}
	title={dark ? 'Switch to light mode' : 'Switch to dark mode'}
	onclick={toggle}
>
	<svg
		class="sun"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="4" />
		<path
			d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"
		/>
	</svg>
	<svg
		class="moon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
	</svg>
</button>

<style>
	.toggle {
		position: absolute;
		top: 24px;
		right: 0;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 38px;
		height: 38px;
		padding: 0;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--card);
		color: var(--muted);
		cursor: pointer;
		transition:
			color 0.15s,
			border-color 0.15s;
	}

	.toggle:hover {
		color: var(--fg);
	}

	svg {
		width: 18px;
		height: 18px;
	}

	.sun {
		display: none;
	}

	:global(:root[data-theme='dark']) .sun {
		display: block;
	}

	:global(:root[data-theme='dark']) .moon {
		display: none;
	}

	@media (prefers-color-scheme: dark) {
		:global(:root:not([data-theme='light'])) .sun {
			display: block;
		}

		:global(:root:not([data-theme='light'])) .moon {
			display: none;
		}
	}
</style>

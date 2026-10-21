<script lang="ts">
	import { resolve } from '$app/paths';
	import icon from '#lib/assets/icon.webp';
	import shareImage from '#lib/assets/og.jpg';
	import CameraCard from '#lib/cards/CameraCard.svelte';
	import FileCard from '#lib/cards/FileCard.svelte';
	import LightCard from '#lib/cards/LightCard.svelte';
	import LiveCard from '#lib/cards/LiveCard.svelte';
	import MenuBarCard from '#lib/cards/MenuBarCard.svelte';
	import SharpCard from '#lib/cards/SharpCard.svelte';
	import Showcase from '#lib/demo/Showcase.svelte';
	import { Player } from '#lib/demo/player.svelte.js';
	import { play } from '#lib/demo/sound.js';
	import DownloadCount from '#lib/DownloadCount.svelte';
	import { reveal } from '#lib/motion.svelte.js';
	import { theme } from '#lib/theme.svelte.js';
	import {
		ARCH,
		AUTHOR,
		AUTHOR_URL,
		AUTHOR_X,
		DESCRIPTION,
		MIN_MACOS,
		NAME,
		OG_IMAGE_ALT,
		REPO_URL,
		SITE_URL,
		TITLE,
		VERSION
	} from '#lib/site.js';

	const url = `${SITE_URL}/`;
	// Imported, so the URL carries a content hash and changes with the image. The browser resolves
	// the import against its own origin, so keep only the path.
	const image = `${SITE_URL}${new URL(shareImage, SITE_URL).pathname}`;
	const app = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': 'SoftwareApplication',
		name: NAME,
		description: DESCRIPTION,
		url,
		image,
		operatingSystem: `macOS ${MIN_MACOS} or later`,
		applicationCategory: 'MultimediaApplication',
		offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
		downloadUrl: `${SITE_URL}/download`,
		softwareVersion: VERSION,
		author: { '@type': 'Person', name: AUTHOR, url: AUTHOR_URL }
	}).replaceAll('<', '\\u003c');
	// A literal closing tag here would end this <script> block.
	const jsonLd = `<script type="application/ld+json">${app}<\/script>`;

	const player = new Player();

	function toggleTheme() {
		theme.toggle();
		if (player.sound) play('switch');
	}

	const recording = [
		{
			title: 'Your camera, anywhere',
			body: 'Drag the webcam anywhere in the preview, or let it snap to a corner. Make it a circle, squircle or pill with a border and shadow, and change it while you record.',
			Visual: CameraCard
		},
		{
			title: 'One file, ready to edit',
			body: 'Screen, system audio, mic and webcam land in one .mov or .mp4, with a mix track plus system audio and mic on their own tracks. H.264, HEVC or ProRes 422 at a constant frame rate.',
			Visual: FileCard
		}
	];

	const quiet = [
		{
			title: 'From any app',
			body: '⇧⌘R starts and stops recording from anywhere, even with Rec hidden. The menu bar shows a red dot and the time, and closing the window keeps Rec running.',
			Visual: MenuBarCard
		},
		{
			title: 'Replay and go live',
			body: 'Keep the last 15 to 300 seconds in memory and save them with ⇧⌘S. Stream to YouTube, Twitch or any RTMP or WHIP server, with or without recording.',
			Visual: LiveCard
		},
		{
			title: 'Sharp at any size',
			body: 'Rec captures your display at full resolution and scales it on the GPU with a Lanczos filter, so small text stays crisp at 1080p and 4K.',
			Visual: SharpCard
		},
		{
			title: 'Light on your Mac',
			body: 'A 4K60 screen recording takes about 6% of one core. A 30-minute test wrote all 108,000 frames, and a crash still leaves a playable file.',
			Visual: LightCard
		}
	];

	const shortcuts = [
		{ keys: ['⇧', '⌘', 'R'], label: 'Start or stop recording' },
		{ keys: ['⇧', '⌘', 'S'], label: 'Save the replay buffer' },
		{ keys: ['⌘', ','], label: 'Settings' },
		{ keys: ['⌘', '0'], label: 'Show the Rec window' },
		{ keys: ['⌘', 'W'], label: 'Close the window' },
		{ keys: ['⌘', 'Q'], label: 'Quit, after saving the file' }
	];

	const commands = [
		{ command: 'rec-rs record --cam -r 4k', label: 'Screen and webcam at 4K' },
		{ command: 'rec-rs record --window Slides', label: 'One window, found by its title' },
		{ command: 'rec-rs pause', label: 'Pause, with no gap in the file' },
		{ command: 'rec-rs stop', label: 'Finish and print the file' },
		{ command: 'rec-rs save-replay', label: 'Save the replay buffer' },
		{ command: 'rec-rs devices', label: 'List displays, windows, cameras and mics' }
	];

	const steps = [
		'Download Rec and drag it to your Applications folder.',
		'Open it and turn on Rec in System Settings → Privacy & Security → Screen & System Audio Recording.',
		'Allow the camera and mic when macOS asks. Then press ⇧⌘R.'
	];
</script>

<svelte:head>
	<title>{TITLE}</title>
	<meta name="description" content={DESCRIPTION} />
	<link rel="canonical" href={url} />
	<meta property="og:type" content="website" />
	<meta property="og:url" content={url} />
	<meta property="og:site_name" content={NAME} />
	<meta property="og:locale" content="en_US" />
	<meta property="og:title" content={TITLE} />
	<meta property="og:description" content={DESCRIPTION} />
	<meta property="og:image" content={image} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:image:type" content="image/jpeg" />
	<meta property="og:image:alt" content={OG_IMAGE_ALT} />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={TITLE} />
	<meta name="twitter:description" content={DESCRIPTION} />
	<meta name="twitter:image" content={image} />
	<meta name="twitter:image:alt" content={OG_IMAGE_ALT} />
	<meta name="twitter:creator" content={AUTHOR_X} />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html jsonLd}
</svelte:head>

{#snippet apple()}
	<svg viewBox="0 0 17 21" aria-hidden="true">
		<path
			d="M11.2 4.3c.6-.8 1-1.8.9-2.8-.9 0-2 .6-2.6 1.4-.6.7-1.1 1.7-.9 2.7 1 .1 2-.5 2.6-1.3zM12.1 5.8c-1.4-.1-2.7.8-3.3.8-.7 0-1.7-.8-2.9-.8C4.4 5.9 3 6.8 2.2 8.1c-1.6 2.7-.4 6.8 1.1 9 .8 1.1 1.7 2.3 2.8 2.3 1.1 0 1.6-.7 2.9-.7 1.4 0 1.8.7 2.9.7 1.2 0 2-1.1 2.7-2.2.9-1.3 1.2-2.5 1.2-2.6 0 0-2.4-.9-2.4-3.6 0-2.2 1.8-3.3 1.9-3.4-1-1.5-2.6-1.7-3.2-1.7z"
		/>
	</svg>
{/snippet}

{#snippet github()}
	<svg viewBox="0 0 16 16" aria-hidden="true">
		<path
			d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
		/>
	</svg>
{/snippet}

{#snippet card(item: (typeof recording)[number], i: number)}
	<article class="card" style:--delay="{i * 90}ms" {@attach reveal}>
		<item.Visual />
		<div class="caption">
			<h3>{item.title}</h3>
			<p>{item.body}</p>
		</div>
	</article>
{/snippet}

<nav class="nav">
	<div class="bar glass">
		<a class="brand" href="#top" aria-label="{NAME}, back to top">
			<img src={icon} width="26" height="26" alt="" />
			<span>{NAME}</span>
		</a>
		<div class="links">
			<a class="link" href="#features">Features</a>
			<a class="link" href="#shortcuts">Shortcuts</a>
			<a class="link" href="#install">Install</a>
			<a class="link" href={REPO_URL}>GitHub</a>
			<button
				type="button"
				class="theme"
				aria-label="Light mode"
				aria-pressed={theme.current === 'light'}
				onclick={toggleTheme}
			>
				<svg class="moon" viewBox="0 0 16 16" aria-hidden="true">
					<path d="M13.5 9.8A5.8 5.8 0 0 1 6.2 2.5a5.8 5.8 0 1 0 7.3 7.3z" />
				</svg>
				<svg class="sun" viewBox="0 0 16 16" aria-hidden="true">
					<circle cx="8" cy="8" r="3" />
					<path
						d="M8 1.5v1.6M8 12.9v1.6M1.5 8h1.6M12.9 8h1.6M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1"
					/>
				</svg>
			</button>
			<a class="pill" href={resolve('/download')} data-sveltekit-reload>Download</a>
		</div>
	</div>
</nav>

<main id="top">
	<header class="hero">
		<img class="logo" src={icon} width="112" height="112" alt="Rec app icon" />
		<h1>Hit record. That's it.</h1>
		<p class="lede">
			A fast, simple screen and camera recorder for the Mac, written in Rust. Your screen, system
			audio, mic and webcam go into one file, ready to edit.
		</p>
		<div class="cta">
			<a class="download" href={resolve('/download')} data-sveltekit-reload>
				{@render apple()}
				Download for macOS
			</a>
			<p class="meta">Free · Version {VERSION} · {ARCH} · macOS {MIN_MACOS} or later<DownloadCount repo={REPO_URL} /></p>
		</div>
	</header>

	<section class="showcase" aria-label="Demo">
		<Showcase {player} />
	</section>

	<div id="features">
		<section class="section" aria-labelledby="recording-title">
			<h2 id="recording-title" {@attach reveal}>Recording</h2>
			<div class="cards">
				{#each recording as item, i (item.title)}
					{@render card(item, i)}
				{/each}
			</div>
		</section>

		<section class="section" aria-labelledby="quiet-title">
			<h2 id="quiet-title" {@attach reveal}>Out of your way</h2>
			<div class="cards">
				{#each quiet as item, i (item.title)}
					{@render card(item, i % 2)}
				{/each}
			</div>
		</section>
	</div>

	<section id="shortcuts" class="section split" aria-labelledby="shortcuts-title">
		<div class="intro" {@attach reveal}>
			<h2 id="shortcuts-title">Shortcuts</h2>
			<p>⇧⌘R works in every app while Rec runs, and so does ⇧⌘S while the replay buffer is on.</p>
		</div>
		<ul class="shortcuts" {@attach reveal}>
			{#each shortcuts as shortcut (shortcut.label)}
				<li>
					<span class="keys">
						{#each shortcut.keys as key (key)}
							<kbd class={{ hot: key === 'R' }}>{key}</kbd>
						{/each}
					</span>
					<span class="label">{shortcut.label}</span>
				</li>
			{/each}
		</ul>
	</section>

	<section class="section split" aria-labelledby="cli-title">
		<div class="intro" {@attach reveal}>
			<h2 id="cli-title">Command line</h2>
			<p>
				Every control works from a shell too, with the rec-rs tool inside Rec.app. To run it by name, add
				/Applications/Rec.app/Contents/MacOS to your PATH.
			</p>
		</div>
		<ul class="commands" {@attach reveal}>
			{#each commands as row (row.command)}
				<li>
					<code><span aria-hidden="true">$</span>{row.command}</code>
					<span class="label">{row.label}</span>
				</li>
			{/each}
		</ul>
	</section>

	<section id="install" class="section split" aria-labelledby="install-title">
		<div class="intro" {@attach reveal}>
			<h2 id="install-title">Install</h2>
			<p>Free. macOS {MIN_MACOS} or later on {ARCH}.</p>
			<a class="download" href={resolve('/download')} data-sveltekit-reload>
				{@render apple()}
				Download for macOS
			</a>
		</div>
		<ol class="steps" {@attach reveal}>
			{#each steps as step, i (step)}
				<li><span class="num">{i + 1}</span>{step}</li>
			{/each}
		</ol>
	</section>
</main>

<footer class="footer">
	<div class="brand">
		<img src={icon} width="20" height="20" alt="" />
		<span>{NAME}</span>
	</div>
	<p>Free and open source under the MIT or Apache 2.0 license. Made by <a href={AUTHOR_URL}>{AUTHOR}</a>.</p>
	<a class="link" href={REPO_URL}>
		{@render github()}
		GitHub
	</a>
</footer>

<style>
	.glass {
		background: var(--glass);
		backdrop-filter: blur(20px) saturate(180%) brightness(106%);
		box-shadow: var(--glass-edge);
	}

	.nav {
		position: sticky;
		top: 0;
		z-index: 10;
		max-width: 1200px;
		margin: 0 auto;
		padding: 8px 24px;
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 8px 8px 16px;
		border-radius: 999px;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 9px;
		font: 600 15px var(--sans);
		letter-spacing: -0.01em;
		text-decoration: none;
	}

	.links {
		display: flex;
		align-items: center;
		gap: 22px;
	}

	.link {
		display: flex;
		align-items: center;
		gap: 7px;
		color: var(--muted);
		font-size: 14px;
		text-decoration: none;
		transition: color 0.15s;
	}

	.link:hover {
		color: var(--fg);
	}

	.link svg {
		width: 15px;
		height: 15px;
		fill: currentColor;
	}

	.theme {
		display: grid;
		place-items: center;
		width: 32px;
		height: 32px;
		margin-inline: -6px;
		padding: 0;
		border: 0;
		border-radius: 999px;
		background: none;
		color: var(--muted);
		cursor: pointer;
		transition:
			color 0.15s,
			background-color 0.15s;
	}

	.theme:hover {
		background: color-mix(in srgb, var(--fg) 8%, transparent);
		color: var(--fg);
	}

	.theme svg {
		grid-area: 1 / 1;
		width: 16px;
		height: 16px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
		stroke-linecap: round;
		stroke-linejoin: round;
		transition:
			opacity 0.25s,
			rotate 0.5s cubic-bezier(0.3, 1.4, 0.5, 1),
			scale 0.5s cubic-bezier(0.3, 1.4, 0.5, 1);
	}

	.sun,
	:global(:root[data-theme='light']) .moon {
		opacity: 0;
		rotate: -90deg;
		scale: 0.5;
	}

	:global(:root[data-theme='light']) .sun {
		opacity: 1;
		rotate: 0deg;
		scale: 1;
	}

	.pill {
		padding: 7px 14px;
		border-radius: 999px;
		background: var(--raised);
		box-shadow: var(--raised-edge);
		font: 500 14px var(--sans);
		text-decoration: none;
		transition: background-color 0.15s;
	}

	.pill:hover {
		background-color: color-mix(in srgb, var(--fg) 8%, transparent);
	}

	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		max-width: 900px;
		margin: 0 auto;
		padding: 72px 24px 48px;
		text-align: center;
	}

	.hero > * {
		animation: rise 0.9s cubic-bezier(0.2, 0.7, 0.2, 1) both;
	}

	.hero > :nth-child(2) {
		animation-delay: 0.08s;
	}

	.hero > :nth-child(3) {
		animation-delay: 0.16s;
	}

	.hero > :nth-child(4) {
		animation-delay: 0.24s;
	}

	.logo {
		filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.55)) drop-shadow(0 0 40px rgba(255, 59, 48, 0.14));
		animation-name: pop;
	}

	:global(:root[data-theme='light']) .logo {
		filter: drop-shadow(0 18px 32px rgba(0, 0, 0, 0.22)) drop-shadow(0 0 40px rgba(255, 59, 48, 0.1));
	}

	h1 {
		margin: 32px 0 0;
		font: 700 58px/1.05 var(--display);
		letter-spacing: -0.035em;
		text-wrap: balance;
	}

	.lede {
		max-width: 600px;
		margin: 20px 0 0;
		color: var(--muted);
		font-size: 18px;
		line-height: 29px;
		text-wrap: pretty;
	}

	.cta {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		margin-top: 32px;
	}

	.download {
		display: inline-flex;
		align-items: center;
		gap: 9px;
		padding: 13px 22px 13px 20px;
		border-radius: 999px;
		background: var(--solid);
		box-shadow: 0 10px 30px rgba(255, 255, 255, 0.07);
		color: var(--on-solid);
		font: 600 15px/18px var(--sans);
		text-decoration: none;
		transition:
			scale 0.2s,
			background-color 0.2s;
	}

	:global(:root[data-theme='light']) .download {
		box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14);
	}

	.download:hover {
		background: var(--solid-hover);
		scale: 1.03;
	}

	.download svg {
		width: 15px;
		height: 18px;
		fill: currentColor;
	}

	.meta {
		margin: 0;
		color: var(--muted);
		font-size: 13px;
	}

	.showcase {
		max-width: 1248px;
		margin: 0 auto;
		padding: 8px 24px 72px;
		animation: rise 1s 0.32s cubic-bezier(0.2, 0.7, 0.2, 1) both;
	}

	.section {
		max-width: 1080px;
		margin: 0 auto;
		padding: 56px 24px;
		border-top: 1px solid var(--line);
	}

	h2 {
		margin: 0 0 24px;
		font: 600 22px/28px var(--display);
		letter-spacing: -0.02em;
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 20px;
	}

	.card {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border-radius: 16px;
		background: var(--card);
		box-shadow: var(--card-edge);
	}

	.caption {
		padding: 20px 22px 24px;
	}

	h3 {
		margin: 0;
		font: 600 16px/20px var(--sans);
		letter-spacing: -0.01em;
	}

	.caption p {
		margin: 6px 0 0;
		color: var(--muted);
		font-size: 14px;
		line-height: 22px;
	}

	.split {
		display: grid;
		grid-template-columns: 300px minmax(0, 1fr);
		gap: 40px;
	}

	.intro h2 {
		margin-bottom: 10px;
	}

	.intro p {
		max-width: 260px;
		margin: 0;
		color: var(--muted);
		font-size: 14px;
		line-height: 22px;
	}

	#install .intro p {
		max-width: none;
	}

	.intro .download {
		margin-top: 22px;
	}

	ul,
	ol {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.shortcuts {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0 32px;
	}

	.shortcuts li {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 9px 0;
	}

	.keys {
		display: flex;
		flex-shrink: 0;
		gap: 4px;
		width: 84px;
	}

	kbd {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 24px;
		height: 24px;
		padding: 0 6px;
		border-radius: 6px;
		background: linear-gradient(#2c2c2f, #222225);
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.1),
			0 1px 0 #000;
		color: #d1d1d6;
		font: 500 12px/14px var(--sans);
	}

	kbd.hot {
		background: linear-gradient(#fff, #f2f2f6 55%, #e6e6ec);
		box-shadow:
			inset 0 1px 0 #fff,
			0 1px 0 #000;
		color: #1c1c1e;
		font-weight: 700;
	}

	:global(:root[data-theme='light']) kbd {
		background: linear-gradient(#fff, #f2f2f4);
		box-shadow:
			inset 0 0 0 0.5px rgba(0, 0, 0, 0.12),
			0 1px 0 rgba(0, 0, 0, 0.16);
		color: #3a3a3c;
	}

	:global(:root[data-theme='light']) kbd.hot {
		background: linear-gradient(#48484a, #2c2c2e 55%, #1c1c1e);
		box-shadow:
			inset 0 1px 0 rgba(255, 255, 255, 0.18),
			0 1px 0 rgba(0, 0, 0, 0.3);
		color: #fff;
	}

	.label {
		color: var(--muted);
		font-size: 14px;
		line-height: 18px;
	}

	.commands {
		padding: 8px 22px;
		border-radius: 16px;
		background: var(--card);
		box-shadow: var(--card-edge);
	}

	.commands li {
		display: flex;
		align-items: center;
		gap: 16px;
		min-height: 40px;
		padding: 8px 0;
	}

	.commands li + li {
		border-top: 1px solid var(--line);
	}

	code {
		display: flex;
		gap: 8px;
		width: 290px;
		flex-shrink: 0;
		font: 13px/18px var(--mono);
		white-space: pre;
	}

	code span {
		color: color-mix(in srgb, var(--muted) 70%, transparent);
	}

	.steps {
		display: flex;
		flex-direction: column;
	}

	.steps li {
		display: flex;
		gap: 14px;
		padding: 9px 0;
		color: var(--muted);
		font-size: 14px;
		line-height: 22px;
	}

	.num {
		display: grid;
		place-items: center;
		width: 24px;
		height: 24px;
		flex-shrink: 0;
		border-radius: 50%;
		background: color-mix(in srgb, var(--fg) 7%, transparent);
		color: var(--fg);
		font: 11px/14px var(--mono);
	}

	.footer {
		display: flex;
		align-items: center;
		gap: 24px;
		max-width: 1080px;
		margin: 0 auto;
		padding: 32px 24px 48px;
		border-top: 1px solid var(--line);
	}

	.footer .brand {
		font-size: 14px;
	}

	.footer p {
		flex: 1;
		margin: 0;
		color: var(--muted);
		font-size: 13px;
	}

	.footer p a {
		text-underline-offset: 2px;
	}

	.footer p a:hover {
		color: var(--fg);
	}

	@keyframes rise {
		from {
			opacity: 0;
			translate: 0 16px;
		}
	}

	@keyframes pop {
		from {
			opacity: 0;
			scale: 0.86;
		}
	}

	@media (max-width: 860px) {
		.cards,
		.split {
			grid-template-columns: minmax(0, 1fr);
		}

		.split {
			gap: 28px;
		}
	}

	@media (max-width: 640px) {
		.links .link {
			display: none;
		}

		.hero {
			padding-top: 48px;
		}

		h1 {
			font-size: 40px;
		}

		.lede {
			font-size: 16px;
			line-height: 25px;
		}

		.showcase {
			padding-inline: 16px;
		}

		.section {
			padding-inline: 16px;
		}

		.shortcuts {
			grid-template-columns: minmax(0, 1fr);
		}

		.commands {
			padding-inline: 16px;
		}

		.commands li {
			flex-direction: column;
			align-items: flex-start;
			gap: 2px;
		}

		code {
			width: auto;
		}

		.footer {
			flex-wrap: wrap;
			gap: 12px 24px;
		}

		.footer p {
			flex-basis: 100%;
			order: 3;
		}
	}
</style>

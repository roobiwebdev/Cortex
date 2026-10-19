<script lang="ts">
	import icon from '#lib/assets/icon.webp';
	import darkLarge from '#lib/assets/rec-dark-2160.webp';
	import darkSmall from '#lib/assets/rec-dark-1200.webp';
	import lightLarge from '#lib/assets/rec-light-2160.webp';
	import lightSmall from '#lib/assets/rec-light-1200.webp';
	import ThemeToggle, { type Theme } from '#lib/ThemeToggle.svelte';
	import { DESCRIPTION, NAME, REPO_URL, SITE_URL, TITLE, VERSION } from '#lib/site.ts';

	const pageUrl = `${SITE_URL}/`;
	const ogImage = `${SITE_URL}/og.jpg`;
	const ogImageAlt =
		'The Rec icon and name above "Hit record. That\'s it." and "A simple screen and camera recorder for the Mac", beside a screenshot of the Rec window recording a slide.';
	const jsonLd = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': 'SoftwareApplication',
		name: NAME,
		description: DESCRIPTION,
		url: pageUrl,
		image: ogImage,
		operatingSystem: 'macOS',
		applicationCategory: 'MultimediaApplication',
		offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
		downloadUrl: `${SITE_URL}/download`,
		softwareVersion: VERSION
	}).replaceAll('<', '\\u003c');
	// `<\/script>`: a literal closing tag would end this component's <script> block.
	const jsonLdTag = `<script type="application/ld+json">${jsonLd}<\/script>`;

	let theme = $state<Theme | null>(null);

	let darkMedia = $derived(
		theme === null ? '(prefers-color-scheme: dark)' : theme === 'dark' ? 'all' : 'not all'
	);

	const sizes = '(max-width: 1080px) calc(100vw - 68px), 992px';
	const shotAlt =
		'The Rec window: a live preview of a slide being recorded, with the scene picker, screen, camera, audio and output settings beside it, and the record button and level meters below.';
</script>

<svelte:head>
	<title>{TITLE}</title>
	<meta name="description" content={DESCRIPTION} />
	<link rel="canonical" href={pageUrl} />
	<meta property="og:type" content="website" />
	<meta property="og:url" content={pageUrl} />
	<meta property="og:site_name" content={NAME} />
	<meta property="og:locale" content="en_US" />
	<meta property="og:title" content={TITLE} />
	<meta property="og:description" content={DESCRIPTION} />
	<meta property="og:image" content={ogImage} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:image:type" content="image/jpeg" />
	<meta property="og:image:alt" content={ogImageAlt} />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={TITLE} />
	<meta name="twitter:description" content={DESCRIPTION} />
	<meta name="twitter:image" content={ogImage} />
	<meta name="twitter:image:alt" content={ogImageAlt} />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html jsonLdTag}
</svelte:head>

<div class="wrap">
	<header>
		<ThemeToggle bind:theme />
		<img class="icon" src={icon} width="112" height="112" alt="Rec app icon" />
		<h1>Hit record. That's it.</h1>
		<p class="lede">
			A simple alternative to OBS for your Mac that records your screen, system audio, mic and
			webcam into one file, ready for Final Cut, Premiere or Resolve.
		</p>
		<a class="btn" href="/download" data-sveltekit-reload>
			<svg
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2.2"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<path d="M12 3v12M6 11l6 6 6-6M5 21h14" />
			</svg>
			Download for Mac
		</a>
		<p class="meta">
			<span>Version {VERSION}</span> · <span>Apple Silicon</span> · <span>macOS 15 or later</span> ·
			<span>Free</span>
		</p>
	</header>

	<div class="frame">
		<span class="rec" aria-hidden="true">REC</span>
		<picture>
			<source
				type="image/webp"
				media={darkMedia}
				srcset="{darkSmall} 1200w, {darkLarge} 2160w"
				{sizes}
			/>
			<!-- Must stay between the <source> and the <img>: it runs during parsing, before the
			     browser picks an image, so a saved theme that differs from the OS gets the
			     matching screenshot before hydration. -->
			<script>
				if (document.documentElement.dataset.theme)
					document.currentScript.previousElementSibling.media =
						document.documentElement.dataset.theme === 'dark' ? 'all' : 'not all';
			</script>
			<img
				src={lightLarge}
				srcset="{lightSmall} 1200w, {lightLarge} 2160w"
				{sizes}
				width="2160"
				height="1440"
				alt={shotAlt}
				fetchpriority="high"
			/>
		</picture>
	</div>

	<section class="features">
		<div class="f">
			<h2>One file, ready to edit</h2>
			<p>
				Screen, system audio, mic and webcam in one .mov or .mp4, with the mic and system audio on
				their own tracks. H.264, HEVC or ProRes, ready for Final Cut, Premiere and Resolve.
			</p>
		</div>
		<div class="f">
			<h2>Sharp text</h2>
			<p>
				Rec captures your display at full resolution and scales it on the GPU with a Lanczos
				filter, so small UI text stays crisp at 1080p and 4K.
			</p>
		</div>
		<div class="f">
			<h2>Your camera, anywhere</h2>
			<p>
				Drag the webcam anywhere or snap it to a corner. Make it a circle, squircle or pill, add a
				border and shadow, and change it live while you record.
			</p>
		</div>
		<div class="f">
			<h2>Out of your way</h2>
			<p>
				<kbd>⇧⌘R</kbd> starts and stops from any app. The menu bar shows a red dot and the time, and
				pausing leaves no gap in the file.
			</p>
		</div>
		<div class="f">
			<h2>Replay and go live</h2>
			<p>
				Keep the last 15 to 300 seconds and save them with <kbd>⇧⌘S</kbd>. Stream to YouTube, Twitch
				or any RTMP or WHIP server, with or without recording.
			</p>
		</div>
		<div class="f">
			<h2>Light and reliable</h2>
			<p>
				Recording your screen at 4K60 takes about 6% of one core. A 30-minute test wrote all
				108,000 frames, and a crash still leaves a playable file.
			</p>
		</div>
	</section>

	<footer>
		Requires macOS 15 or later on Apple Silicon. <a href={REPO_URL}>Source on GitHub</a>. Made by
		<a href="https://rasmic.xyz">Micky</a>.
	</footer>
</div>

<style>
	.wrap {
		max-width: 1080px;
		margin: 0 auto;
		padding: 0 24px;
	}

	header {
		position: relative;
		padding: 88px 0 48px;
		text-align: center;
	}

	.icon {
		display: block;
		width: 112px;
		height: 112px;
		margin: 0 auto;
		filter: drop-shadow(0 12px 28px rgb(0 0 0 / 0.22));
	}

	h1 {
		margin: 20px 0 14px;
		font-size: clamp(40px, 7vw, 64px);
		font-weight: 600;
		line-height: 1.04;
		letter-spacing: -0.04em;
		text-wrap: balance;
	}

	.lede {
		max-width: 580px;
		margin: 0 auto 32px;
		color: var(--muted);
		font-size: 19px;
		text-wrap: balance;
	}

	.btn {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		padding: 14px 26px;
		border-radius: 12px;
		background: var(--accent);
		color: #fff;
		font-size: 16px;
		font-weight: 600;
		text-decoration: none;
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.18),
			0 6px 18px rgb(217 44 38 / 0.3);
		transition:
			transform 0.15s,
			box-shadow 0.15s,
			background-color 0.15s;
	}

	.btn:hover {
		background: var(--accent-hover);
		transform: translateY(-1px);
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.18),
			0 10px 26px rgb(217 44 38 / 0.38);
	}

	.btn svg {
		width: 18px;
		height: 18px;
	}

	.meta {
		margin: 16px 0 0;
		color: var(--muted);
		font: 13px/1.6 var(--mono);
		text-wrap: balance;
	}

	.meta span {
		white-space: nowrap;
	}

	.frame {
		--corner: color-mix(in srgb, var(--muted) 55%, transparent);
		--arm: 22px;
		position: relative;
		padding: 20px;
		background:
			linear-gradient(var(--corner) 0 0) top left / var(--arm) 1.5px,
			linear-gradient(var(--corner) 0 0) top left / 1.5px var(--arm),
			linear-gradient(var(--corner) 0 0) top right / var(--arm) 1.5px,
			linear-gradient(var(--corner) 0 0) top right / 1.5px var(--arm),
			linear-gradient(var(--corner) 0 0) bottom left / var(--arm) 1.5px,
			linear-gradient(var(--corner) 0 0) bottom left / 1.5px var(--arm),
			linear-gradient(var(--corner) 0 0) bottom right / var(--arm) 1.5px,
			linear-gradient(var(--corner) 0 0) bottom right / 1.5px var(--arm);
		background-repeat: no-repeat;
	}

	.rec {
		position: absolute;
		top: -5px;
		left: calc(var(--arm) + 10px);
		display: inline-flex;
		align-items: center;
		gap: 7px;
		color: var(--muted);
		font: 500 11px/1 var(--mono);
		letter-spacing: 0.12em;
	}

	.rec::before {
		content: '';
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--rec);
		animation: blink 1.6s steps(1, end) infinite;
	}

	@keyframes blink {
		50% {
			opacity: 0.25;
		}
	}

	picture img {
		display: block;
		width: 100%;
		height: auto;
		border-radius: 2.17% / 3.25%;
		box-shadow:
			0 0 0 1px color-mix(in srgb, var(--fg) 9%, transparent),
			0 30px 80px var(--shadow);
	}

	.features {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 16px;
		padding: 80px 0 24px;
	}

	.f {
		padding: 22px 22px 24px;
		border: 1px solid var(--line);
		border-radius: 14px;
		background: var(--card);
	}

	.f h2 {
		margin: 0 0 6px;
		font-size: 17px;
		font-weight: 600;
		line-height: 1.3;
		letter-spacing: -0.015em;
	}

	.f p {
		margin: 0;
		color: var(--muted);
		font-size: 15px;
		text-wrap: pretty;
	}

	kbd {
		padding: 1px 5px;
		border: 1px solid var(--line);
		border-bottom-width: 2px;
		border-radius: 5px;
		background: var(--bg);
		color: var(--fg);
		font:
			500 12.5px/1.4 -apple-system,
			BlinkMacSystemFont,
			var(--sans);
		white-space: nowrap;
	}

	footer {
		padding: 48px 0 64px;
		color: var(--muted);
		font-size: 13px;
		text-align: center;
	}

	footer a {
		text-underline-offset: 2px;
	}

	footer a:hover {
		color: var(--fg);
	}

	@media (max-width: 900px) {
		.features {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 600px) {
		header {
			padding: 76px 0 40px;
		}

		.lede {
			font-size: 17px;
		}

		.frame {
			--arm: 14px;
			padding: 10px;
		}

		.rec {
			display: none;
		}

		.features {
			grid-template-columns: 1fr;
			padding-top: 56px;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.btn {
			transition: none;
		}

		.btn:hover {
			transform: none;
		}

		.rec::before {
			animation: none;
		}
	}
</style>

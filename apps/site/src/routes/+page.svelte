<script lang="ts">
	import { onMount, type Component } from 'svelte';
	import icon from '#lib/assets/icon.webp';
	import shareImage from '#lib/assets/og.jpg';
	import AreaCard from '#lib/cards/AreaCard.svelte';
	import CameraCard from '#lib/cards/CameraCard.svelte';
	import CameraTrackCard from '#lib/cards/CameraTrackCard.svelte';
	import FileCard from '#lib/cards/FileCard.svelte';
	import FourKCard from '#lib/cards/FourKCard.svelte';
	import LiveCard from '#lib/cards/LiveCard.svelte';
	import MenuBarCard from '#lib/cards/MenuBarCard.svelte';
	import SafeCard from '#lib/cards/SafeCard.svelte';
	import Showcase from '#lib/demo/Showcase.svelte';
	import { Player } from '#lib/demo/player.svelte.js';
	import { play } from '#lib/demo/sound.js';
	import { reveal } from '#lib/motion.svelte.js';
	import { detectLinuxArch, detectPlatform, type Platform } from '#lib/platform.js';
	import { theme } from '#lib/theme.svelte.js';
	import {
		ARCH,
		AUTHOR,
		AUTHOR_URL,
		BETA,
		DESCRIPTION,
		DOWNLOADS,
		FEATURES,
		GITHUB_URL,
		LICENSES_PATH,
		LINUX_ARCH_LABEL,
		LINUX_DISTROS,
		MIN_MACOS,
		NAME,
		NOTARIZED,
		OG_IMAGE_ALT,
		REQUIREMENTS,
		SITE_URL,
		TITLE,
		VERSION,
		WINDOWS_SIGNED,
		type Download,
		type DownloadOs,
		type LinuxArch
	} from '#lib/site.js';

	const url = `${SITE_URL}/`;
	// Imported, so the URL carries a content hash and changes with the image. The browser resolves
	// the import against its own origin, so keep only the path.
	const image = `${SITE_URL}${new URL(shareImage, SITE_URL).pathname}`;
	// No price or offer: pricing isn't decided, and the page makes no claim about it.
	const app = JSON.stringify({
		'@context': 'https://schema.org',
		'@type': 'SoftwareApplication',
		name: NAME,
		description: DESCRIPTION,
		url,
		image,
		operatingSystem: `macOS ${MIN_MACOS} or later, Windows 10 (version 2004) or 11, Linux`,
		applicationCategory: 'MultimediaApplication',
		downloadUrl: `${SITE_URL}/download`,
		softwareVersion: VERSION,
		author: { '@type': 'Person', name: AUTHOR, url: AUTHOR_URL }
	}).replaceAll('<', '\\u003c');
	// A literal closing tag here would end this <script> block, so it is split in two.
	const jsonLd = `<script type="application/ld+json">${app}<` + `/script>`;

	const player = new Player();

	function toggleTheme() {
		theme.toggle();
		if (player.sound) play('switch');
	}

	// Prerendered for macOS; once the page loads, the hero and the install tabs switch to the
	// visitor's own system (and, on Linux, CPU).
	let platform = $state<Platform>('mac');
	let linuxArch = $state<LinuxArch>('x86_64');
	let tab = $state<DownloadOs>('mac');
	onMount(() => {
		platform = detectPlatform();
		if (platform !== 'other') tab = platform;
		void detectLinuxArch().then((arch) => (linuxArch = arch));
	});

	const OSES: readonly DownloadOs[] = ['mac', 'windows', 'linux'];
	const OS_LABEL: Record<DownloadOs, string> = { mac: 'macOS', windows: 'Windows', linux: 'Linux' };
	const LINUX_ARCHES: readonly LinuxArch[] = ['x86_64', 'aarch64'];
	const isBeta = (os: DownloadOs) => os !== 'mac' && BETA[os];

	const mac = DOWNLOADS.mac.universal;
	const win = DOWNLOADS.windows.x86_64;
	let linux = $derived(DOWNLOADS.linux[linuxArch]);

	/** The visitor's own primary installer, for the nav pill. */
	let primary = $derived<Download | null>(
		platform === 'windows' ? win.exe : platform === 'linux' ? linux.appimage : platform === 'mac' ? mac.dmg : null
	);

	// Windows and Linux use Ctrl+Shift where macOS uses ⇧⌘ (the engines register exactly these).
	let pc = $derived(platform === 'windows' || platform === 'linux');
	let recordKeys = $derived(pc ? 'Ctrl+Shift+R' : '⇧⌘R');

	/** A card's caveat for the visitor's platform, when that platform does less. */
	type Limits = Partial<Record<'windows' | 'linux', string>>;
	interface Card {
		title: string;
		body: string;
		Visual: Component;
		limits?: Limits;
	}
	const limitFor = (limits: Limits | undefined) =>
		platform === 'windows' || platform === 'linux' ? limits?.[platform] : undefined;

	let recording = $derived<Card[]>([
		{
			title: 'Put your camera anywhere',
			body: 'Drag the camera around the preview and let go near a corner to snap it there. Choose one of six shapes, then set its size, roundness, border and shadow, and mirror it if you like.',
			Visual: CameraCard
		},
		{
			title: 'Record exactly what you mean',
			body: 'A whole display, a single window, every window of one app, or an area you drag out. Cortex leaves its own windows and controls out of the picture.',
			Visual: AreaCard,
			limits: {
				windows: 'On Windows, choosing an app records its largest window.',
				linux: 'On Linux you pick the screen or window in the system dialog, there is no app capture, and Cortex’s own windows can appear in the video.'
			}
		},
		{
			title: 'Everything in one file',
			body: 'Screen, system audio, mic and camera are written to one .mov or .mp4: a mixed audio track, then system audio and mic on tracks of their own. H.264, HEVC or ProRes, at a constant frame rate.',
			Visual: FileCard,
			limits: {
				windows: 'Windows writes .mp4 with H.264, or HEVC when the PC has an encoder.',
				linux: 'Linux writes .mp4 or .mov with H.264, or HEVC when an encoder is installed.'
			}
		},
		{
			title: 'The camera on its own track',
			body: 'Turn on the camera track and the file also keeps the raw camera next to the screen, so an editor can reframe it later. With .mp4 it is saved beside the file as a .camera.mov.',
			Visual: CameraTrackCard,
			limits: {
				windows: 'Not on Windows yet.',
				linux: 'On Linux the camera is a second video track inside the file.'
			}
		}
	]);

	let quiet = $derived<Card[]>([
		{
			title: 'Start from anywhere',
			body: pc
				? `${recordKeys} starts and stops a recording from any app. A small floating bar keeps the time, and closing the window leaves ${NAME} in the system tray.`
				: '⇧⌘R starts and stops a recording from any app. A small floating bar keeps the time without showing up in the file, and closing the window leaves Cortex in the menu bar.',
			Visual: MenuBarCard,
			limits: { linux: 'On Linux the shortcut needs a desktop with the GlobalShortcuts portal.' }
		},
		{
			title: 'Save the last minute, or stream it',
			body: 'Keep the last 15 seconds to 5 minutes in memory and save them with ⇧⌘S. Go live to YouTube, Twitch, Kick or your own RTMP, RTMPS or WHIP server, with stream keys kept in the Keychain.',
			Visual: LiveCard,
			limits: { windows: 'Mac only for now.', linux: 'Mac only for now.' }
		},
		{
			title: 'True 4K',
			body: 'With HEVC, the default above 1080p, 4K means 3840 pixels across at your screen’s own shape: a MacBook Pro display records at 3840 × 2482, with no black bars. The GPU scales it with a Lanczos filter.',
			Visual: FourKCard
		},
		{
			title: 'Built to keep every take',
			body: 'A 10-minute 4K60 test kept all 36,000 frames. Cortex writes the file in 2-second pieces, so a crash still leaves it playable, and quitting saves the recording first.',
			Visual: SafeCard,
			limits: { windows: 'On Windows a crash can still lose the file for now.' }
		}
	]);

	let shortcuts = $derived(
		pc
			? [
					{ keys: ['Ctrl', 'Shift', 'R'], label: 'Start or stop recording' },
					{ keys: ['Esc'], label: 'Cancel the countdown' },
					{ keys: ['Ctrl', ','], label: 'Settings' }
				]
			: [
					{ keys: ['⇧', '⌘', 'R'], label: 'Start or stop recording' },
					{ keys: ['⇧', '⌘', 'S'], label: 'Save the replay buffer' },
					{ keys: ['esc'], label: 'Cancel the countdown' },
					{ keys: ['⌘', ','], label: 'Settings' },
					{ keys: ['⌘', 'Q'], label: 'Quit, after saving the file' }
				]
	);

	const trimKeys = [
		{ keys: ['Space'], label: 'Play or pause' },
		{ keys: ['J', 'K', 'L'], label: 'Play backwards, pause, play forwards' },
		{ keys: ['I', 'O'], label: 'Set the start and the end' },
		{ keys: ['←', '→'], label: 'Step one frame, or one second with ⇧' },
		{ keys: ['Home', 'End'], label: 'Jump to the start or the end' }
	];

	/** One install step: plain text, plus commands to copy. */
	interface Step {
		text: string;
		code?: readonly string[];
	}

	// TODO(notarize): with NOTARIZED = true in site.ts, the Gatekeeper step becomes a plain "open it".
	const macSteps: readonly Step[] = [
		{ text: `Open ${mac.dmg.file} and drag ${NAME} to your Applications folder.` },
		NOTARIZED
			? { text: `Open ${NAME} from your Applications folder.` }
			: {
					text: `The first time, macOS says “Apple could not verify “${NAME}” is free of malware”, because ${NAME} isn’t notarized yet. Click Done, then open System Settings › Privacy & Security, scroll down and click Open Anyway.`
				},
		{
			text: `Allow Screen Recording when ${NAME} asks; the guided tour walks you through it. Then press ⇧⌘R to record.`
		}
	];

	// TODO(sign): with WINDOWS_SIGNED = true in site.ts, the SmartScreen sentence disappears.
	const windowsSteps: readonly Step[] = [
		{
			text: WINDOWS_SIGNED
				? `Run ${win.exe.file}.`
				: `Run ${win.exe.file}. If Windows says “Windows protected your PC”, click More info, then Run anyway. The installer isn’t code-signed yet.`
		},
		{
			text: `It installs for your user only, so it needs no admin rights. If WebView2 is missing, the installer adds it.`
		},
		{
			text: `If ${NAME} asks for the camera or mic, turn them on in Settings › Privacy & security › Camera and Microphone, including “Let desktop apps access”. Then press Ctrl+Shift+R to record.`
		}
	];

	let linuxSteps = $derived<readonly Step[]>([
		{
			text: 'AppImage: download it, make it executable and run it. It uses your system’s GStreamer plugins.',
			code: [`chmod +x ${linux.appimage.file} && ./${linux.appimage.file}`]
		},
		{
			text: 'Or install the package for your distribution, which also installs GStreamer:',
			code: [`sudo apt install ./${linux.deb.file}`, `sudo dnf install ./${linux.rpm.file}`]
		},
		{
			text: 'When you start recording, pick the screen or window in the system dialog and click Share. Screen sharing needs PipeWire, which current Ubuntu, Fedora and Debian desktops include.'
		}
	]);

	let steps = $derived<Record<DownloadOs, readonly Step[]>>({
		mac: macSteps,
		windows: windowsSteps,
		linux: linuxSteps
	});

	let copied = $state<string | null>(null);
	async function copy(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = text;
			setTimeout(() => {
				if (copied === text) copied = null;
			}, 1600);
		} catch {
			// Clipboard blocked: the command stays selectable.
		}
	}

	/** Arrow keys move between the OS tabs, as the WAI-ARIA tabs pattern expects. */
	function tabKey(event: KeyboardEvent) {
		const i = OSES.indexOf(tab);
		const next =
			event.key === 'ArrowRight'
				? OSES[(i + 1) % OSES.length]
				: event.key === 'ArrowLeft'
					? OSES[(i + OSES.length - 1) % OSES.length]
					: undefined;
		if (!next) return;
		event.preventDefault();
		tab = next;
		document.getElementById(`tab-${next}`)?.focus();
	}

	const mark = (v: boolean | string) => (v === true ? 'Yes' : v === false ? 'Not yet' : v);
</script>

<svelte:head>
	<title>{TITLE}</title>
	<meta name="description" content={DESCRIPTION} />
	<meta name="author" content={AUTHOR} />
	<link rel="author" href={AUTHOR_URL} />
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
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html jsonLd}
</svelte:head>

{#snippet osIcon(os: DownloadOs)}
	{#if os === 'mac'}
		<svg viewBox="0 0 17 21" aria-hidden="true">
			<path
				d="M11.2 4.3c.6-.8 1-1.8.9-2.8-.9 0-2 .6-2.6 1.4-.6.7-1.1 1.7-.9 2.7 1 .1 2-.5 2.6-1.3zM12.1 5.8c-1.4-.1-2.7.8-3.3.8-.7 0-1.7-.8-2.9-.8C4.4 5.9 3 6.8 2.2 8.1c-1.6 2.7-.4 6.8 1.1 9 .8 1.1 1.7 2.3 2.8 2.3 1.1 0 1.6-.7 2.9-.7 1.4 0 1.8.7 2.9.7 1.2 0 2-1.1 2.7-2.2.9-1.3 1.2-2.5 1.2-2.6 0 0-2.4-.9-2.4-3.6 0-2.2 1.8-3.3 1.9-3.4-1-1.5-2.6-1.7-3.2-1.7z"
			/>
		</svg>
	{:else if os === 'windows'}
		<svg viewBox="0 0 16 16" aria-hidden="true">
			<path d="M0 2.2 6.5 1.3v6.3H0zM7.3 1.2 16 0v7.6H7.3zM0 8.4h6.5v6.3L0 13.8zM7.3 8.4H16V16l-8.7-1.2z" />
		</svg>
	{:else}
		<svg viewBox="0 0 16 16" aria-hidden="true">
			<path d="M7.2 1h1.6v8.1l2.7-2.7 1.1 1.1L8 12.1 3.4 7.5l1.1-1.1 2.7 2.7zM2 13.4h12V15H2z" />
		</svg>
	{/if}
{/snippet}

{#snippet github()}
	<svg viewBox="0 0 16 16" aria-hidden="true">
		<path
			d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
		/>
	</svg>
{/snippet}

{#snippet heroDownload()}
	{#if platform === 'windows'}
		<a class="download" href={win.exe.url}>{@render osIcon('windows')} Download for Windows</a>
		<p class="meta">
			Version {VERSION} · <span class="beta">Beta</span> · Windows 11, or Windows 10 (version 2004 or newer), 64-bit · or the
			<a href={win.msi.url}>.msi</a>
		</p>
	{:else if platform === 'linux'}
		<a class="download" href={linux.appimage.url}>{@render osIcon('linux')} Download AppImage</a>
		<p class="meta">
			Version {VERSION} · <span class="beta">Beta</span> · {LINUX_ARCH_LABEL[linuxArch]} · {LINUX_DISTROS} · or the
			<a href={linux.deb.url}>.deb</a> · <a href={linux.rpm.url}>.rpm</a>
		</p>
	{:else if platform === 'mac'}
		<a class="download" href={mac.dmg.url}>{@render osIcon('mac')} Download for macOS</a>
		<p class="meta">Version {VERSION} · {ARCH} · macOS {MIN_MACOS} or later</p>
	{:else}
		<a class="download" href="#install">See downloads</a>
		<p class="meta">{NAME} runs on Mac, Windows and Linux computers.</p>
	{/if}
	{#if platform !== 'other'}
		<a class="other" href="#install">Other platforms</a>
	{/if}
{/snippet}

{#snippet card(item: Card, i: number)}
	{@const limit = limitFor(item.limits)}
	<article class="card" style:--delay="{i * 90}ms" {@attach reveal}>
		<item.Visual />
		<div class="caption">
			<h3>{item.title}</h3>
			<p>{item.body}</p>
			{#if limit}<p class="limit">{limit}</p>{/if}
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
			<a class="link" href="#trim">Quick trim</a>
			<a class="link" href="#install">Install</a>
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
			{#if primary}
				<a class="pill" href={primary.url}>Download</a>
			{:else}
				<a class="pill" href="#install">Get {NAME}</a>
			{/if}
		</div>
	</div>
</nav>

<main id="top">
	<header class="hero">
		<img class="logo" src={icon} width="112" height="112" alt="{NAME} app icon" />
		<h1>One key. One file.</h1>
		<p class="lede">
			{NAME} records a display, a window or just an area, with your camera, mic and system audio, into one file for
			your editor. On Mac, Windows and Linux. Press {recordKeys} from any app to start.
		</p>
		<div class="cta">
			{@render heroDownload()}
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
					{@render card(item, i % 2)}
				{/each}
			</div>
		</section>

		<section class="section" aria-labelledby="quiet-title">
			<h2 id="quiet-title" {@attach reveal}>Stays out of the way</h2>
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
			{#if pc}
				<p>
					{recordKeys} works in every app while {NAME} is open{platform === 'linux'
						? ', on desktops that offer global shortcuts'
						: ''}. Esc cancels the countdown.
				</p>
			{:else}
				<p>⇧⌘R works in every app while {NAME} is open, and ⇧⌘S does too while the replay buffer is on.</p>
			{/if}
		</div>
		<ul class="shortcuts" {@attach reveal}>
			{#each shortcuts as shortcut (shortcut.label)}
				<li>
					<span class={{ keys: true, wide: pc }}>
						{#each shortcut.keys as key (key)}
							<kbd class={{ hot: key === 'R' }}>{key}</kbd>
						{/each}
					</span>
					<span class="label">{shortcut.label}</span>
				</li>
			{/each}
		</ul>
	</section>

	<section id="trim" class="section split" aria-labelledby="trim-title">
		<div class="intro" {@attach reveal}>
			<h2 id="trim-title">Quick trim</h2>
			<p>
				Cut the ends off a recording without leaving {NAME}. Fast copies the video as it is, with no re-encode;
				Exact re-encodes for a cut on the exact frame. Save a copy, or replace the original and send it to the
				Trash.
			</p>
			{#if pc}<p class="limit">Mac only for now.</p>{/if}
		</div>
		<ul class="commands" {@attach reveal}>
			{#each trimKeys as row (row.label)}
				<li>
					<span class="combo">
						{#each row.keys as key (key)}<kbd>{key}</kbd>{/each}
					</span>
					<span class="label">{row.label}</span>
				</li>
			{/each}
		</ul>
	</section>

	<section id="install" class="section" aria-labelledby="install-title">
		<div class="install-head" {@attach reveal}>
			<h2 id="install-title">Install</h2>
			<p>Version {VERSION}. Free to download; nothing to sign up for.</p>
		</div>

		<div class="tabs" role="tablist" aria-label="Operating system" {@attach reveal}>
			{#each OSES as os (os)}
				<button
					type="button"
					role="tab"
					id="tab-{os}"
					aria-selected={tab === os}
					aria-controls="panel-{os}"
					tabindex={tab === os ? 0 : -1}
					class="tab"
					onclick={() => (tab = os)}
					onkeydown={tabKey}
				>
					{@render osIcon(os)}
					{OS_LABEL[os]}
					{#if isBeta(os)}<span class="beta">Beta</span>{/if}
				</button>
			{/each}
		</div>

		{#each OSES as os (os)}
			<div class="panel" role="tabpanel" id="panel-{os}" aria-labelledby="tab-{os}" hidden={tab !== os}>
				<div class="get">
					{#if os === 'mac'}
						<a class="download" href={mac.dmg.url}>{@render osIcon('mac')} Download {mac.dmg.label}</a>
						<p class="files">{mac.dmg.file}</p>
					{:else if os === 'windows'}
						<a class="download" href={win.exe.url}>{@render osIcon('windows')} Download installer</a>
						<p class="files">
							{win.exe.file} · or the <a href={win.msi.url}>{win.msi.label}</a>, which installs for every user and
							needs admin
						</p>
					{:else}
						<div class="arch" role="radiogroup" aria-label="Processor">
							{#each LINUX_ARCHES as arch (arch)}
								<button type="button" role="radio" aria-checked={linuxArch === arch} onclick={() => (linuxArch = arch)}>
									{LINUX_ARCH_LABEL[arch]}
								</button>
							{/each}
						</div>
						<a class="download" href={linux.appimage.url}>{@render osIcon('linux')} Download AppImage</a>
						<p class="files">
							or the <a href={linux.deb.url}>.deb</a> (Ubuntu, Debian) ·
							<a href={linux.rpm.url}>.rpm</a> (Fedora, openSUSE)
						</p>
					{/if}
					<h3 class="req-title">Requires</h3>
					<ul class="req">
						{#each REQUIREMENTS[os] as line (line)}<li>{line}</li>{/each}
						{#if os === 'linux'}<li>{LINUX_DISTROS}</li>{/if}
					</ul>
				</div>
				<ol class="steps">
					{#each steps[os] as step, i (step.text)}
						<li>
							<span class="num">{i + 1}</span>
							<div class="step">
								<span>{step.text}</span>
								{#each step.code ?? [] as line (line)}
									<div class="code">
										<code>{line}</code>
										<button type="button" class="copy" onclick={() => copy(line)} aria-label="Copy command">
											{copied === line ? 'Copied' : 'Copy'}
										</button>
									</div>
								{/each}
							</div>
						</li>
					{/each}
				</ol>
			</div>
		{/each}

		<div class="availability" {@attach reveal}>
			<h3>Feature availability</h3>
			<p>Windows and Linux are new and in beta. Some Mac features are still on the way.</p>
			<div class="table">
				<table>
					<thead>
						<tr>
							<th scope="col">Feature</th>
							{#each OSES as os (os)}<th scope="col">{OS_LABEL[os]}</th>{/each}
						</tr>
					</thead>
					<tbody>
						{#each FEATURES as row (row.name)}
							<tr>
								<th scope="row">{row.name}</th>
								{#each OSES as os (os)}
									<td class={{ no: row[os] === false }}>{mark(row[os])}</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
	</section>
</main>

<footer class="footer">
	<div class="brand">
		<img src={icon} width="20" height="20" alt="" />
		<span>{NAME}</span>
	</div>
	<p>Built by <a href={AUTHOR_URL}>{AUTHOR}</a>.</p>
	<a class="link" href={GITHUB_URL}>
		{@render github()}
		{NAME} on GitHub
	</a>
	<a class="link licenses" href={LICENSES_PATH}>Licenses</a>
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
		filter: drop-shadow(0 18px 40px rgba(0, 0, 0, 0.55)) drop-shadow(0 0 40px var(--brand-glow));
		animation-name: pop;
	}

	:global(:root[data-theme='light']) .logo {
		filter: drop-shadow(0 18px 32px rgba(0, 0, 0, 0.22)) drop-shadow(0 0 40px var(--brand-glow));
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
		background: var(--brand);
		box-shadow: 0 10px 30px var(--brand-glow);
		color: var(--on-brand);
		font: 600 15px/18px var(--sans);
		text-decoration: none;
		transition:
			scale 0.2s,
			background-color 0.2s;
	}

	:global(:root[data-theme='light']) .download {
		box-shadow: 0 10px 30px var(--brand-glow);
	}

	.download:hover {
		background: var(--brand-hover);
		scale: 1.03;
	}

	.meta a,
	.files a {
		color: var(--fg);
		text-underline-offset: 2px;
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

	.combo {
		display: flex;
		gap: 4px;
		width: 150px;
		flex-shrink: 0;
	}

	.steps {
		display: flex;
		flex-direction: column;
		min-width: 0;
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

	.other {
		color: var(--muted);
		font-size: 13px;
		text-underline-offset: 2px;
	}

	.other:hover {
		color: var(--fg);
	}

	.beta {
		display: inline-block;
		padding: 1px 6px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--brand) 16%, transparent);
		color: var(--fg);
		font: 600 10.5px/15px var(--sans);
		letter-spacing: 0.02em;
		text-transform: uppercase;
		vertical-align: 1px;
	}

	p.limit,
	.caption p.limit {
		margin-top: 8px;
		color: var(--fg);
		font-size: 13px;
		line-height: 20px;
		opacity: 0.72;
	}

	.intro p.limit {
		margin-top: 10px;
	}

	.keys.wide {
		width: 128px;
	}

	.install-head p {
		margin: -14px 0 0;
		color: var(--muted);
		font-size: 14px;
	}

	.tabs {
		display: inline-flex;
		gap: 4px;
		margin-top: 22px;
		padding: 4px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--fg) 6%, transparent);
	}

	.tab {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		padding: 7px 14px;
		border: 0;
		border-radius: 999px;
		background: none;
		color: var(--muted);
		font: 500 14px var(--sans);
		cursor: pointer;
		transition:
			color 0.15s,
			background-color 0.15s;
	}

	.tab svg {
		width: 13px;
		height: 14px;
		fill: currentColor;
	}

	.tab:hover {
		color: var(--fg);
	}

	.tab[aria-selected='true'] {
		background: var(--card);
		box-shadow: var(--card-edge);
		color: var(--fg);
	}

	.panel {
		display: grid;
		grid-template-columns: 300px minmax(0, 1fr);
		gap: 40px;
		margin-top: 28px;
	}

	.panel[hidden] {
		display: none;
	}

	.get {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
	}

	.files {
		margin: 12px 0 0;
		color: var(--muted);
		font-size: 13px;
		line-height: 20px;
	}

	.arch {
		display: inline-flex;
		gap: 2px;
		margin-bottom: 14px;
		padding: 3px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--fg) 6%, transparent);
	}

	.arch button {
		padding: 4px 11px;
		border: 0;
		border-radius: 999px;
		background: none;
		color: var(--muted);
		font: 500 12.5px var(--mono);
		cursor: pointer;
	}

	.arch button[aria-checked='true'] {
		background: var(--card);
		box-shadow: var(--card-edge);
		color: var(--fg);
	}

	.req-title {
		margin-top: 26px;
		color: var(--fg);
		font-size: 13px;
	}

	.req {
		margin-top: 6px;
	}

	.req li {
		padding: 2px 0;
		color: var(--muted);
		font-size: 13px;
		line-height: 20px;
	}

	.step {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}

	.code {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 6px 6px 12px;
		border-radius: 10px;
		background: var(--card);
		box-shadow: var(--card-edge);
	}

	.code code {
		flex: 1;
		min-width: 0;
		overflow-x: auto;
		color: var(--fg);
		font: 12.5px/20px var(--mono);
		white-space: nowrap;
	}

	.copy {
		flex-shrink: 0;
		min-width: 64px;
		padding: 4px 10px;
		border: 0;
		border-radius: 7px;
		background: color-mix(in srgb, var(--fg) 8%, transparent);
		color: var(--fg);
		font: 500 12px var(--sans);
		cursor: pointer;
	}

	.copy:hover {
		background: color-mix(in srgb, var(--fg) 14%, transparent);
	}

	.availability {
		margin-top: 48px;
	}

	.availability h3 {
		font-size: 14px;
	}

	.availability > p {
		margin: 4px 0 0;
		color: var(--muted);
		font-size: 13px;
	}

	.table {
		margin-top: 14px;
		overflow-x: auto;
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 13px;
		line-height: 18px;
	}

	th,
	td {
		padding: 8px 12px 8px 0;
		border-bottom: 1px solid var(--line);
		text-align: left;
		vertical-align: top;
	}

	thead th {
		color: var(--fg);
		font-weight: 600;
	}

	tbody th {
		color: var(--fg);
		font-weight: 400;
	}

	td {
		color: var(--muted);
	}

	td.no {
		opacity: 0.6;
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
		color: inherit;
		text-underline-offset: 2px;
		transition: color 0.15s;
	}

	.footer p a:hover {
		color: var(--fg);
	}

	.footer .licenses {
		font-size: 13px;
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
		.split,
		.panel {
			grid-template-columns: minmax(0, 1fr);
		}

		.split,
		.panel {
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

		.combo {
			width: auto;
		}

		.tabs {
			display: flex;
		}

		.tab {
			flex: 1;
			justify-content: center;
			padding-inline: 8px;
		}

		.availability td,
		.availability th {
			padding-right: 8px;
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

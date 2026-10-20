<!-- The window being recorded in the demo: a talk's slide, 664×415. -->
<div class="slide">
	<div class="text">
		<div class="kicker">EPISODE 15 · RUST ON THE MAC</div>
		<div class="title">Sign and ship a Mac app</div>
		<ul>
			<li>Sign with a Developer ID</li>
			<li>Notarize it in CI</li>
			<li>One DMG, ready to download</li>
		</ul>
	</div>
	<div class="code">
		<div class="dots"><i></i><i></i><i></i></div>
		<pre>$ codesign --sign "Developer ID" \
    --options runtime Clips.app
$ xcrun notarytool submit Clips.zip \
    --keychain-profile ci --wait
$ xcrun stapler staple Clips.app
<b>✓ Accepted. Ready to ship.</b></pre>
	</div>
</div>

<style>
	.slide {
		display: flex;
		align-items: flex-start;
		gap: 28px;
		width: 664px;
		height: 415px;
		padding: 64px 40px 0;
		background: #fbfbfa;
		color: #141416;
		font-family: var(--sans);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: 14px;
		width: 280px;
		flex-shrink: 0;
	}

	.kicker {
		color: #8a8a90;
		font-size: 9px;
		font-weight: 600;
		letter-spacing: 0.14em;
		line-height: 12px;
	}

	.title {
		width: 230px;
		font: 700 32px/34px var(--display);
		letter-spacing: -0.03em;
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin: 0;
		padding: 4px 0 0;
		color: #3a3a3e;
		font-size: 12px;
		line-height: 16px;
		list-style: none;
	}

	li::before {
		content: '·';
		margin-right: 8px;
	}

	.code {
		display: flex;
		flex-direction: column;
		gap: 10px;
		width: 276px;
		padding: 12px 14px 16px;
		border-radius: 10px;
		background: #17171b;
		box-shadow: 0 12px 30px rgba(0, 0, 0, 0.18);
	}

	.dots {
		display: flex;
		gap: 5px;
	}

	.dots i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: #3a3a40;
	}

	pre {
		margin: 0;
		color: #d6d6dc;
		font: 9.5px/15px var(--mono);
	}

	pre b {
		color: #30d158;
		font-weight: 400;
	}
</style>

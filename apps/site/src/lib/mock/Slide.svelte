<!-- The window being recorded in the demo: a review deck's slide, 664×415. -->
<div class="slide">
	<div class="text">
		<div class="kicker">DESIGN REVIEW · WEEK 45</div>
		<div class="title">Onboarding, round two</div>
		<ul>
			<li>A shorter welcome screen</li>
			<li>Ask for each permission in context</li>
			<li>Ship it to the beta on Friday</li>
		</ul>
	</div>
	<div class="code">
		<div class="dots"><i></i><i></i><i></i></div>
		<pre>$ pnpm test onboarding
  welcome screen     4 passed
  permission steps   9 passed
$ pnpm build
  built in 2.4 s
<b>✓ Ready for the beta.</b></pre>
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

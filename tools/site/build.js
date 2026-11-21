// Scratch generator for docs/site/index.html. The FAQ is written once here and
// printed twice: as visible HTML and as FAQPage JSON-LD, so the two always match.
const fs = require( 'fs' )
const OUT = require( 'path' ).join( __dirname, '../../docs/site/index.html' )
const SITE = 'https://deikus-lxxvii.github.io/Scorpio-Crosshair/'
const REPO = 'https://github.com/Deikus-LXXVII/Scorpio-Crosshair'
const DOWNLOAD = REPO + '/releases/latest'
const VERSION = '0.1.0'
const DATE = '2026-11-21'

const TITLE = 'Scorpio: crosshair overlay for Windows and Linux'
const DESC = 'Free crosshair overlay for Windows and Linux. Scorpio draws a CS2-based crosshair over your game only while you play, and never touches the game or its anti-cheat.'

const BANDS = 'M647.8 403.4L896 512L512 680L484.2 667.8L453.5 597.7L512 623.2L766.3 512L678.5 473.6ZM403.4 376.2L512 128L680 512L667.8 539.8L597.7 570.5L623.2 512L512 257.7L473.6 345.5ZM376.2 620.6L128 512L512 344L539.8 356.2L570.5 426.3L512 400.8L257.7 512L345.5 550.4ZM620.6 647.8L512 896L344 512L356.2 484.2L426.3 453.5L400.8 512L512 766.3L550.4 678.5Z'
const mark = ( cls, label ) => `<svg class="${cls}" viewBox="128 128 768 768" ${label ? `role="img" aria-label="${label}"` : 'aria-hidden="true"'}><path fill="#ebe5da" d="${BANDS}"/><path fill="#c8435a" d="M468 512L512 468L556 512L512 556Z"/></svg>`

// [question, answer as HTML]. Keep the answers plain: links and <b> only.
const FAQ = [
	[ 'Is a crosshair overlay bannable?',
		`No overlay can promise that you will never be banned. Scorpio never reads or changes the game, but no anti-cheat vendor has approved it, and a game's rules can forbid external crosshairs.` ],
	[ 'Does Scorpio work with Vanguard, Easy Anti-Cheat, BattlEye or VAC?',
		`Nobody has tested it against them yet, and none of these vendors has approved it.` ],
	[ 'Do CS2 crosshair codes work in Scorpio?',
		`Yes. Paste the code into <b>Share code</b> on the <b>Crosshair</b> tab, and <b>Copy</b> gives you a code back. A dynamic crosshair shows in its static resting style.` ],
	[ 'Does Scorpio work on Linux?',
		`Yes, with X11 or XWayland, but it is less tested there than on Windows. Native Wayland windows are not supported.` ],
	[ 'Does the crosshair show on my stream?',
		`With OBS Display Capture, yes. With Game Capture, follow <a class="link" href="#obs">the OBS steps</a> on this page.` ],
	[ 'Why does the crosshair not show over my game?',
		`If the title bar says "Hidden by shortcut", press the hide shortcut again. Otherwise add the game on the <b>Games</b> tab, or switch the game from exclusive fullscreen to borderless.` ],
	[ 'Does Scorpio lower my frame rate or add input lag?',
		`Scorpio does not touch the game or its input. Its effect on the frame rate has not been measured yet.` ],
	[ 'Does Scorpio collect data?',
		`No. It has no telemetry and no account, and it goes online only to check for and download updates from GitHub.` ],
]
const plain = html => html.replace( /<[^>]+>/g, '' )

const SHOTS = {
	games: [ 'scorpio-game-detection.webp', 'Scorpio Games tab: two added games, one with its own crosshair, and switches for fullscreen windows and games from launchers' ],
	editor: [ 'scorpio-saved-and-pro-crosshairs.webp', 'Scorpio Crosshair tab: a green crosshair in the preview at 4x, its share code, four saved crosshairs and pro player crosshairs' ],
	controls: [ 'scorpio-hide-while-aiming-and-shortcuts.webp', 'Scorpio Controls tab: the hide while aiming switch, the aim button, hold or toggle mode, and the six global shortcuts' ],
	obs: [ 'scorpio-obs-browser-source.webp', 'Scorpio System tab: Show the crosshair in OBS is on, with three setup steps and the Browser source address' ],
}
const shot = key => `<figure class="shot"><img src="images/${SHOTS[key][0]}" width="1960" height="1400" loading="lazy" decoding="async" alt="${SHOTS[key][1]}"></figure>`

const ld = {
	'@context': 'https://schema.org',
	'@graph': [
		{
			'@type': 'SoftwareApplication',
			'@id': SITE + '#software',
			name: 'Scorpio',
			alternateName: [ 'Scorpio Crosshair', 'Scorpio crosshair overlay' ],
			description: 'A crosshair overlay for PC games on Windows and Linux. Scorpio draws a CS2-based crosshair in its own window over the game, shows it only while a game is in front, and never opens, reads, hooks or injects into the game or its anti-cheat.',
			url: SITE,
			image: SITE + 'og.png',
			screenshot: Object.values( SHOTS ).map( ( [ file ] ) => SITE + 'images/' + file ),
			applicationCategory: 'UtilitiesApplication',
			applicationSubCategory: 'Crosshair overlay for games',
			operatingSystem: 'Windows 10, Windows 11, Linux (X11 or XWayland)',
			softwareVersion: VERSION,
			softwareRequirements: 'Microsoft Edge WebView2 on Windows. X11 or XWayland on Linux.',
			downloadUrl: DOWNLOAD,
			releaseNotes: REPO + '/releases',
			license: 'https://polyformproject.org/licenses/noncommercial/1.0.0',
			isAccessibleForFree: true,
			offers: { '@type': 'Offer', price: 0, priceCurrency: 'USD' },
			author: { '@type': 'Organization', name: 'LXXVII', url: 'https://lxxvll.com' },
			sameAs: [ REPO ],
			inLanguage: 'en',
			dateModified: DATE,
			featureList: [
				'Shows the crosshair only while a game is the window in front',
				'Detects games you added, fullscreen windows, and games installed by Steam, Epic Games, GOG, Ubisoft Connect, EA, Battle.net, Riot, Xbox, Heroic and Lutris',
				'CS2-based crosshairs with five static styles',
				'CS2 crosshair codes work in Scorpio, in and out',
				'Thickness in half steps and an outline of 1 to 6 pixels',
				'13 pro player crosshairs',
				'A crosshair per game and saved crosshairs',
				'Exact centring on the monitor of the game',
				'Hide while aiming, read through raw input',
				'Global shortcuts',
				'Opt-in OBS Browser source for streams that use Game Capture',
				'Signed automatic updates, installed only between games',
				'Never opens, reads, hooks or injects into the game or its anti-cheat',
			],
		},
		{
			'@type': 'FAQPage',
			'@id': SITE + '#faq',
			mainEntity: FAQ.map( ( [ q, a ] ) => ( { '@type': 'Question', name: q, acceptedAnswer: { '@type': 'Answer', text: plain( a ) } } ) ),
		},
	],
}

const css = `
@font-face{font-family:"Forum";font-style:normal;font-weight:400;font-display:swap;src:url("fonts/forum-latin.woff2") format("woff2")}
@font-face{font-family:"Inter";font-style:normal;font-weight:300 500;font-display:swap;src:url("fonts/inter-latin.woff2") format("woff2")}
:root{color-scheme:dark;--cut:#060605;--basalt:#0b0a09;--stone:#141311;--raised:#1c1a17;--lit:#fff8ec;--marble:#ebe5da;--mute:#8a837a;--faint:#4a4640;--line:rgba(235,229,218,.10);--line-hi:rgba(235,229,218,.22);--line-ctl:rgba(235,229,218,.40);--porphyry-hi:#c8435a;--porphyry-ink:#d16174;--cramp-1:#6b5a40;--cramp-2:#3f3426;--cramp-3:#2c241a;--rivet:#a08a63;--rivet-lo:#3a2f22;--display:"Forum","Times New Roman",serif;--text:"Inter",system-ui,sans-serif;--mono:ui-monospace,"SF Mono",Menlo,Consolas,monospace;--ease:cubic-bezier(.2,.7,.15,1);--gutter:40px}
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%;scroll-behavior:smooth}
html,body{margin:0;background:var(--basalt)}
body{color:var(--marble);font:300 16px/1.6 var(--text);-webkit-font-smoothing:antialiased}
h1,h2,h3,p,ul,ol,dl,dd,figure{margin:0}
ul,ol{padding:0;list-style:none}
a{color:inherit;text-decoration:none}
img{display:block;max-width:100%;height:auto}
::selection{background:rgba(235,229,218,.22)}
:focus{outline:none}
:focus-visible{outline:1px solid var(--marble);outline-offset:3px}
.sr{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}
.skip{position:absolute;left:var(--gutter);top:-60px;z-index:10;padding:8px 12px;background:var(--raised);font-size:13px}
.skip:focus{top:8px}
.wrap{max-width:1280px;margin:0 auto;padding:0 var(--gutter)}
.link{color:var(--marble);text-decoration:underline;text-decoration-thickness:1px;text-underline-offset:4px;text-decoration-color:var(--line-ctl);transition:text-decoration-color .3s var(--ease)}
.link:hover{text-decoration-color:var(--marble)}
b{font-weight:400;color:var(--marble)}
code{font:400 .86em/1 var(--mono);color:#cfc7ba}
kbd{display:inline-block;padding:1px 6px;font:400 12px/1.5 var(--text);letter-spacing:.04em;color:var(--marble);background:var(--cut);border-radius:1px;box-shadow:inset 0 0 0 1px var(--line-hi)}

/* hero: one tablet on the wall */
.wall{padding:28px 28px 0}
.tablet{position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center;min-height:max(620px,calc(100vh - 100px));padding:72px 24px 64px;text-align:center;background:var(--stone);border-radius:2px 2px 0 0;box-shadow:inset 0 0 0 1px rgba(235,229,218,.06),inset 0 1px 0 rgba(235,229,218,.10)}
.tablet-edge{height:16px;background:linear-gradient(#100f0d,#080706);border-top:1px solid rgba(235,229,218,.14);box-shadow:0 1px 0 rgba(0,0,0,.8),0 30px 60px rgba(0,0,0,.75),0 10px 22px rgba(0,0,0,.6)}
.cramps::before,.cramps::after{content:"";position:absolute;left:7%;right:7%;height:14px;pointer-events:none;--r:radial-gradient(circle at 35% 35%,var(--rivet),var(--rivet-lo) 2.5px,transparent 3px);--i:linear-gradient(var(--cramp-1),var(--cramp-2) 60%,var(--cramp-3));background:var(--r) left 1.5px top 0/14px 14px no-repeat,var(--r) left 26.5px top 0/14px 14px no-repeat,var(--r) right 1.5px top 0/14px 14px no-repeat,var(--r) right 26.5px top 0/14px 14px no-repeat,var(--i) left top/42px 14px no-repeat,var(--i) right top/42px 14px no-repeat;filter:drop-shadow(0 2px 2px rgba(0,0,0,.6))}
.cramps::before{top:-7px}
.cramps::after{display:none}
.hero-mark{width:clamp(84px,9vw,128px);height:auto;margin-bottom:36px}
.hero-h{display:flex;flex-direction:column;align-items:center;font-weight:400}
/* the carved name, static form: the cut, its lit far wall, and light pooled in the cut */
.inscr{position:relative;display:inline-block;font:400 clamp(58px,15.5vw,236px)/.9 var(--display);letter-spacing:.06em;margin-right:-.06em;text-transform:uppercase;white-space:nowrap;user-select:none;color:var(--cut);text-shadow:0 2px 0 rgba(235,225,205,.03),0 -2.4px 1px rgba(0,0,0,.7),0 0 1px rgba(0,0,0,.6)}
.inscr::before,.inscr::after{content:attr(data-t);position:absolute;inset:0;color:transparent;pointer-events:none}
.inscr::before{text-shadow:0 2px 0 rgba(240,228,206,.28);-webkit-mask-image:radial-gradient(110% 260% at 42% 30%,#000,rgba(0,0,0,.17) 45%,transparent 72%);mask-image:radial-gradient(110% 260% at 42% 30%,#000,rgba(0,0,0,.17) 45%,transparent 72%)}
.inscr::after{text-shadow:none;background:radial-gradient(110% 260% at 42% 30%,rgba(240,228,206,.72),rgba(240,228,206,.12) 45%,transparent 72%);-webkit-background-clip:text;background-clip:text}
.lede{max-width:56ch;margin:28px auto 0;font-size:17px;color:var(--mute)}
.cta{display:flex;flex-wrap:wrap;align-items:center;justify-content:center;gap:12px 32px;margin-top:40px}
.btn{display:inline-flex;align-items:center;justify-content:center;height:52px;padding:0 28px;font:500 14px/1 var(--text);white-space:nowrap;color:var(--basalt);background:var(--marble);border:1px solid var(--marble);border-radius:2px;transition:transform .4s var(--ease),background-color .3s var(--ease),border-color .3s var(--ease)}
.btn:hover{background:var(--lit);border-color:var(--lit);transform:scale(1.03)}
.btn:active{transform:scale(.98)}
.go{display:inline-block;padding:10px 0;font:500 11px/1 var(--text);letter-spacing:.28em;text-transform:uppercase;color:var(--mute);transition:color .5s var(--ease)}
.go:hover,.go:focus-visible{color:var(--lit)}
.fine{margin-top:18px;font-size:12px;color:var(--mute)}

/* the row of facts under the hero */
.meta{display:grid;grid-template-columns:repeat(4,1fr);margin-top:72px;border-top:1px solid var(--line);border-bottom:1px solid var(--line)}
.meta>div{padding:18px 20px 18px 0}
.meta>div+div{padding-left:20px;border-left:1px solid var(--line)}
.meta dt{font:500 10px/1 var(--text);letter-spacing:.26em;text-transform:uppercase;color:var(--mute)}
.meta dd{margin-top:10px;font-size:14px;color:var(--marble)}

/* contents */
.toc{display:flex;flex-wrap:wrap;gap:6px 34px;margin-top:40px;padding-bottom:22px;border-bottom:1px solid var(--line)}
.toc a{display:inline-flex;gap:10px;align-items:baseline;padding:6px 0;font-size:14px;color:var(--marble);transition:color .3s var(--ease)}
.toc a:hover,.toc a:focus-visible{color:var(--lit)}

/* two-column blocks: copy 5, figure 7, alternating */
.block{display:grid;grid-template-columns:5fr 7fr;gap:72px;align-items:center;padding-top:130px}
.block.flip{grid-template-columns:7fr 5fr}
.block.flip .copy{order:2}
.copy h2{margin-top:22px;font:400 clamp(36px,3.4vw,50px)/1.05 var(--display)}
.copy p{max-width:46ch;margin-top:24px;color:var(--mute)}
.copy p+p{margin-top:14px}
.copy .facts{margin-top:30px}
.shot{border-radius:2px;box-shadow:0 0 0 1px var(--line)}
.shot img{width:100%;border-radius:2px}

/* document sections */
.sec{padding-top:150px}
.sec h2{margin-top:14px;font:400 clamp(36px,3.4vw,50px)/1.05 var(--display)}
.sec .lead{max-width:62ch;margin-top:22px;font-size:17px;color:var(--mute)}
.sec .lead+.lead{margin-top:14px}
.obj{margin-top:42px}
.scroll{overflow-x:auto;scrollbar-width:thin;scrollbar-color:var(--raised) transparent}
.scroll::-webkit-scrollbar{height:10px;background:transparent}
.scroll::-webkit-scrollbar-thumb{background:var(--raised);border:3px solid var(--basalt);border-radius:0}
.scroll .tbl{min-width:var(--tbl-min,0)}
.tbl{width:100%;border-collapse:collapse;font-size:15.5px}
.tbl th{padding:0 16px 14px 0;text-align:left;font:500 10px/1 var(--text);letter-spacing:.24em;text-transform:uppercase;color:var(--mute);border-bottom:1px solid var(--line-hi)}
.tbl td{padding:15px 16px 15px 0;border-bottom:1px solid var(--line);color:var(--mute);vertical-align:top}
.tbl td:first-child{color:var(--marble)}
.rows>li{position:relative;padding:13px 0 13px 19px;border-bottom:1px solid var(--line);color:var(--mute)}
.rows>li::before{content:"";position:absolute;left:0;top:25px;width:5px;height:5px;border-radius:50%;background:var(--faint)}
.notice{max-width:78ch;padding:16px 20px;font-size:15px;line-height:1.6;color:var(--mute);background:var(--raised);box-shadow:inset 2px 0 0 var(--porphyry-hi);border-radius:2px}
.stats{display:grid;grid-template-columns:repeat(4,1fr);border-top:1px solid var(--line);border-bottom:1px solid var(--line)}
.stats>div{padding:24px 20px 24px 0}
.stats>div+div{padding-left:20px;border-left:1px solid var(--line)}
.stats b{display:block;font:400 40px/1 var(--display);font-variant-numeric:tabular-nums;color:var(--marble)}
.stats span{display:block;margin-top:10px;font-size:13px;color:var(--mute)}
.cells{display:grid;grid-template-columns:repeat(2,1fr);gap:1px;background:var(--line);border:1px solid var(--line)}
.cell{padding:26px 28px;background:var(--basalt)}
.cell h3{font:400 21px/1.2 var(--display)}
.cell p,.cell li{margin-top:10px;font-size:14.5px;color:var(--mute)}
.cell ol{counter-reset:step}
.cell ol li{position:relative;padding-left:26px;counter-increment:step}
.cell ol li::before{content:counter(step);position:absolute;left:0;font:400 15px/1.6 var(--display);color:var(--marble)}
.steps{max-width:78ch;counter-reset:step}
.steps li{position:relative;padding:13px 0 13px 40px;border-bottom:1px solid var(--line);color:var(--mute);counter-increment:step}
.steps li::before{content:counter(step);position:absolute;left:0;top:10px;font:400 21px/1.4 var(--display);color:var(--marble)}
.faq{max-width:860px}
.faq>div{padding:26px 0;border-bottom:1px solid var(--line)}
.faq>div:first-child{border-top:1px solid var(--line)}
.faq h3{font:400 21px/1.25 var(--display)}
.faq p{max-width:70ch;margin-top:10px;color:var(--mute)}

footer{margin-top:160px;border-top:1px solid var(--line)}
.foot{display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:16px 40px;padding-top:40px;padding-bottom:56px}
.sig{display:inline-flex;align-items:center;gap:12px;font:400 17px/1 var(--display);letter-spacing:.18em;text-transform:uppercase}
.sig svg{width:22px;height:22px}
.foot nav{display:flex;flex-wrap:wrap;gap:6px 28px}
.year{font:400 15px/1 var(--display);letter-spacing:.2em;color:var(--mute)}

@media (max-width:900px){
	.block,.block.flip{grid-template-columns:1fr;gap:36px;padding-top:96px}
	.block .copy,.block.flip .copy{order:2}
	.meta,.stats{grid-template-columns:repeat(2,1fr)}
	.meta>div:nth-child(3),.stats>div:nth-child(3){padding-left:0;border-left:0}
	.meta>div:nth-child(n+3),.stats>div:nth-child(n+3){border-top:1px solid var(--line)}
	.sec{padding-top:110px}
}
@media (max-width:640px){
	:root{--gutter:20px}
	.wall{padding:14px 14px 0}
	.tablet{min-height:560px;padding:56px 16px 48px}
	.cramps::before{left:10%;right:10%}
	.cells{grid-template-columns:1fr}
	.meta{margin-top:56px}
	.sec{padding-top:88px}
	footer{margin-top:110px}
}
@media (prefers-reduced-motion:reduce){
	html{scroll-behavior:auto}
	*,*::before,*::after{transition:none!important}
}
`

const sec = ( n, id, title, body ) => `
<section class="sec wrap" id="${id}" aria-labelledby="${id}-h">
	<h2 id="${id}-h">${title}</h2>
	${body}
</section>`

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${TITLE}</title>
<meta name="description" content="${DESC}">
<meta name="robots" content="index, follow, max-image-preview:large, max-snippet:-1">
<link rel="canonical" href="${SITE}">
<meta name="theme-color" content="#0b0a09">
<meta name="color-scheme" content="dark">
<link rel="icon" href="favicon.svg" type="image/svg+xml">
<link rel="icon" href="favicon.png" type="image/png" sizes="32x32">
<link rel="preload" href="fonts/forum-latin.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="fonts/inter-latin.woff2" as="font" type="font/woff2" crossorigin>
<link rel="sitemap" href="sitemap.xml" type="application/xml">
<link rel="describedby" href="llms.txt" type="text/plain">
<link rel="alternate" href="https://raw.githubusercontent.com/Deikus-LXXVII/Scorpio-Crosshair/main/README.md" type="text/markdown" title="Scorpio README in Markdown">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Scorpio">
<meta property="og:locale" content="en_GB">
<meta property="og:title" content="${TITLE}">
<meta property="og:description" content="${DESC}">
<meta property="og:url" content="${SITE}">
<meta property="og:image" content="${SITE}og.png">
<meta property="og:image:type" content="image/png">
<meta property="og:image:width" content="1280">
<meta property="og:image:height" content="640">
<meta property="og:image:alt" content="The Scorpio logo on a stone tablet, with the line: a crosshair over your game, only while you play">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="${TITLE}">
<meta name="twitter:description" content="${DESC}">
<meta name="twitter:image" content="${SITE}og.png">
<meta name="twitter:image:alt" content="The Scorpio logo on a stone tablet, with the line: a crosshair over your game, only while you play">
<script type="application/ld+json">
${JSON.stringify( ld, null, '\t' ).replace( /</g, '\\u003c' )}
</script>
<style>${css}</style>
</head>
<body>
<a class="skip" href="#main">Skip to the content</a>

<header class="wall">
	<div class="tablet cramps">
		${mark( 'hero-mark' )}
		<h1 class="hero-h">
			<span class="inscr" data-t="Scorpio">Scorpio</span>
		</h1>
		<p class="lede">Scorpio draws a <b>CS2-based crosshair</b> in its own window over your game. It shows the crosshair only while a game is in front, and it never opens, reads, hooks or injects into the game or its anti-cheat.</p>
		<div class="cta">
			<a class="btn" href="${DOWNLOAD}">Download Scorpio</a>
			<a class="go" href="${REPO}">Read the source on GitHub</a>
		</div>
		<p class="fine">Version ${VERSION} · Windows installer, Linux AppImage and .deb · Free for non-commercial use</p>
	</div>
	<div class="tablet-edge"></div>
</header>

<main id="main">

<div class="wrap">
	<dl class="meta">
		<div><dt>Systems</dt><dd>Windows 10 and 11. Linux with X11 or XWayland</dd></div>
		<div><dt>Crosshair</dt><dd>CS2-based crosshairs in five static styles. CS2 crosshair codes work</dd></div>
		<div><dt>While it waits</dt><dd>6.9 MB of memory, 0 ms of CPU over 10 s</dd></div>
		<div><dt>Licence</dt><dd>PolyForm Noncommercial 1.0.0, source-available</dd></div>
	</dl>
	<nav class="toc" aria-label="Contents">
		<a href="#games">Only over games</a>
		<a href="#crosshair">The crosshair</a>
		<a href="#controls">Controls</a>
		<a href="#obs">OBS</a>
		<a href="#safety">Anti-cheat safety</a>
		<a href="#weight">Weight</a>
		<a href="#install">Install</a>
		<a href="#faq">Questions</a>
		<a href="#status">What is tested</a>
	</nav>
</div>

<section class="block wrap" id="games" aria-labelledby="games-h">
	<div class="copy">
		<h2 id="games-h">Only while a game is in front</h2>
		<p>The crosshair appears when a game is the window in front and disappears when you switch away. A game is a window <b>you added</b>, any <b>fullscreen</b> window, or a game from a launcher such as Steam, Epic Games or GOG.</p>
	</div>
	${shot( 'games' )}
</section>

<section class="block flip wrap" id="crosshair" aria-labelledby="crosshair-h">
	<div class="copy">
		<h2 id="crosshair-h">CS2 crosshair codes work in Scorpio</h2>
		<p>Paste a share code to import a crosshair, and <b>Copy</b> to export yours. You get five static styles, thickness in half steps, an outline of 1 to 6 pixels, 13 pro player crosshairs and a crosshair per game.</p>
	</div>
	${shot( 'editor' )}
</section>

<section class="block wrap" id="controls" aria-labelledby="controls-h">
	<div class="copy">
		<h2 id="controls-h">Hide while aiming, and six shortcuts</h2>
		<p>Scorpio hides the crosshair while you hold the aim button, or on each press. It reads the button through raw input and sends nothing to the game. Global shortcuts lock, centre, hide and switch the crosshair, and add the game in front.</p>
	</div>
	${shot( 'controls' )}
</section>

<section class="block flip wrap" id="obs" aria-labelledby="obs-h">
	<div class="copy">
		<h2 id="obs-h">The crosshair in OBS, also with Game Capture</h2>
		<p>Game Capture records only the game, so add Scorpio to OBS as a <b>Browser source</b>.</p>
		<ol class="steps">
			<li>In Scorpio, open the System tab and switch on <b>Show the crosshair in OBS</b>.</li>
			<li>In OBS, add a Browser source above your game capture.</li>
			<li>Set its URL to <code>http://127.0.0.1:47117/obs</code>.</li>
			<li>Set its width and height to your canvas size.</li>
			<li>Keep it filling the whole canvas, with no transform.</li>
		</ol>
	</div>
	${shot( 'obs' )}
</section>
${sec( '05', 'safety', 'It never touches the game or its anti-cheat', `
	<p class="lead">Scorpio never opens another process, reads or writes its memory, injects code, installs a hook, sends synthetic input, loads a driver, or draws into the game's frames. No anti-cheat vendor has reviewed or approved it. The table lists every system call it makes.</p>
	<div class="obj scroll" style="--tbl-min:820px" tabindex="0" role="region" aria-label="Operating system calls that Scorpio makes">
		<table class="tbl">
			<thead><tr><th scope="col">Need</th><th scope="col">Windows</th><th scope="col">Linux (X11)</th><th scope="col">Why it stays outside the game</th></tr></thead>
			<tbody>
				<tr><td>Which window is in front</td><td><code>SetWinEventHook</code>, out of context</td><td><code>_NET_ACTIVE_WINDOW</code> events</td><td>Nothing is loaded into another process</td></tr>
				<tr><td>Title, class and size of the window</td><td><code>GetWindowTextW</code>, <code>GetClassNameW</code>, <code>GetWindowRect</code></td><td>Window manager properties</td><td>No process is opened</td></tr>
				<tr><td>Path of the program</td><td><code>NtQuerySystemInformation</code></td><td><code>/proc/&lt;pid&gt;/exe</code></td><td>No process handle is opened</td></tr>
				<tr><td>Aim button and hide key</td><td>Raw Input on Scorpio's own window</td><td>XInput2 raw events</td><td>No hooks</td></tr>
				<tr><td>Shortcuts</td><td><code>RegisterHotKey</code></td><td><code>XGrabKey</code></td><td>The standard shortcut API</td></tr>
				<tr><td>The crosshair</td><td>A layered, topmost, click-through window</td><td>An override-redirect ARGB window</td><td>Nothing is drawn into the game</td></tr>
				<tr><td>Crosshair in OBS, opt-in</td><td>HTTP server on <code>127.0.0.1:47117</code></td><td>The same</td><td>Your own computer only</td></tr>
			</tbody>
		</table>
	</div>
	<p class="notice obj"><b>Leave Hide from screen capture off.</b> Cheat overlays use the same Windows call, so an anti-cheat could take a closer look at Scorpio.</p>` )}
${sec( '06', 'weight', 'About 7 MB of memory and no CPU while it waits', `
	<p class="lead">Measured on Windows 11 with a release build. A check fails if a figure grows by more than 25 %.</p>
	<div class="obj stats">
		<div><b>6.9 MB</b><span>Private memory, idle in the tray</span></div>
		<div><b>0 ms</b><span>CPU time over 10 s idle</span></div>
		<div><b>0</b><span>Web view processes with the settings window closed</span></div>
		<div><b>3.7 MB</b><span>Size of scorpio.exe</span></div>
	</div>` )}
${sec( '07', 'install', 'Install Scorpio on Windows or Linux', `
	<p class="lead">Get the files from the <a class="link" href="${DOWNLOAD}">latest release</a>.</p>
	<div class="obj cells">
		<div class="cell">
			<h3>Windows 10 and 11</h3>
			<ol>
				<li>Download the <code>-setup.exe</code> file.</li>
				<li>Run it. It needs no administrator rights.</li>
				<li>Click the tray icon, paste a share code, and switch to your game.</li>
			</ol>
			<p>The installer is unsigned, so SmartScreen may warn you the first time.</p>
		</div>
		<div class="cell">
			<h3>Linux with X11 or XWayland</h3>
			<ol>
				<li>Download the <code>.AppImage</code>, or the <code>.deb</code> for Debian and Ubuntu.</li>
				<li>Make the AppImage executable and run it, or run <code>sudo apt install ./Scorpio_*.deb</code>.</li>
				<li>Click the tray icon, paste a share code, and switch to your game.</li>
			</ol>
			<p>Native Wayland windows are not supported.</p>
		</div>
	</div>` )}
${sec( '08', 'faq', 'Questions', `
	<div class="obj faq">
${FAQ.map( ( [ q, a ] ) => `\t\t<div><h3>${q}</h3><p>${a}</p></div>` ).join( '\n' )}
	</div>` )}
${sec( '09', 'status', 'What is tested and what is not', `
	<p class="lead">Version ${VERSION} on 21 November 2026. The screenshots show the real settings window with sample data.</p>
	<div class="obj scroll" style="--tbl-min:480px" tabindex="0" role="region" aria-label="What is tested">
		<table class="tbl">
			<thead><tr><th scope="col">Area</th><th scope="col">State</th></tr></thead>
			<tbody>
				<tr><td>Crosshair maths, share codes, game matching</td><td>Unit tests</td></tr>
				<tr><td>Windows build</td><td>Run and measured on Windows 11</td></tr>
				<tr><td>Inside a real game</td><td>Not tested yet</td></tr>
				<tr><td>Linux</td><td>Virtual X server only</td></tr>
				<tr><td>OBS</td><td>Local server tested, OBS not yet</td></tr>
				<tr><td>Automatic updates</td><td>Not yet run against a release</td></tr>
				<tr><td>Any anti-cheat</td><td>Not tested</td></tr>
			</tbody>
		</table>
	</div>` )}

</main>

<footer>
	<div class="wrap foot">
		<span class="sig">${mark( '', 'Scorpio' )}Scorpio</span>
		<nav aria-label="Links">
			<a class="go" href="${REPO}">GitHub</a>
			<a class="go" href="${DOWNLOAD}">Releases</a>
			<a class="go" href="${REPO}/blob/main/LICENSE">Licence</a>
			<a class="go" href="${REPO}/blob/main/README.ru.md" hreflang="ru" lang="ru">Русский</a>
			<a class="go" href="https://lxxvll.com">LXXVII</a>
		</nav>
		<span class="year" title="2026">MMXXVI</span>
	</div>
</footer>
</body>
</html>
`
fs.writeFileSync( OUT, html )
console.log( OUT, html.length )

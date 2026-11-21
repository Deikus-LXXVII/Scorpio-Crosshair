// Generator for docs/site/index.html. Run: node tools/site/build.js
// The page follows the lxxvii style (~/.claude/skills/lxxvii-style, rules R1 to R17).
// The FAQ is written once here and printed twice: as visible HTML and as FAQPage JSON-LD, so the two always match.
const fs = require( 'fs' )
const OUT = require( 'path' ).join( __dirname, '../../docs/site/index.html' )
const SITE = 'https://scorpio.lxxvll.com/'
const REPO = 'https://github.com/Deikus-LXXVII/Scorpio-Crosshair'
const DOWNLOAD = REPO + '/releases/latest'
const VERSION = '0.1.1'
const DATE = '2026-11-21'

const TITLE = 'Scorpio: crosshair overlay for Windows and Linux'
// The definition sentence. It lives in the meta description and the JSON-LD; the hero carries only the short line.
const DESC = 'Free crosshair overlay for Windows and Linux. Scorpio draws a CS2-based crosshair over your game only while you play, and never touches the game or its anti-cheat.'
const LINE = 'A crosshair over your game, only while you play'

const BANDS = 'M647.8 403.4L896 512L512 680L484.2 667.8L453.5 597.7L512 623.2L766.3 512L678.5 473.6ZM403.4 376.2L512 128L680 512L667.8 539.8L597.7 570.5L623.2 512L512 257.7L473.6 345.5ZM376.2 620.6L128 512L512 344L539.8 356.2L570.5 426.3L512 400.8L257.7 512L345.5 550.4ZM620.6 647.8L512 896L344 512L356.2 484.2L426.3 453.5L400.8 512L512 766.3L550.4 678.5Z'
const mark = label => `<svg viewBox="128 128 768 768" role="img" aria-label="${label}"><path fill="#ebe5da" d="${BANDS}"/><path fill="#c8435a" d="M468 512L512 468L556 512L512 556Z"/></svg>`

// [question, answer as HTML]. Three sentences at most; links and <b> only.
const FAQ = [
	[ 'Is a crosshair overlay bannable?',
		`No overlay can promise that you will never be banned. Scorpio never reads or changes the game, but a game's own rules can forbid external crosshairs.` ],
	[ 'Does it work with Vanguard, Easy Anti-Cheat, BattlEye or VAC?',
		`None of them has approved it directly. Scorpio crosses none of their lines, since it never touches the game or the anti-cheat. The exception is a game whose own rules forbid crosshair overlays.` ],
	[ 'Does Scorpio lower my frame rate?',
		`Most likely not, though frame rates have not been measured. On Windows it runs as one process with about 10 MB of memory, uses tens of milliseconds of processor time every 30 seconds while idle, and loads nothing into the game.` ],
	[ 'Does Scorpio collect data?',
		`No. It sends no usage data, has no account, and goes online only for updates from GitHub.` ],
]
const plain = html => html.replace( /<[^>]+>/g, '' )

// Every operating system call Scorpio makes, in plain words first: [what it needs, how it stays outside the game,
// the call names on Windows · Linux]. The names stay, in a quiet last column; on a phone the rows stack.
const HEAD = [ 'Scorpio needs to', 'How it stays outside', 'Windows · Linux' ]
const CALLS = [
	[ 'Know which window is in front', 'The system tells it when you switch windows.', '<code>SetWinEventHook</code> · <code>_NET_ACTIVE_WINDOW</code>' ],
	[ 'Recognise a game', 'It reads the window’s title and size and the program’s name, without opening it.', '<code>GetWindowTextW</code>, <code>NtQuerySystemInformation</code> · <code>/proc/&lt;pid&gt;/exe</code>' ],
	[ 'Hide while you aim', 'It hears the aim button through its own window, with no hooks.', 'Raw Input · XInput2' ],
	[ 'Run your shortcuts', 'Ordinary system shortcuts, as any app uses.', '<code>RegisterHotKey</code> · <code>XGrabKey</code>' ],
	[ 'Draw the crosshair', 'Its own see-through window on top; clicks pass through.', 'Layered window · override-redirect' ],
	[ 'Show it in OBS, if asked', 'Only on your own computer.', '<code>127.0.0.1:47117</code>' ],
]

// Efficiency, measured by the owner on a Windows 11 gaming PC, 2026-11-21: each program idle with its crosshair shown,
// no settings window, three runs, 25 s after start; private memory and CPU time over the next 30 s, summed over all
// of a program's processes. The chart shows the middle of the three runs; the runs are kept here as measured.
const EFF = [
	{ name: 'Scorpio', ver: '0.1.1', mem: [ 10.3, 10.4, 10.3 ], cpu: [ 16, 31, 31 ], me: true },
	{ name: 'Simple Crosshair Overlay', ver: '1.2.1', mem: [ 6.4, 6.4, 6.4 ], cpu: [ 625, 438, 516 ] },
	{ name: 'Crosshair Y', ver: '4.2', mem: [ 249.8, 250.5, 251.7 ], cpu: [ 16, 0, 16 ] },
	{ name: 'CrossOver', ver: '3.4.11', mem: [ 358.1, 359.1, 360.0 ], cpu: [ 47, 31, 47 ] },
]
const mid = a => [ ...a ].sort( ( x, y ) => x - y )[ 1 ]
const chart = () => {
	const maxM = Math.max( ...EFF.map( p => mid( p.mem ) ) ), maxC = Math.max( ...EFF.map( p => mid( p.cpu ) ) )
	const cell = ( v, max, unit ) => `<td><span class="v">${v} ${unit}</span><span class="track"><span class="bar" style="width:${( v / max * 100 ).toFixed( 2 )}%"></span></span></td>`
	return `<figure class="chart">
			<table>
				<thead><tr><td></td><th scope="col">Memory</th><th scope="col">Processor time in 30 s</th></tr></thead>
				<tbody>
${EFF.map( p => `\t\t\t\t\t<tr${p.me ? ' class="me"' : ''}><th scope="row">${p.name} <span>${p.ver}</span></th>${cell( mid( p.mem ), maxM, 'MB' )}${cell( mid( p.cpu ), maxC, 'ms' )}</tr>` ).join( '\n' )}
				</tbody>
			</table>
			<figcaption>Windows 11 gaming PC, 21 November 2026: each program idle, crosshair shown, all its processes counted. Middle of three runs.</figcaption>
		</figure>`
}

const SHOTS = {
	games: [ 'scorpio-game-detection.webp', 'Scorpio Games tab: two added games, one with its own crosshair, and switches for fullscreen windows and games from launchers' ],
	editor: [ 'scorpio-saved-and-pro-crosshairs.webp', 'Scorpio Crosshair tab: a green crosshair in the preview at 4x, its share code, four saved crosshairs and pro player crosshairs' ],
	obs: [ 'scorpio-obs-browser-source.webp', 'Scorpio System tab: Show the crosshair in OBS is on, with three setup steps and the Browser source address' ],
	controls: [ 'scorpio-hide-while-aiming-and-shortcuts.webp', 'Scorpio Controls tab: the hide while aiming switch, the aim button, hold or toggle mode, and the six global shortcuts' ],
}
// The hero. Three concepts; switch with this one line.
//   'a' Signum: the Nodus mark cut into the tablet, large, beside the name; its porphyry point is the page's one colour.
//   'b' In front: the tablet is a screen with a drawn hall in it, and the crosshair stands at its vanishing point.
//   'c' Lineae: the crosshair's arms are cut across the tablet; the name and the action sit in its quadrants, on the page's own keylines.
const HERO = 'b'

// Hall for concept b: a vaulted colonnade in one-point perspective, drawn around its vanishing point (0, 0), in px.
const hall = () => {
	const F = 560, W = 1.15, FLOOR = .62, TOP = -.95, NEAR = .55, FAR = 11
	const Z = [ 1, 1.45, 2.1, 3.05, 4.4, 6.4 ]
	const p = ( x, y, z ) => [ +( x * F / z ).toFixed( 1 ), +( y * F / z ).toFixed( 1 ) ]
	const ln = ( a, b, o ) => `<path d="M${a[ 0 ]} ${a[ 1 ]}L${b[ 0 ]} ${b[ 1 ]}" stroke-opacity="${o}"/>`
	const out = []
	const op = z => Math.max( .05, .2 - z * .022 ).toFixed( 3 )
	// the floor's edges, the architrave and the floor's joints run to the vanishing point
	for ( const s of [ -1, 1 ] ) {
		out.push( ln( p( s * W, FLOOR, NEAR ), p( s * W, FLOOR, FAR ), .16 ) )
		out.push( ln( p( s * W, TOP, NEAR ), p( s * W, TOP, FAR ), .16 ) )
	}
	for ( let x = -W + .23; x < W - .1; x += .23 ) if ( Math.abs( x ) > .01 ) out.push( ln( p( x, FLOOR, NEAR ), p( x, FLOOR, FAR ), .07 ) )
	for ( const z of Z ) {
		const o = op( z )
		out.push( ln( p( -W, FLOOR, z ), p( W, FLOOR, z ), o * .6 ) )
		// a column on each side: two edges, a capital and a base
		for ( const s of [ -1, 1 ] ) {
			const xo = s * W, xi = s * ( W - .12 )
			out.push( ln( p( xo, FLOOR, z ), p( xo, TOP, z ), o ), ln( p( xi, FLOOR, z ), p( xi, TOP, z ), o ) )
			out.push( ln( p( s * ( W + .03 ), TOP + .05, z ), p( s * ( W - .15 ), TOP + .05, z ), o ) )
			out.push( ln( p( s * ( W + .03 ), FLOOR - .04, z ), p( s * ( W - .15 ), FLOOR - .04, z ), o ) )
		}
		// the vault: a half circle from column to column
		const [ cx, cy ] = p( 0, TOP, z ), r = +( W * F / z ).toFixed( 1 )
		out.push( `<path d="M${cx - r} ${cy}A${r} ${r} 0 0 1 ${cx + r} ${cy}" stroke-opacity="${o}"/>` )
	}
	return `<svg class="hb-hall" viewBox="-1200 -900 2400 1800" aria-hidden="true"><g fill="none" stroke="#ebe5da" stroke-width="1" vector-effect="non-scaling-stroke">${out.join( '' )}</g></svg>`
}

// The crosshair at the vanishing point: four porphyry arms with a cut outline, CS2's classic shape.
const crosshair = `<svg class="hb-x" viewBox="-16 -16 32 32" aria-hidden="true"><g fill="#c8435a" stroke="#060605" stroke-width="1"><rect x="-1.5" y="-13.5" width="3" height="9"/><rect x="-1.5" y="4.5" width="3" height="9"/><rect x="-13.5" y="-1.5" width="9" height="3"/><rect x="4.5" y="-1.5" width="9" height="3"/></g></svg>`

const cue = cls => `<a class="cue${cls ? ' ' + cls : ''}" href="#crosshair" aria-label="Go to the first section"><i></i></a>`
const download = `<a class="btn" href="${DOWNLOAD}">Download Scorpio</a>`

const HEROES = {
	a: `
	<header class="tablet hero-a">
		<div class="ha-copy">
			<h1 class="ha-name">Scorpio</h1>
			<p class="caps">${LINE}</p>
			${download}
		</div>
		<svg class="ha-mark" viewBox="128 128 768 768" aria-hidden="true">
			<defs>
				<path id="ha-b" d="${BANDS}"/>
				<radialGradient id="ha-l" gradientUnits="userSpaceOnUse" cx="330" cy="300" r="520"><stop offset="0" stop-color="#f0e4ce" stop-opacity=".34"/><stop offset=".45" stop-color="#f0e4ce" stop-opacity=".07"/><stop offset=".72" stop-color="#f0e4ce" stop-opacity="0"/></radialGradient>
			</defs>
			<use href="#ha-b" transform="translate(0 2.5)" fill="#ebe1cd" fill-opacity=".05"/>
			<use href="#ha-b" transform="translate(0 -3)" fill="#000" fill-opacity=".7"/>
			<use href="#ha-b" fill="#060605"/>
			<use href="#ha-b" fill="url(#ha-l)"/>
			<path d="M468 512L512 468L556 512L512 556Z" fill="#c8435a"/>
			<path d="M468 512L512 468L556 512" fill="none" stroke="#e8899a" stroke-opacity=".5" stroke-width="2"/>
		</svg>
		${cue()}
	</header>`,
	b: `
	<header class="tablet hero-b">
		<div class="hb-view" aria-hidden="true">${hall()}${crosshair}</div>
		<div class="hb-copy">
			<h1 class="hb-name">Scorpio</h1>
			<p class="caps">${LINE}</p>
			${download}
		</div>
		${cue()}
	</header>`,
	c: `
	<header class="tablet hero-c">
		<i class="hc-arm h" aria-hidden="true"></i><i class="hc-arm v" aria-hidden="true"></i><i class="hc-pt" aria-hidden="true"></i>
		<h1 class="hc-name" aria-label="Scorpio"><span class="t-hero carved" data-t="Scorpio" aria-hidden="true">Scorpio</span></h1>
		<div class="hc-act">
			<p class="caps">${LINE}</p>
			${download}
		</div>
		${cue( 'hc-cue' )}
	</header>`,
}

// Per-concept styles. Shared parts (wall, tablet, edge, cramps, caps, button, cue) stay in the main sheet.
// --kx is the page's left keyline in the tablet's coordinates: where the copy of every block starts.
// --fx is where the figure column of the first block starts; --ax is the middle of the 72px gap between them.
const HERO_CSS = {
	a: `
.hero-a{display:grid;grid-template-columns:minmax(0,5fr) minmax(0,7fr);gap:72px;align-items:center;justify-items:start;padding:0 var(--kx);text-align:left}
.ha-copy{display:flex;flex-direction:column;align-items:flex-start}
.ha-name{font:400 clamp(72px,8.4vw,128px)/.9 var(--display);letter-spacing:.12em;margin-right:-.12em;text-transform:uppercase;color:var(--lit)}
.ha-copy .caps{margin-top:40px}
.ha-copy .btn{margin-top:40px}
.ha-mark{justify-self:center;width:min(100%,560px,max(440px,calc(100vh - 300px)));height:auto;aspect-ratio:1}
@media (max-width:860px){
	.hero-a{grid-template-columns:minmax(0,1fr);grid-template-rows:auto auto;gap:40px;align-content:center;justify-items:center;padding:0 24px 72px;text-align:center}
	.ha-copy{align-items:center;order:2}
	.ha-name{font-size:clamp(56px,17vw,96px)}
	.ha-copy .caps{margin-top:28px;max-width:300px;margin-right:calc(-1 * var(--track-caps))}
	.ha-copy .btn{margin-top:28px}
	.ha-mark{width:min(100%,max(220px,calc(100vh - 560px)))}
}`,
	b: `
.hero-b{display:block;background:var(--cut);box-shadow:inset 0 0 0 1px var(--line-hi);--vy:42%}
.hb-view{position:absolute;inset:0}
/* the hall fades towards the edges, so the eye goes to the far end and the copy stands on a quiet ground */
.hb-hall{position:absolute;left:calc(50% - 1200px);top:calc(var(--vy) - 900px);width:2400px;height:1800px;-webkit-mask-image:radial-gradient(900px 640px at 50% 50%,#000 25%,rgba(0,0,0,.3) 75%);mask-image:radial-gradient(900px 640px at 50% 50%,#000 25%,rgba(0,0,0,.3) 75%)}
.hb-x{position:absolute;left:calc(50% - 20px);top:calc(var(--vy) - 20px);width:40px;height:40px}
.hb-copy{position:absolute;left:var(--kx);bottom:72px;display:flex;flex-direction:column;align-items:flex-start;text-align:left}
.hb-name{font:400 clamp(64px,8vw,120px)/.9 var(--display);letter-spacing:.12em;margin-right:-.12em;text-transform:uppercase;color:var(--lit);text-shadow:0 0 40px var(--cut),0 0 12px var(--cut)}
.hb-copy .caps{margin-top:28px;text-shadow:0 0 12px var(--cut),0 0 4px var(--cut)}
.hb-copy .btn{margin-top:40px}
@media (max-width:860px){
	.hero-b{--vy:30%}
	.hb-hall{transform:scale(.6)}
	.hb-copy{left:24px;right:24px;bottom:112px}
	.hb-name{font-size:clamp(56px,17vw,96px)}
	.hb-copy .caps{max-width:300px}
	.hb-copy .btn{margin-top:28px}
}`,
	c: `
.hero-c{display:block;--ay:52%;--g:28px;--nf:min(132px,calc((var(--ax) - var(--kx) - 64px) / 4.1))}
.hc-arm{position:absolute;pointer-events:none}
/* the arms are cut into the stone: a dark upper wall and a lit lower one, broken by the crosshair's gap */
.hc-arm.h{left:0;right:0;top:calc(var(--ay) - 1px);height:2px;background:linear-gradient(rgba(0,0,0,.75) 50%,rgba(235,229,218,.09) 50%);-webkit-mask-image:linear-gradient(90deg,#000 calc(var(--ax) - var(--g)),transparent 0 calc(var(--ax) + var(--g)),#000 0);mask-image:linear-gradient(90deg,#000 calc(var(--ax) - var(--g)),transparent 0 calc(var(--ax) + var(--g)),#000 0)}
.hc-arm.v{top:0;bottom:0;left:calc(var(--ax) - 1px);width:2px;background:linear-gradient(90deg,rgba(0,0,0,.75) 50%,rgba(235,229,218,.09) 50%);-webkit-mask-image:linear-gradient(#000 calc(var(--ay) - var(--g)),transparent 0 calc(var(--ay) + var(--g)),#000 0);mask-image:linear-gradient(#000 calc(var(--ay) - var(--g)),transparent 0 calc(var(--ay) + var(--g)),#000 0)}
.hc-pt{position:absolute;left:calc(var(--ax) - 5px);top:calc(var(--ay) - 5px);width:10px;height:10px;transform:rotate(45deg) scale(.72);border-radius:1px;background:var(--porphyry-hi)}
.hc-name{position:absolute;left:var(--kx);bottom:calc(100% - var(--ay) + 40px);font-weight:400}
.hc-name .t-hero{font-size:var(--nf)}
.hc-act{position:absolute;left:calc(var(--ax) + 36px);top:calc(var(--ay) + 40px);display:flex;flex-direction:column;align-items:flex-start;text-align:left}
.hc-act .btn{margin-top:40px}
/* the cue travels down the lower arm itself */
.hc-cue{left:var(--ax);transform:translateX(-50%)}
.hc-cue i{background:none}
/* narrow: the copy stands above the crosshair, whose upper arm stops short of it; the lower arm still carries the cue */
@media (max-width:860px){
	.hero-c{--ax:50%;--ay:calc(100% - 220px);--arm:84px;--nf:min(96px,calc((100vw - 72px) / 4.1));display:flex;flex-direction:column;justify-content:flex-end;align-items:center;padding:0 24px calc(220px + var(--g) + var(--arm) + 28px);text-align:center}
	.hc-arm.v{-webkit-mask-image:linear-gradient(transparent calc(var(--ay) - var(--g) - var(--arm)),#000 0 calc(var(--ay) - var(--g)),transparent 0 calc(var(--ay) + var(--g)),#000 0);mask-image:linear-gradient(transparent calc(var(--ay) - var(--g) - var(--arm)),#000 0 calc(var(--ay) - var(--g)),transparent 0 calc(var(--ay) + var(--g)),#000 0)}
	.hc-name,.hc-act{position:static}
	.hc-act{align-items:center;margin-top:28px;text-align:center}
	.hc-act .caps{max-width:300px;margin-right:calc(-1 * var(--track-caps))}
	.hc-act .btn{margin-top:28px}
}`,
}

const shot = key =>`<figure class="shot"><img src="images/${SHOTS[key][0]}" width="1960" height="1400" loading="lazy" decoding="async" alt="${SHOTS[key][1]}"></figure>`

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
				'CS2 crosshair codes work in Scorpio',
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

// Tokens and components are taken from the lxxvii starter (tokens.css, components.css); only what the page uses.
const css = `
@font-face{font-family:"Forum";font-style:normal;font-weight:400;font-display:swap;src:url("fonts/forum-latin.woff2") format("woff2")}
@font-face{font-family:"Inter";font-style:normal;font-weight:300 500;font-display:swap;src:url("fonts/inter-latin.woff2") format("woff2")}
:root{color-scheme:dark;--cut:#060605;--basalt:#0b0a09;--stone:#141311;--raised:#1c1a17;--lit:#fff8ec;--marble:#ebe5da;--mute:#8a837a;--rest:#7d766c;--faint:#4a4640;--line:rgba(235,229,218,.10);--line-hi:rgba(235,229,218,.22);--line-ctl:rgba(235,229,218,.40);--porphyry-hi:#c8435a;--cramp-1:#6b5a40;--cramp-2:#3f3426;--cramp-3:#2c241a;--rivet:#a08a63;--rivet-lo:#3a2f22;--lit-glow-display:0 0 3px rgba(255,240,215,.75),0 0 18px rgba(250,234,206,.55),0 0 46px rgba(244,226,196,.35),0 0 110px rgba(244,226,196,.18);--shadow-slab:0 30px 60px rgba(0,0,0,.75),0 10px 22px rgba(0,0,0,.6);--display:"Forum","Times New Roman",serif;--text:"Inter",system-ui,sans-serif;--mono:ui-monospace,"SF Mono",Menlo,Consolas,monospace;--fs-hero:clamp(96px,15.5vw,236px);--fs-h1:clamp(48px,5vw,78px);--track-caps:.28em;--ease:cubic-bezier(.2,.7,.15,1);--t-color:.3s;--t-move:.4s;--ease-loop:cubic-bezier(.45,0,.2,1);--gutter:40px}
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
html,body{margin:0;background:var(--basalt)}
body{color:var(--marble);font:300 16px/1.6 var(--text);-webkit-font-smoothing:antialiased}
h1,h2,p,ol,dl,dd,figure{margin:0}
ol{padding:0;list-style:none}
a{color:inherit;text-decoration:none}
img{display:block;max-width:100%;height:auto}
b{font-weight:400;color:var(--marble)}
code{font:400 14px/1 var(--mono);color:#cfc7ba}
::selection{background:rgba(235,229,218,.22)}
:focus{outline:none}
:focus-visible{outline:1px solid var(--marble);outline-offset:3px}

/* the page's own scrollbar is hidden (R8); it still scrolls by wheel, touch and keys */
html.page{scrollbar-width:none;scroll-behavior:smooth}
html.page::-webkit-scrollbar{display:none}
@media (forced-colors:active),(prefers-contrast:more){html.page{scrollbar-width:thin}html.page::-webkit-scrollbar{display:block}}

.skip{position:absolute;left:var(--gutter);top:-60px;z-index:10;padding:8px 16px;font:400 13px/18px var(--text);background:var(--raised);border-radius:2px}
.skip:focus{top:8px}

/* the hero: one tablet on a wall, 56px of the page below show (R3, R7) */
.wall{position:relative;padding:28px 28px 0}
/* the page's keylines in the tablet's coordinates (the tablet starts 28px in): left margin, and the middle of the first block's column gap */
.tablet{--bw:min(1280px,100vw);--kx:calc((100vw - var(--bw)) / 2 + var(--gutter) - 28px);--ax:calc(var(--kx) + (var(--bw) - 2 * var(--gutter) - 72px) * 5 / 12 + 36px)}
.tablet{position:relative;display:grid;place-items:center;height:calc(100vh - 100px);min-height:620px;overflow:hidden;text-align:center;background:var(--stone);border-radius:2px 2px 0 0;box-shadow:inset 0 0 0 1px rgba(235,229,218,.06),inset 0 1px 0 rgba(235,229,218,.10)}
.tablet-edge{position:relative;z-index:1;height:16px;border-radius:0 0 2px 2px;background:linear-gradient(#100f0d,#080706);border-top:1px solid rgba(235,229,218,.14);box-shadow:0 1px 0 rgba(0,0,0,.8),var(--shadow-slab)}
.cramp{position:absolute;z-index:3;width:42px;height:14px;border-radius:2px;background:linear-gradient(180deg,var(--cramp-1),var(--cramp-2) 60%,var(--cramp-3));border:1px solid rgba(0,0,0,.7);box-shadow:inset 0 1px 0 rgba(255,236,200,.18),0 2px 4px rgba(0,0,0,.6);pointer-events:none}
.cramp::before,.cramp::after{content:"";position:absolute;top:4px;width:5px;height:5px;border-radius:50%;background:radial-gradient(circle at 35% 35%,var(--rivet),var(--rivet-lo) 70%)}
.cramp::before{left:6px}
.cramp::after{right:6px}
.cramp.t{top:21px}
.cramp.b{bottom:9px}
.cramp.l{left:calc(28px + 7%)}
.cramp.r{right:calc(28px + 7%)}
.hero-lines{display:flex;flex-direction:column;align-items:center;padding:0 24px}
.hero-lines h1{font-weight:400}
.t-hero{font:400 var(--fs-hero)/.9 var(--display);letter-spacing:.06em;margin-right:-.06em;text-transform:uppercase}
/* the carved name, static form: darker than the stone, its light placed once (motifs.md) */
.carved{color:var(--cut);text-shadow:0 2px 0 rgba(235,225,205,.03),0 -2.4px 1px rgba(0,0,0,.7),0 0 1px rgba(0,0,0,.6);user-select:none;white-space:nowrap}
.carved[data-t]{position:relative;display:inline-block}
.carved[data-t]::before,.carved[data-t]::after{content:attr(data-t);position:absolute;inset:0;color:transparent;pointer-events:none}
.carved[data-t]::before{text-shadow:0 2px 0 rgba(240,228,206,.28);-webkit-mask-image:radial-gradient(60% 200% at 35% 30%,#000,rgba(0,0,0,.17) 45%,transparent 72%);mask-image:radial-gradient(60% 200% at 35% 30%,#000,rgba(0,0,0,.17) 45%,transparent 72%)}
.carved[data-t]::after{text-shadow:none;background:radial-gradient(60% 200% at 35% 30%,rgba(240,228,206,.72),rgba(240,228,206,.12) 45%,transparent 72%);-webkit-background-clip:text;background-clip:text}
.caps{font:500 11px/16px var(--text);letter-spacing:var(--track-caps);text-transform:uppercase;color:var(--mute)}
.hero-lines .caps{margin:56px calc(-1 * var(--track-caps)) 0 0}
.hero-lines .btn{margin-top:40px}
.btn{display:inline-flex;align-items:center;justify-content:center;height:52px;padding:0 28px;font:500 14px/1 var(--text);white-space:nowrap;color:var(--basalt);background:var(--marble);border:1px solid var(--marble);border-radius:2px;transition:transform .4s var(--ease),background-color .3s var(--ease),border-color .3s var(--ease)}
.btn:hover{background:var(--lit);border-color:var(--lit);transform:scale(1.03)}
.btn:active{transform:scale(.98)}
/* the scroll cue: a line and a travelling mark, a link with no words; it scrolls away with the tablet (R4, R7) */
.cue{position:absolute;left:50%;bottom:28px;z-index:2;display:flex;padding:12px 24px;transform:translateX(-50%)}
.cue i{position:relative;width:1px;height:46px;background:linear-gradient(rgba(235,229,218,.32),rgba(235,229,218,0))}
.cue i::after{content:"";position:absolute;left:-1px;top:0;width:3px;height:9px;border-radius:1px;background:rgba(235,229,218,.75);animation:cue 2.8s var(--ease-loop) infinite}
@keyframes cue{0%{transform:translateY(0);opacity:0}20%{opacity:1}80%{opacity:.8}100%{transform:translateY(40px);opacity:0}}

/* a landing block: copy and one figure, changing sides; about one screen (R13) */
.block{position:relative;display:grid;grid-template-columns:minmax(0,5fr) minmax(0,7fr);gap:72px;align-items:center;max-width:1280px;margin:0 auto;padding:160px var(--gutter) 28px}
.block.flip{grid-template-columns:minmax(0,7fr) minmax(0,5fr)}
.block.flip>.copy{order:2}
.block.wide{display:block}
.copy>*+*{margin-top:24px}
.copy p{max-width:46ch;color:var(--mute)}
.name{text-wrap:balance;font:400 var(--fs-h1)/1 var(--display);letter-spacing:.01em;color:var(--rest);transition:color .9s var(--ease),text-shadow .9s var(--ease)}
.block.current .name,.block:has(.go:focus-visible) .name{color:var(--lit);text-shadow:var(--lit-glow-display)}
.go{display:inline-block;padding:12px 0;font:500 11px/1 var(--text);letter-spacing:var(--track-caps);text-transform:uppercase;color:var(--mute);transition:color .5s var(--ease)}
.go:hover,.go:focus-visible{color:var(--lit)}
.go:focus-visible{outline:none;text-decoration:underline;text-underline-offset:8px;text-decoration-thickness:1px}
.copy .go{margin-top:28px}
.shot{border-radius:2px;box-shadow:0 0 0 1px var(--line-hi)}
.shot img{width:100%;border-radius:2px}

/* steps: rows parted by hairlines, a Roman numeral as the ordinal */
.steps{counter-reset:step}
.steps li{position:relative;padding:12px 0 12px 40px;border-bottom:1px solid var(--line);color:var(--mute);counter-increment:step}
.steps li:first-child{border-top:1px solid var(--line)}
.steps li::before{content:counter(step,upper-roman);position:absolute;left:0;top:12px;font:400 15px/1.6 var(--display);letter-spacing:.1em;color:var(--marble)}

/* the wide block: lead, the table of calls, the questions */
.lead{max-width:62ch;margin-top:24px;font-size:17px;color:var(--mute)}
.obj{margin-top:40px}
.tbl{width:100%;border-collapse:collapse}
.tbl thead th{padding:0 24px 12px 0;text-align:left;font:500 11px/16px var(--text);letter-spacing:var(--track-caps);text-transform:uppercase;color:var(--mute);border-bottom:1px solid var(--line-hi)}
.tbl tbody th,.tbl td{padding:16px 24px 16px 0;text-align:left;border-bottom:1px solid var(--line);vertical-align:top}
.tbl tbody th{width:28%;font-weight:400;color:var(--marble)}
.tbl td{color:var(--mute)}
.tbl .tech{width:30%;padding-right:0;font:400 13px/20px var(--mono);color:var(--mute)}
.tbl .tech code{font:inherit;color:inherit}

/* the efficiency chart: a table drawn as bars, one row a program, two measures side by side on their own scales */
.chart table{width:100%;border-collapse:collapse}
.chart tr{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);column-gap:24px}
.chart thead td{display:none}
.chart thead th{padding:0 0 12px;text-align:left;font:400 14px/20px var(--text);color:var(--mute);border-bottom:1px solid var(--line-hi)}
.chart tbody tr{padding:16px 0;border-bottom:1px solid var(--line)}
.chart tbody th{grid-column:1/-1;padding-bottom:8px;text-align:left;font:400 16px/24px var(--text);color:var(--marble)}
.chart tbody th span{color:var(--mute)}
.chart td{padding:0}
.chart .v{display:block;font:400 15px/20px var(--text);font-variant-numeric:tabular-nums;color:var(--mute)}
.chart .track{display:block;margin-top:8px;height:8px;border-bottom:1px solid var(--line)}
.chart .bar{display:block;min-width:2px;height:7px;border-radius:1px 1px 0 0;background:rgba(235,229,218,.68)}
.chart .me th{font-weight:500}
.chart .me .v{color:var(--lit)}
.chart .me .bar{background:var(--porphyry-hi)}
.chart figcaption{margin-top:16px;max-width:62ch;color:var(--mute)}
.faq{max-width:760px}
.qa{border-bottom:1px solid var(--line)}
.qa summary{text-wrap:balance;display:flex;align-items:center;justify-content:space-between;gap:24px;padding:24px 0;font:400 21px/1.2 var(--display);color:var(--marble);list-style:none;cursor:pointer;transition:color var(--t-color) var(--ease)}
.qa summary::-webkit-details-marker{display:none}
.qa summary:hover,.qa[open] summary{color:var(--lit)}
.qa summary i{position:relative;flex:none;width:12px;height:12px}
.qa summary i::before,.qa summary i::after{content:"";position:absolute;left:0;top:50%;width:12px;height:1px;background:currentColor;transition:transform var(--t-move) var(--ease)}
.qa summary i::after{transform:rotate(90deg)}
.qa[open] summary i::after{transform:rotate(0deg)}
.qa .a{overflow:hidden}
.qa .a p{max-width:52ch;padding-bottom:24px;color:var(--mute)}

footer{max-width:1280px;margin:160px auto 0;padding:0 var(--gutter)}
.get{max-width:62ch;padding-bottom:112px}
.get>*+*{margin-top:24px}
.get p{color:var(--mute)}
.get .go{margin-top:28px}
.foot{display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:16px 40px;padding:40px 0 56px;border-top:1px solid var(--line)}
.sig{display:inline-flex;align-items:center;gap:12px;font:400 17px/1 var(--display);letter-spacing:.18em;text-transform:uppercase}
.sig svg{width:24px;height:24px}
.foot nav{display:flex;flex-wrap:wrap;gap:4px 28px}
.year{font:400 15px/1 var(--display);letter-spacing:.2em;color:var(--mute)}

/* reveal, once: the copy column of a block, only when the script is there to show it (R5) */
html.js .rv{opacity:0;transform:translateY(28px);transition:opacity 1.1s var(--ease),transform 1.1s var(--ease)}
html.js .rv.in{opacity:1;transform:none}
@media print{html.js .rv{opacity:1;transform:none}}

/* narrow screens (proposed in the guide: the live site has none) */
@media (max-width:860px){
	:root{--gutter:20px}
	.wall{padding:12px 12px 0}
	.cramp.l{left:calc(12px + 7%)}
	.cramp.r{right:calc(12px + 7%)}
	.block,.block.flip{grid-template-columns:minmax(0,1fr);gap:40px;padding-top:72px}
	.block.flip>.copy{order:0}
	footer{margin-top:72px}
	/* a finger needs 44px: the caps links keep their look and grow their box */
	.go{display:inline-flex;align-items:center;min-height:44px;padding:0}
	.copy .go,.get .go{margin-top:16px}
	/* the table's rows stack: what it needs, how, then the call names in the quiet line */
	.tbl thead{display:none}
	.tbl,.tbl tbody,.tbl tr,.tbl tbody th,.tbl td{display:block;width:auto}
	.tbl tr{padding:16px 0;border-bottom:1px solid var(--line)}
	.tbl tr:first-child{border-top:1px solid var(--line-hi)}
	.tbl tbody th,.tbl td{padding:0;border:0}
	.tbl td{margin-top:4px}
	.tbl .tech{width:auto;margin-top:8px}
}
@media (max-width:640px){
	:root{--fs-hero:clamp(56px,19vw,96px)}
}
${HERO_CSS[ HERO ]}
/* a phone on its side (proposed; the guide's least height of 620px would push the button below the fold): the tablet
   still stops short of the fold, and the copy sits low on the left, clear of the cue */
@media (max-height:500px) and (orientation:landscape){
	.tablet{min-height:0}
	.hero-b{--vy:38%}
	.hb-copy{left:24px;right:auto;bottom:28px}
	.hb-name{font-size:56px}
	.hb-copy .caps{margin-top:16px}
	.hb-copy .btn{margin-top:20px}
}
@media (prefers-reduced-motion:reduce){
	*,*::before,*::after{transition:none!important}
	html.page{scroll-behavior:auto}
	html.js .rv{opacity:1;transform:none}
	.cue i::after{animation:none;top:18px;opacity:1}
	.btn:hover,.btn:active{transform:none}
}
`

// reveal once, and light the name of the block in the middle of the screen. Both start a change; neither follows the scroll.
const js = `
// FAQ answers open and close with their height, on the guide's one curve.
// Reduced motion leaves the native, instant toggle alone.
for ( const d of document.querySelectorAll( '.qa' ) ) {
	const s = d.querySelector( 'summary' ), a = d.querySelector( '.a' )
	let run = null
	s.addEventListener( 'click', e => {
		if ( matchMedia( '(prefers-reduced-motion: reduce)' ).matches ) return
		e.preventDefault()
		const from = run ? a.offsetHeight : ( d.open ? a.scrollHeight : 0 )
		run?.cancel()
		const opening = !d.open || ( run && run.closing )
		d.open = true
		const to = opening ? a.scrollHeight : 0
		run = a.animate( [ { height: from + 'px', opacity: opening ? 0 : 1 }, { height: to + 'px', opacity: opening ? 1 : 0 } ],
			{ duration: opening ? 400 : 300, easing: 'cubic-bezier(.2,.7,.15,1)' } )
		run.closing = !opening
		run.onfinish = () => { if ( !opening ) d.open = false; run = null }
	} )
}
(() => {
	if (!('IntersectionObserver' in window)) { document.querySelectorAll('.rv').forEach((el) => el.classList.add('in')); return }
	const rv = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add('in'); rv.unobserve(e.target) } }), { threshold: .15 })
	document.querySelectorAll('.rv').forEach((el) => rv.observe(el))
	const lit = new IntersectionObserver((es) => es.forEach((e) => e.target.classList.toggle('current', e.isIntersecting)), { rootMargin: '-40% 0px -40% 0px' })
	document.querySelectorAll('.block').forEach((el) => lit.observe(el))
})()
`

const block = ( id, name, copy, figure, flip ) => `
<section class="block${flip ? ' flip' : ''}" id="${id}" aria-labelledby="${id}-h">
	<div class="copy rv">
		<h2 class="name" id="${id}-h">${name}</h2>
		${copy}
	</div>
	${figure}
</section>`

// The download closes the page inside the footer, so the landing page keeps five blocks (R13).
const html = `<!doctype html>
<html lang="en" class="page">
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
<script>document.documentElement.classList.add('js')</script>
<style>${css}</style>
</head>
<body>
<a class="skip" href="#main">Skip to the content</a>

<div class="wall">
${HEROES[ HERO ]}
	<div class="tablet-edge"></div>
	<i class="cramp t l"></i><i class="cramp t r"></i><i class="cramp b l"></i><i class="cramp b r"></i>
</div>

<main id="main">
${block( 'crosshair', 'The crosshair', `
		<p><b>Make your own crosshair, or bring yours from CS.</b> Scorpio's crosshairs are CS2-based, so CS2 crosshair codes work in Scorpio: paste one, or copy yours.</p>
		<p>Five static styles, thickness in half steps, an outline of 1 to 6 pixels. Save favourites, try one of 13 pro players', or give each game its own.</p>`, shot( 'editor' ) )}

<section class="block wide" id="safety" aria-labelledby="safety-h">
	<h2 class="name" id="safety-h">Outside the game</h2>
	<p class="lead"><b>Scorpio never touches the game or its anti-cheat.</b> It does not open the game, read or change its memory, slip code or hooks into it, fake your keys or mouse, install a driver, or draw into the game's picture.</p>
	<table class="tbl obj">
		<thead><tr>${HEAD.map( h => `<th scope="col">${h}</th>` ).join( '' )}</tr></thead>
		<tbody>
${CALLS.map( ( [ need, how, tech ] ) => `\t\t\t<tr><th scope="row">${need}</th><td>${how}</td><td class="tech">${tech}</td></tr>` ).join( '\n' )}
		</tbody>
	</table>
	<div class="faq obj" id="faq">
${FAQ.map( ( [ q, a ] ) => `\t\t<details class="qa"><summary>${q}<i aria-hidden="true"></i></summary><div class="a"><p>${a}</p></div></details>` ).join( '\n' )}
	</div>
</section>
${block( 'games', 'Only over&nbsp;games', `
		<p>The crosshair shows while a game is in front and hides when you switch away. A game is one <b>you added</b>, any <b>fullscreen</b> window, or a game from a launcher such as Steam, Epic Games or GOG.</p>
		<p>It can also hide while you aim, and keyboard shortcuts lock, centre and switch it.</p>`, shot( 'games' ), true )}
${block( 'efficiency', 'Efficiency', `
		<p><b>Light on memory and idle on the processor.</b> Simple Crosshair Overlay uses less memory, but it redraws all the time and keeps the processor 15 to 40 times busier.</p>
		<p>The download is 2.1 MB. CrossOver's is 101.7 MB and Crosshair Y's 142 MB; Simple Crosshair Overlay is one 1.5 MB file.</p>`, chart() )}
${block( 'obs', 'On stream', `
		<p>OBS's Game Capture records only the game, so add the crosshair as a <b>Browser source</b>.</p>
		<ol class="steps">
			<li>In Scorpio, on the System tab, switch on <b>Show the crosshair in OBS</b>.</li>
			<li>In OBS, add a Browser source above your game capture.</li>
			<li>Set its URL to <code>http://127.0.0.1:47117/obs</code>.</li>
			<li>Make it the size of your canvas, with no transform.</li>
		</ol>`, shot( 'obs' ), true )}
</main>

<footer id="download" aria-labelledby="download-h">
	<div class="get">
		<h2 class="name" id="download-h">Download</h2>
		<ol class="steps">
			<li>On Windows 10 or 11, run the <code>-setup.exe</code>. It needs no administrator rights; Windows may warn you once, as it is unsigned.</li>
			<li>On Linux with X11 or XWayland, run the <code>.AppImage</code> or install the <code>.deb</code>.</li>
			<li>Click the tray icon, make a crosshair or paste a code, and start your game.</li>
		</ol>
		<p>Free for non-commercial use under the PolyForm Noncommercial 1.0.0 licence.</p>
		<a class="go" href="${DOWNLOAD}">Open the latest release</a>
	</div>
	<div class="foot">
		<span class="sig">${mark( 'Scorpio' )}Scorpio</span>
		<nav aria-label="Links">
			<a class="go" href="${REPO}">GitHub</a>
			<a class="go" href="${REPO}/blob/main/LICENSE">Licence</a>
			<a class="go" href="https://lxxvll.com">LXXVII</a>
		</nav>
		<span class="year" title="2026">MMXXVI</span>
	</div>
</footer>
<script>${js}</script>
</body>
</html>
`
fs.writeFileSync( OUT, html )
console.log( OUT, html.length )

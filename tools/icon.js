// Renders Scorpio's icons from the vector sources in assets/brand.
//
//   node tools/icon.js           writes tools/icon.png (1024 px) and the two tray PNGs
//   node tools/icon.js --icons   the same, then runs `npx tauri icon tools/icon.png`,
//                                deletes the outputs Scorpio does not ship, and puts the
//                                pixel-drawn tray icon into the small sizes
//
// Why the small sizes are replaced: Tauri gives the tray the 32 px icon (48 px at
// 150 % scaling) and the shell shrinks it to half. A 1024 px drawing shrunk that far
// is a blur. The tray drawings sit on whole pixels and are stored at exactly double
// size, so they halve cleanly.
//
// The renderer is a headless Chromium. Set CHROME to its path, or let the script
// look in the Playwright cache and the usual install places.
const fs = require( 'fs' )
const os = require( 'os' )
const path = require( 'path' )
const { execFileSync, execSync } = require( 'child_process' )
const { pathToFileURL } = require( 'url' )

const ROOT = path.join( __dirname, '..' )
const BRAND = path.join( ROOT, 'assets', 'brand' )
const ICONS = path.join( ROOT, 'src-tauri', 'icons' )

// size in src-tauri/icons -> the tray drawing it is rendered from
const SMALL = {
	16: 'scorpio-tray.svg',
	24: 'scorpio-tray-24.svg',
	32: 'scorpio-tray.svg',
	48: 'scorpio-tray-24.svg',
}

const findChrome = () => {

	const names = [ 'chrome-headless-shell', 'chrome-headless-shell.exe', 'chrome', 'chrome.exe', path.join( 'Chromium.app', 'Contents', 'MacOS', 'Chromium' ) ]
	const caches = [
		path.join( os.homedir(), 'Library', 'Caches', 'ms-playwright' ),
		path.join( os.homedir(), '.cache', 'ms-playwright' ),
		path.join( process.env.LOCALAPPDATA || '', 'ms-playwright' ),
	]
	const found = []

	if ( process.env.CHROME ) found.push( process.env.CHROME )

	for ( const cache of caches ) {

		if ( ! fs.existsSync( cache ) ) continue

		for ( const build of fs.readdirSync( cache ).filter( name => name.startsWith( 'chromium' ) ).sort().reverse() ) {

			for ( const inner of fs.readdirSync( path.join( cache, build ) ) ) {

				for ( const name of names ) found.push( path.join( cache, build, inner, name ) )

			}

		}

	}

	found.push(
		'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
		'/usr/bin/chromium',
		'/usr/bin/chromium-browser',
		'/usr/bin/google-chrome',
		'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
		'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
	)

	const chrome = found.find( file => fs.existsSync( file ) && fs.statSync( file ).isFile() )

	if ( ! chrome ) throw new Error( 'No Chromium found. Set CHROME to a Chrome, Chromium or Edge binary.' )

	return chrome

}

const chrome = findChrome()

// Draws one SVG file into a square PNG with a transparent ground.
const render = ( svg, png, size ) => {

	execFileSync( chrome, [
		'--headless',
		'--hide-scrollbars',
		'--default-background-color=00000000',
		`--screenshot=${ png }`,
		`--window-size=${ size },${ size }`,
		pathToFileURL( svg ).href,
	], { stdio: 'ignore' } )

	const head = fs.readFileSync( png )

	if ( head.readUInt32BE( 16 ) !== size || head.readUInt32BE( 20 ) !== size ) throw new Error( `${ png } is not ${ size } px square` )

	if ( png.startsWith( ROOT ) ) console.log( path.relative( ROOT, png ), `${ size } px` )

}

// Swaps the PNG entries of the given sizes in an .ico file. Entry order stays as Tauri wrote it.
const patchIco = ( file, layers ) => {

	const ico = fs.readFileSync( file )
	const count = ico.readUInt16LE( 4 )
	const entries = []

	for ( let i = 0; i < count; i++ ) {

		const at = 6 + ( i * 16 )
		const size = ico[at] || 256
		const data = layers[size] || ico.subarray( ico.readUInt32LE( at + 12 ), ico.readUInt32LE( at + 12 ) + ico.readUInt32LE( at + 8 ) )

		entries.push( { head: Buffer.from( ico.subarray( at, at + 16 ) ), data } )

	}

	const missing = Object.keys( layers ).filter( size => ! entries.some( entry => ( entry.head[0] || 256 ) === Number( size ) ) )

	if ( missing.length ) throw new Error( `${ file } has no ${ missing.join( ', ' ) } px entry` )

	let offset = 6 + ( count * 16 )

	for ( const entry of entries ) {

		entry.head.writeUInt32LE( entry.data.length, 8 )
		entry.head.writeUInt32LE( offset, 12 )
		offset += entry.data.length

	}

	fs.writeFileSync( file, Buffer.concat( [ ico.subarray( 0, 6 ), ...entries.map( entry => entry.head ), ...entries.map( entry => entry.data ) ] ) )

}

render( path.join( BRAND, 'scorpio-icon.svg' ), path.join( __dirname, 'icon.png' ), 1024 )
render( path.join( BRAND, 'scorpio-tray.svg' ), path.join( BRAND, 'scorpio-tray-16.png' ), 16 )
render( path.join( BRAND, 'scorpio-tray.svg' ), path.join( BRAND, 'scorpio-tray-32.png' ), 32 )

if ( process.argv.includes( '--icons' ) ) {

	execSync( 'npx tauri icon tools/icon.png', { cwd: ROOT, stdio: 'inherit' } )

	// Scorpio ships for Windows and Linux only
	for ( const name of fs.readdirSync( ICONS ) ) {

		if ( name === 'android' || name === 'ios' || name === 'icon.icns' || name === 'StoreLogo.png' || name.startsWith( 'Square' ) ) fs.rmSync( path.join( ICONS, name ), { recursive: true } )

	}

	const tmp = fs.mkdtempSync( path.join( os.tmpdir(), 'scorpio-icon-' ) )
	const layers = {}

	for ( const [ size, drawing ] of Object.entries( SMALL ) ) {

		const png = path.join( tmp, `${ size }.png` )

		render( path.join( BRAND, drawing ), png, Number( size ) )
		layers[size] = fs.readFileSync( png )

	}

	fs.writeFileSync( path.join( ICONS, '32x32.png' ), layers[32] )
	patchIco( path.join( ICONS, 'icon.ico' ), layers )
	fs.rmSync( tmp, { recursive: true } )
	console.log( 'src-tauri/icons: 16, 24, 32 and 48 px are the pixel-drawn tray icon' )

}

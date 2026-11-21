const assert = require( 'node:assert/strict' )
const { test } = require( 'node:test' )
const { centred } = require( '../../src/main/center' )
const { compare, findGame, fingerprint, fullscreenGame, identifiable, nextState, parseActiveWindow, parseLine, parseWindowProps, saveTarget, toLine } = require( '../../src/main/focus/match' )

const OWN = 4242
const cs2 = { pid: 20, name: 'cs2', path: 'C:\\Steam\\steamapps\\common\\Counter-Strike Global Offensive\\game\\bin\\win64\\cs2.exe', title: 'Counter-Strike 2', windowClass: 'SDL_app', fullscreen: true }
const finals = { pid: 30, name: 'Discovery', path: 'C:\\Steam\\steamapps\\common\\The Finals\\Discovery.exe', title: 'THE FINALS', windowClass: 'UnrealWindow', fullscreen: true }
const browser = { pid: 10, name: 'brave', path: 'C:\\Brave\\brave.exe', title: 'New Tab - Brave', windowClass: 'Chrome_WidgetWin_1', fullscreen: false }
const games = [ { id: 'cs2', fingerprint: fingerprint( cs2 ) }, { id: 'finals', fingerprint: fingerprint( finals ) } ]

test( 'two of four checks make a match; empty checks do not vote', () => {

	assert.equal( compare( fingerprint( cs2 ), cs2 ).agree, 4 )
	assert.equal( compare( fingerprint( cs2 ), { ...cs2, title: 'CS2 - 300 fps', path: 'D:\\moved\\cs2.exe' } ).match, true )
	assert.equal( compare( fingerprint( cs2 ), { ...browser, windowClass: 'SDL_app' } ).match, false )
	assert.deepEqual( compare( fingerprint( cs2 ), { pid: 1, name: 'CS2.EXE', path: cs2.path.toUpperCase().replaceAll( '\\', '/' ) } ).results, { name: true, path: true, title: null, windowClass: null } )

} )

test( 'the best-agreeing saved game wins', () => {

	const twin = { id: 'twin', fingerprint: { ...fingerprint( cs2 ), title: 'x', windowClass: 'y' } }

	assert.equal( findGame( [ twin, ...games ], cs2 ).game.id, 'cs2' )
	assert.equal( findGame( games, browser ), null )

} )

test( 'focus: games show it, other windows hide it, Scorpio\'s own windows show it and keep the game', () => {

	const step = ( previous, front, detect ) => nextState( { previous, front, ownPid: OWN, games, detect } )
	const own = { pid: OWN, name: 'Scorpio' }

	let s = { visible: true, gameId: null, detected: null }
	s = step( s, browser )
	assert.deepEqual( s, { visible: false, gameId: null, detected: null } )
	s = step( s, cs2 )
	assert.deepEqual( s, { visible: true, gameId: 'cs2', detected: null } )
	s = step( s, own )
	assert.deepEqual( s, { visible: true, gameId: 'cs2', detected: null }, 'settings in front: shown, same game' )
	assert.equal( step( step( s, browser ), own ).visible, true, 'settings opened over the browser: shown to tune it' )
	assert.equal( step( s, null ), s )

	const detected = { source: 'Steam', name: 'Apex', key: 'x' }

	assert.deepEqual( step( s, { ...browser, pid: 77 }, () => detected ), { visible: true, gameId: null, detected } )

} )

test( 'a fullscreen window is a game unless it is the desktop or the shell', () => {

	assert.equal( fullscreenGame( cs2 ).key, `exe:${cs2.path.toLowerCase()}` )
	assert.equal( fullscreenGame( browser ), null )
	assert.equal( fullscreenGame( { ...cs2, name: 'explorer', windowClass: 'Progman' } ), null )
	assert.equal( fullscreenGame( { ...cs2, name: 'plasmashell', windowClass: 'plasmashell' } ), null )

} )

test( 'the save hotkey never saves the window used before Scorpio\'s own', () => {

	assert.equal( saveTarget( { frontPid: 20, lastOutside: cs2, ownPid: OWN } ), cs2 )
	assert.equal( saveTarget( { frontPid: OWN, lastOutside: browser, ownPid: OWN } ), null )
	assert.equal( saveTarget( { frontPid: null, lastOutside: null, ownPid: OWN } ), null )
	assert.equal( identifiable( { pid: 1, name: ' ', path: '', title: 't' } ), false )

} )

test( 'watcher lines round-trip and junk is refused', () => {

	assert.deepEqual( parseLine( toLine( cs2 ) ), cs2 )
	assert.deepEqual( parseLine( '7\tapp\t\ta\tb\t0\r' ), { pid: 7, name: 'app', path: '', title: 'a', windowClass: 'b', fullscreen: false } )
	assert.deepEqual( parseLine( toLine( { ...cs2, title: 'tab\there' } ) ).title, 'tab here' )

	for ( const junk of [ '', '0\tx', 'Add-Type : error', '12a\tx' ] ) {

		assert.equal( parseLine( junk ), null, junk )

	}

} )

test( 'X11: the active window and its properties from xprop', () => {

	assert.equal( parseActiveWindow( '_NET_ACTIVE_WINDOW(WINDOW): window id # 0x3a00007' ), '0x3a00007' )
	assert.equal( parseActiveWindow( '_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0' ), null )

	const props = [
		'_NET_WM_PID(CARDINAL) = 4821',
		'WM_CLASS(STRING) = "steam_app_730", "steam_app_730"',
		'_NET_WM_NAME(UTF8_STRING) = "Counter-Strike 2 \\"beta\\""',
		'_NET_WM_STATE(ATOM) = _NET_WM_STATE_FULLSCREEN, _NET_WM_STATE_FOCUSED',
	].join( '\n' )

	assert.deepEqual( parseWindowProps( props ), { pid: 4821, windowClass: 'steam_app_730', title: 'Counter-Strike 2 "beta"', fullscreen: true } )
	assert.deepEqual( parseWindowProps( '_NET_WM_PID:  not found.' ), { pid: null, windowClass: '', title: '', fullscreen: false } )

} )

test( 'centring lands on the display\'s middle; at 150% within half a pixel', () => {

	for ( const area of [ { x: 0, y: 0, width: 2560, height: 1440 }, { x: 1920, y: 1080, width: 1920, height: 1080 }, { x: -1920, y: 0, width: 1920, height: 1080 } ] ) {

		const b = centred( { area, size: { width: 201, height: 125 } } )

		assert.deepEqual( [ b.x + ( b.width / 2 ), b.y + ( b.height / 2 ), b.miss ], [ area.x + ( area.width / 2 ), area.y + ( area.height / 2 ), 0 ] )

	}

	for ( const scale of [ 1.25, 1.5, 1.75 ] ) {

		const b = centred( { area: { x: 0, y: 0, width: Math.round( 2560 / scale ), height: Math.round( 1440 / scale ) }, size: { width: 160, height: 160 }, scale } )

		assert.ok( b.miss <= 0.5 && Number.isInteger( b.x ) && Number.isInteger( b.y ), `${scale}: ${JSON.stringify( b )}` )

	}

} )

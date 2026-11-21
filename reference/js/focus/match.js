// Deciding whether the window in front is a game, from what the platform
// watchers report. Pure: test/unit/focus.test.js runs it under plain node.
//
// A window is described by four things that identify a program, plus whether
// it fills its monitor:
//   { pid, name, path, title, windowClass, fullscreen }
// A saved game matches a window when at least two of the four agree.

const CHECKS = [ 'name', 'path', 'title', 'windowClass' ]
const AGREE = 2

const clean = value => String( value ?? '' ).trim()
const programName = value => clean( value ).toLowerCase().replace( /\.exe$/, '' )

// How each check compares: Windows paths and names ignore case
const SAME = {
	name: ( a, b ) => programName( a ) === programName( b ),
	path: ( a, b ) => clean( a ).toLowerCase().replaceAll( '/', '\\' ) === clean( b ).toLowerCase().replaceAll( '/', '\\' ),
	title: ( a, b ) => clean( a ) === clean( b ),
	windowClass: ( a, b ) => clean( a ) === clean( b ),
}

// One watcher line: pid, name, path, title, class, fullscreen (1 or 0), tab separated
const parseLine = line => {

	const [ pid = '', name = '', path = '', title = '', windowClass = '', fullscreen = '' ] = String( line ).replace( /\r$/, '' ).split( '\t' )

	if ( !/^[1-9]\d*$/.test( pid.trim() ) ) {

		return null

	}

	return { pid: Number( pid ), name: clean( name ), path: clean( path ), title: clean( title ), windowClass: clean( windowClass ), fullscreen: fullscreen.trim() === '1' }

}

const toLine = w => [ w.pid, w.name, w.path, w.title, w.windowClass, w.fullscreen ? 1 : 0 ].map( part => String( part ?? '' ).replace( /[\t\r\n]/g, ' ' ) ).join( '\t' )

// A program can be told apart only by its name or its executable
const identifiable = w => Boolean( clean( w?.name ) || clean( w?.path ) )

const fingerprint = w => Object.fromEntries( CHECKS.map( check => [ check, clean( w?.[check] ) ] ) )

// Each check: true, false, or null when either side has nothing to compare
const compare = ( saved, w ) => {

	const results = {}
	let agree = 0

	for ( const check of CHECKS ) {

		if ( !clean( saved?.[check] ) || !clean( w?.[check] ) ) {

			results[check] = null

		} else {

			results[check] = SAME[check]( saved[check], w[check] )
			agree += results[check] ? 1 : 0

		}

	}

	return { results, agree, match: agree >= AGREE }

}

// The saved game this window is, with the most agreeing checks; or null
const findGame = ( games, w ) => {

	let best = null

	for ( const game of games || [] ) {

		const verdict = compare( game.fingerprint, w )

		if ( verdict.match && ( !best || verdict.agree > best.verdict.agree ) ) {

			best = { game, verdict }

		}

	}

	return best

}

// The desktop and the shell can fill a monitor too
const SHELL = new Set( [ 'progman', 'workerw', 'shell_traywnd', 'windows.ui.core.corewindow', 'desktop_window', 'xfdesktop', 'nautilus-desktop' ] )

// A borderless or fullscreen window counts as a game when nothing more specific knows it
const fullscreenGame = w => {

	if ( !w?.fullscreen || SHELL.has( clean( w.windowClass ).toLowerCase() ) || [ 'explorer', 'plasmashell', 'gnome-shell' ].includes( programName( w.name ) ) ) {

		return null

	}

	return { source: 'Fullscreen', name: clean( w.name ), key: `exe:${clean( w.path || w.name ).toLowerCase()}` }

}

// The state after the window in front changes. Scorpio's own windows show the
// crosshair (it is being tuned) and keep the game that was in focus.
//   previous: { visible, gameId, detected }
const nextState = ( { previous, front, ownPid, games, detect = () => null } ) => {

	if ( !front ) {

		return previous

	}

	if ( front.pid === ownPid ) {

		return { ...previous, visible: true }

	}

	const found = findGame( games, front )

	if ( found ) {

		return { visible: true, gameId: found.game.id, detected: null }

	}

	const detected = detect( front ) || null

	return { visible: Boolean( detected ), gameId: null, detected }

}

// What the "save this game" hotkey saves: the window in front, never the one
// used before a Scorpio window took the focus
const saveTarget = ( { frontPid, lastOutside, ownPid } ) => ( frontPid === null || frontPid === ownPid ? null : lastOutside )

/* Linux (X11) */

// `xprop -root -spy _NET_ACTIVE_WINDOW` prints "_NET_ACTIVE_WINDOW(WINDOW): window id # 0x3a00007"
const parseActiveWindow = line => {

	const match = /window id # (0x[\da-f]+)/i.exec( String( line ) )

	return match && match[1] !== '0x0' ? match[1] : null

}

// `xprop -id <id> _NET_WM_PID WM_CLASS _NET_WM_NAME _NET_WM_STATE`
const parseWindowProps = text => {

	const value = name => new RegExp( `^${name}\\([^)]*\\)\\s*=\\s*(.*)$`, 'm' ).exec( String( text ) )?.[1] ?? ''
	const strings = raw => [ ...raw.matchAll( /"((?:[^"\\]|\\.)*)"/g ) ].map( match => match[1].replace( /\\(.)/g, '$1' ) )
	const pid = Number.parseInt( value( '_NET_WM_PID' ), 10 )

	return {
		pid: Number.isInteger( pid ) ? pid : null,
		// WM_CLASS is "instance", "class"; the class names the program
		windowClass: strings( value( 'WM_CLASS' ) ).at( -1 ) || '',
		title: strings( value( '_NET_WM_NAME' ) )[0] || '',
		fullscreen: value( '_NET_WM_STATE' ).includes( '_NET_WM_STATE_FULLSCREEN' ),
	}

}

module.exports = { AGREE, CHECKS, compare, findGame, fingerprint, fullscreenGame, identifiable, nextState, parseActiveWindow, parseLine, parseWindowProps, programName, saveTarget, toLine }

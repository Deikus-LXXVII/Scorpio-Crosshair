// The settings window. Local state is the truth while the window is open;
// every change is saved through `save_settings`, which returns the
// validated settings. Changes made elsewhere (shortcuts) arrive as events.
'use strict'

const { h, setting, toggle, segmented, select, slider, colorButton, keybind, dialog, toast } = window.ui
const tauri = window.__TAURI__
const invoke = ( name, args ) => tauri.core.invoke( name, args )

const S = {
	tab: 'crosshair',
	view: null,
	settings: null,
	status: null,
	refused: [],
	target: 'main',
	zoom: 1,
	savedPick: null,
	pros: [],
}

const clone = value => JSON.parse( JSON.stringify( value ) )

/* Saving */

let saveTimer = 0
let lastSaved = ''

const save = ( { now = false } = {} ) => {

	clearTimeout( saveTimer )
	const run = async () => {

		const [ settings, refused ] = await invoke( 'save_settings', { settings: S.settings } )

		lastSaved = JSON.stringify( settings )
		S.refused = refused
		if ( refused.length ) {

			toast( 'Those keys are taken, by another program or another shortcut. Choose different keys.', { alert: true } )
			if ( S.tab === 'controls' ) render()

		}

	}

	if ( now ) return run()
	saveTimer = setTimeout( run, 150 )

}

/* The crosshair being edited: the main one or a game's own */

const targetGame = () => ( S.target === 'main' ? null : S.settings.games[S.target] || null )

const editing = () => {

	const game = targetGame()

	return game?.crosshair || S.settings.crosshair

}

const change = mutate => {

	mutate( editing() )
	paintPreview()
	save()

}

/* Preview */

let previewFrame = 0
let previewAsked = 0

const paintPreview = () => {

	cancelAnimationFrame( previewFrame )
	previewFrame = requestAnimationFrame( async () => {

		const well = document.getElementById( 'preview' )

		if ( !well ) return
		// Answers can arrive out of order; only the newest is drawn.
		const asked = ++previewAsked
		const svg = await invoke( 'preview', { crosshair: editing() } )

		if ( asked !== previewAsked ) return
		well.innerHTML = svg
		sizeSvg( well.firstElementChild, S.zoom )
		const code = document.getElementById( 'code' )

		if ( code && document.activeElement !== code ) code.value = await invoke( 'write_code', { crosshair: editing() } )

	} )

}

// SVG units are physical pixels: at 1x the preview is exactly what the
// overlay draws on this screen.
const sizeSvg = ( svg, zoom ) => {

	if ( !svg ) return
	const box = svg.viewBox?.baseVal
	const width = box?.width || Number( svg.getAttribute( 'width' ) ) || 0
	const height = box?.height || Number( svg.getAttribute( 'height' ) ) || 0
	const scale = zoom / window.devicePixelRatio

	svg.setAttribute( 'width', String( width * scale ) )
	svg.setAttribute( 'height', String( height * scale ) )
	svg.style.shapeRendering = zoom === 1 ? 'crispEdges' : 'auto'

}

const thumb = async ( crosshair, node ) => {

	node.innerHTML = await invoke( 'preview', { crosshair } )
	const svg = node.firstElementChild

	if ( !svg ) return
	const box = svg.viewBox?.baseVal
	const size = Math.max( box?.width || 1, box?.height || 1 )
	const fit = Math.min( 1, 64 / size )

	svg.setAttribute( 'width', String( ( box?.width || size ) * fit / window.devicePixelRatio * window.devicePixelRatio ) )
	svg.setAttribute( 'height', String( ( box?.height || size ) * fit ) )

}

/* Status in the title bar */

const paintStatus = () => {

	const node = document.getElementById( 'status' )
	const s = S.status

	if ( !node || !s ) return
	let text = 'Hidden until a game is in front'
	let on = false

	if ( !s.locked ) {

		text = 'Position unlocked: drag the crosshair, then lock it'
		on = true

	} else if ( s.hidden ) {

		text = 'Hidden by shortcut'

	} else if ( s.visible ) {

		text = s.game ? `Shown over ${s.game}` : 'Shown over a detected game'
		on = true

	}

	node.querySelector( '.dot' ).className = `dot${on ? ' on' : ''}`
	node.querySelector( 'span' ).textContent = text

}

/* Tabs */

const crosshairTab = () => {

	const c = editing()
	const styles = S.view.styles.map( ( [ id, name ] ) => [ id, name ] )
	const ownGames = S.settings.games.map( ( g, i ) => [ i, g ] ).filter( ( [ , g ] ) => g.crosshair )
	const targets = [ [ 'main', 'Main crosshair' ], ...ownGames.map( ( [ i, g ] ) => [ i, `${g.name}'s own crosshair` ] ) ]
	const outlineOff = c.outline === 0

	const code = h( 'input.input#code', { spellcheck: 'false', 'aria-label': 'Share code', placeholder: 'Paste a CS2 share code' } )

	code.addEventListener( 'change', async () => {

		const read = await invoke( 'read_code', { code: code.value } )

		if ( !read ) {

			code.setAttribute( 'aria-invalid', 'true' )
			toast( 'That is not a crosshair share code. They start with CSGO- or CS.', { alert: true } )

			return

		}

		code.setAttribute( 'aria-invalid', 'false' )
		const keep = editing().outlineWidth

		change( target => Object.assign( target, read.crosshair, { outlineWidth: keep } ) )
		render()
		toast( read.codeStyle !== read.crosshair.style ? 'Code imported. Its dynamic style is shown at rest.' : 'Code imported.' )

	} )

	const copy = h( 'button.btn', { type: 'button', text: 'Copy', onclick: async () => {

		await navigator.clipboard.writeText( await invoke( 'write_code', { crosshair: editing() } ) )
		toast( 'Share code copied. Paste it in CS2 or send it to a friend.' )

	} } )

	const zoom = segmented( [ [ 1, 'Actual size' ], [ 4, '4×' ] ], S.zoom, z => {

		S.zoom = z
		sizeSvg( document.getElementById( 'preview' ).firstElementChild, z )

	}, { label: 'Preview size' } )

	const left = h( 'section.stage', {},
		targets.length > 1 ? select( targets, S.target, t => {

			S.target = t
			render()

		}, { label: 'Crosshair to edit' } ) : null,
		h( 'div.well.cramps#preview', { 'aria-label': 'Crosshair preview' } ),
		h( 'div.stage-row', {}, zoom ),
		h( 'div.field', {}, h( 'label.field-label', { for: 'code', text: 'Share code' } ), h( 'div.code-row', {}, code, copy ),
			h( 'div.help', { text: 'Paste a code to import it. Your outline width stays, CS2 codes do not carry it.' } ) ),
	)

	const shape = setting( 'Style', null, select( styles, c.style, v => {

		change( t => {

			t.style = v

		} )
		render()

	}, { label: 'Style' } ) )

	const sizes = h( 'div.slab.flush', {},
		shape,
		setting( 'Length', null, slider( { min: 0, max: 40, value: c.length, label: 'Length', onInput: v => change( t => {

			t.length = v

		} ) } ), { stack: true } ),
		setting( 'Thickness', 'In half steps, finer than CS2.', slider( { min: 0, max: 10, step: 0.5, value: c.thickness, label: 'Thickness', format: v => v.toFixed( 1 ), onInput: v => change( t => {

			t.thickness = v

		} ) } ), { stack: true } ),
		setting( 'Gap', null, slider( { min: -10, max: 20, value: c.gap, label: 'Gap', onInput: v => change( t => {

			t.gap = v

		} ) } ), { stack: true } ),
		setting( 'Centre dot', null, toggle( c.dot, v => change( t => {

			t.dot = v

		} ), { label: 'Centre dot' } ) ),
		setting( 'T shape', 'Leaves out the top line.', toggle( c.tShape, v => change( t => {

			t.tShape = v

		} ), { label: 'T shape' } ) ),
	)

	const colour = h( 'div.slab.flush', {},
		setting( 'Colour', null, colorButton( c.color, v => change( t => {

			t.color = v

		} ), { label: 'Crosshair colour' } ) ),
		setting( 'Opacity', null, slider( { min: 0, max: 255, value: c.alpha, label: 'Opacity', format: v => `${Math.round( v / 2.55 )}%`, onInput: v => change( t => {

			t.alpha = v

		} ) } ), { stack: true } ),
	)

	const outline = h( 'div.slab.flush', {},
		setting( 'Outline', null, segmented( [ [ 0, 'None' ], [ 1, 'Full' ], [ 2, 'Top and left' ] ], c.outline, v => {

			change( t => {

				t.outline = v

			} )
			render()

		}, { label: 'Outline' } ) ),
		setting( 'Outline width', 'CS2 draws 1 pixel. Wider outlines stay in Scorpio.', slider( { min: 1, max: 6, value: c.outlineWidth, label: 'Outline width', format: v => `${v} px`, onInput: v => change( t => {

			t.outlineWidth = v

		} ) } ), { stack: true, off: outlineOff } ),
		setting( 'Outline colour', null, colorButton( c.outlineColor, v => change( t => {

			t.outlineColor = v

		} ), { label: 'Outline colour' } ), { off: outlineOff } ),
		setting( 'Outline opacity', null, slider( { min: 0, max: 255, value: c.outlineAlpha, label: 'Outline opacity', format: v => `${Math.round( v / 2.55 )}%`, onInput: v => change( t => {

			t.outlineAlpha = v

		} ) } ), { stack: true, off: outlineOff } ),
	)

	const right = h( 'section.controls', {},
		h( 'h2.t-h4', { text: 'Shape' } ), sizes,
		h( 'h2.t-h4', { text: 'Colour' } ), colour,
		h( 'h2.t-h4', { text: 'Outline' } ), outline,
		savedSection(),
		proSection(),
	)

	return h( 'div.crosshair-tab', {}, left, right )

}

const savedSection = () => {

	const saved = S.settings.saved
	const tiles = h( 'div.tiles' )

	saved.forEach( ( entry, index ) => {

		const preview = h( 'span.thumb' )

		thumb( entry.crosshair, preview )
		tiles.append( h( 'button.tile', {
			type: 'button',
			'aria-pressed': String( S.savedPick === index ),
			title: 'Use this crosshair',
			onclick: () => {

				S.savedPick = index
				change( t => Object.assign( t, clone( entry.crosshair ) ) )
				render()

			},
		}, preview, h( 'span', { text: entry.name } ) ) )

	} )

	const add = h( 'button.btn', { type: 'button', text: 'Save this crosshair', onclick: () => {

		const name = `Crosshair ${saved.length + 1}`

		saved.push( { name, crosshair: clone( editing() ) } )
		S.savedPick = saved.length - 1
		save()
		render()

	} } )

	const picked = saved[S.savedPick]
	const edit = picked ? h( 'div.saved-edit', {},
		h( 'input.input', { value: picked.name, 'aria-label': 'Name', maxlength: '48', oninput: event => {

			picked.name = event.target.value
			save()

		}, onchange: render } ),
		h( 'button.btn', { type: 'button', text: 'Update', title: 'Replace it with the crosshair shown now', onclick: () => {

			picked.crosshair = clone( editing() )
			save()
			render()
			toast( `${picked.name} updated.` )

		} } ),
		h( 'button.btn.danger', { type: 'button', text: 'Delete', onclick: () => {

			saved.splice( S.savedPick, 1 )
			S.savedPick = null
			save()
			render()

		} } ),
	) : null

	return h( 'div.saved', {},
		h( 'div.section-head', {}, h( 'h2.t-h4', { text: 'Your crosshairs' } ), add ),
		saved.length ? tiles : h( 'p.help', { text: 'Save a crosshair to switch back to it in one click, or with the next and previous shortcuts.' } ),
		edit,
	)

}

const proSection = () => {

	if ( !S.pros.length ) return null
	const tiles = h( 'div.tiles' )

	for ( const pro of S.pros ) {

		const preview = h( 'span.thumb' )

		invoke( 'read_code', { code: pro.code } ).then( read => {

			if ( read ) thumb( read.crosshair, preview )

		} )
		tiles.append( h( 'button.tile', { type: 'button', title: pro.team ? `${pro.name}, ${pro.team}` : pro.name, onclick: async () => {

			const read = await invoke( 'read_code', { code: pro.code } )

			if ( !read ) return
			const keep = editing().outlineWidth

			change( t => Object.assign( t, read.crosshair, { outlineWidth: keep } ) )
			render()
			toast( `${pro.name}'s crosshair applied.` )

		} }, preview, h( 'span', { text: pro.name } ) ) )

	}

	return h( 'div.saved', {}, h( 'div.section-head', {}, h( 'h2.t-h4', { text: 'Pro players' } ) ), tiles )

}

const gamesTab = () => {

	const games = S.settings.games
	const shortcut = S.settings.keys.addGame

	const rows = games.map( ( game, index ) => {

		const where = game.fingerprint.path || game.fingerprint.name || game.fingerprint.windowClass

		return h( 'div.game-row', {},
			h( 'div.game-name', {},
				h( 'input.input.inline', { value: game.name, 'aria-label': 'Game name', maxlength: '48', oninput: event => {

					game.name = event.target.value
					save()

				} } ),
				h( 'div.help.path', { text: where, title: where } ),
			),
			h( 'div.game-own', {},
				h( 'span.setting-name', { text: 'Own crosshair' } ),
				toggle( Boolean( game.crosshair ), on => {

					game.crosshair = on ? clone( S.settings.crosshair ) : null
					if ( !on && S.target === index ) S.target = 'main'
					save()
					render()

				}, { label: `${game.name} has its own crosshair` } ),
			),
			h( 'div.game-actions', {},
				game.crosshair ? h( 'button.btn', { type: 'button', text: 'Edit crosshair', onclick: () => {

					S.target = index
					go( 'crosshair' )

				} } ) : null,
				h( 'button.btn.quiet', { type: 'button', text: 'Remove', onclick: () => {

					games.splice( index, 1 )
					if ( S.target === index ) S.target = 'main'
					else if ( typeof S.target === 'number' && S.target > index ) S.target -= 1
					save()
					render()

				} } ),
			),
		)

	} )

	const add = h( 'button.btn.primary', { type: 'button', text: 'Add a running game', onclick: pickWindow } )
	const list = games.length
		? h( 'div.slab.flush.games', {}, rows )
		: h( 'div.empty', {}, h( 'h3', { text: 'No games added yet' } ), h( 'p', { text: 'Add a game so the crosshair shows only while it is in front, with its own crosshair if you like.' } ) )

	return h( 'div.page', {},
		h( 'div.section-head', {}, h( 'h2.t-h1-app', { text: 'Games' } ), add ),
		shortcut ? h( 'p.help.lede', { text: `Or press ${shortcut.replaceAll( '+', ' + ' )} while the game is in front.` } ) : null,
		list,
		h( 'h2.t-h4', { text: 'Without adding' } ),
		h( 'div.slab.flush', {},
			setting( 'Fullscreen windows', 'Show the crosshair over any fullscreen or borderless window, except the desktop.', toggle( S.settings.detect.fullscreen, v => {

				S.settings.detect.fullscreen = v
				save()

			}, { label: 'Fullscreen windows' } ) ),
			setting( 'Games from launchers', S.view.installed
				? `Show the crosshair over the ${S.view.installed} games Steam, Epic, GOG and other launchers installed.`
				: 'Show the crosshair over games Steam, Epic, GOG and other launchers installed.', toggle( S.settings.detect.launchers, v => {

				S.settings.detect.launchers = v
				save()

			}, { label: 'Games from launchers' } ) ),
		),
	)

}

const pickWindow = async () => {

	const windows = await invoke( 'running_windows' )
	const list = h( 'div.window-list' )
	let close = () => {}

	if ( !windows.length ) list.append( h( 'p.help', { text: 'No windows found. Start the game, then try again.' } ) )
	for ( const w of windows ) {

		list.append( h( 'button.window-item', { type: 'button', onclick: async () => {

			const added = await invoke( 'add_window', { window: w } )

			close()
			if ( added ) {

				const fresh = await invoke( 'view' )

				S.settings = fresh.settings
				lastSaved = JSON.stringify( fresh.settings )
				render()
				toast( `${w.name || w.title} added.` )

			} else {

				toast( 'That game is already added.' )

			}

		} }, h( 'b', { text: w.name || w.windowClass } ), h( 'span', { text: w.title } ) ) )

	}

	close = dialog( 'Add a running game', list, [ h( 'button.btn', { type: 'button', text: 'Cancel', onclick: () => close() } ) ] )

}

const BUTTONS = [ [ 'Right', 'Right button' ], [ 'Middle', 'Middle button' ], [ 'Back', 'Back side button' ], [ 'Forward', 'Forward side button' ], [ 'Left', 'Left button' ] ]
const SHORTCUTS = [
	[ 'lock', 'Unlock or lock position', 'Unlocked, drag the crosshair anywhere.' ],
	[ 'center', 'Centre exactly', 'Puts the crosshair back on the exact centre of the screen.' ],
	[ 'hide', 'Hide or show', null ],
	[ 'next', 'Next saved crosshair', null ],
	[ 'previous', 'Previous saved crosshair', null ],
	[ 'addGame', 'Add the game in front', null ],
]

const controlsTab = () => {

	const aim = S.settings.aim
	const off = !aim.hide

	return h( 'div.page', {},
		h( 'h2.t-h1-app', { text: 'Controls' } ),
		h( 'h2.t-h4', { text: 'Aiming' } ),
		h( 'div.slab.flush', {},
			setting( 'Hide while aiming', 'For scoped weapons. Scorpio reads the button the way every game does and sends nothing to the game.', toggle( aim.hide, v => {

				aim.hide = v
				save()
				render()

			}, { label: 'Hide while aiming' } ) ),
			setting( 'Aim button', null, select( BUTTONS, aim.button, v => {

				aim.button = v
				save()

			}, { label: 'Aim button' } ), { off } ),
			setting( 'When pressed', null, segmented( [ [ 'hold', 'Hide while held' ], [ 'toggle', 'Each press hides or shows' ] ], aim.mode, v => {

				aim.mode = v
				save()

			}, { label: 'Aim mode' } ), { off } ),
		),
		h( 'h2.t-h4', { text: 'Shortcuts' } ),
		h( 'p.help.lede', { text: 'Click a shortcut, then press the keys. Escape cancels, Backspace clears.' } ),
		h( 'div.slab.flush', {}, SHORTCUTS.map( ( [ id, name, help ] ) => {

			const refused = S.refused.includes( id )

			return setting( name, refused ? 'Taken, by another program or another shortcut.' : help, keybind( S.settings.keys[id], v => {

				S.settings.keys[id] = v
				S.refused = S.refused.filter( r => r !== id )
				save()

			}, { label: name, error: refused } ) )

		} ) ),
	)

}

const systemTab = () => {

	const s = S.settings
	const windows = S.view.platform === 'windows'
	const [ dx, dy ] = s.offset
	const centred = dx === 0 && dy === 0

	return h( 'div.page', {},
		h( 'h2.t-h1-app', { text: 'System' } ),
		h( 'h2.t-h4', { text: 'Position' } ),
		h( 'div.slab.flush', {},
			setting( 'Position', centred ? 'Exactly centred.' : `${Math.abs( dx )} px ${dx < 0 ? 'left' : 'right'} and ${Math.abs( dy )} px ${dy < 0 ? 'up' : 'down'} from the centre.`, h( 'div.inline-actions', {},
				h( 'button.btn', { type: 'button', text: S.status?.locked === false ? 'Lock' : 'Unlock', onclick: async () => {

					await invoke( 'set_locked', { locked: S.status?.locked === false } )

				} } ),
				h( 'button.btn', { type: 'button', text: 'Centre exactly', disabled: centred, onclick: () => {

					s.offset = [ 0, 0 ]
					save()
					render()

				} } ),
			) ),
		),
		h( 'h2.t-h4', { text: 'Scorpio' } ),
		h( 'div.slab.flush', {},
			setting( 'Start with the system', 'Scorpio waits in the tray and costs nothing until a game is in front.', toggle( s.startWithSystem, v => {

				s.startWithSystem = v
				save()

			}, { label: 'Start with the system' } ) ),
			setting( 'Hide from screen capture', windows
				? 'Screenshots, recordings and streams show the game without the crosshair. Cheat overlays hide from anti-cheat screenshots the same way, so an anti-cheat could take a closer look at Scorpio. Leave it off unless you need it.'
				: 'Not available on Linux: X11 has no way to keep a window out of captures.', toggle( s.hideFromCapture, v => {

				s.hideFromCapture = v
				save()

			}, { label: 'Hide from screen capture', disabled: !windows } ), { off: !windows } ),
		),
		h( 'h2.t-h4', { text: 'Updates' } ),
		updatesBlock(),
		h( 'div.about', {},
			h( 'p.t-h3', { text: `Scorpio ${S.view.version}` } ),
			h( 'p.help', { text: 'A crosshair overlay. It draws its own window over the game and never opens, reads or changes the game or its anti-cheat.' } ),
			h( 'p.help', { text: 'Free for non-commercial use under the PolyForm Noncommercial licence 1.0.0. Made by LXXVII.' } ),
			h( 'button.btn.danger', { type: 'button', text: 'Quit Scorpio', onclick: () => invoke( 'quit_app' ) } ),
		),
	)

}

const UPDATE_TEXT = {
	idle: () => 'Not checked yet.',
	checking: () => 'Checking for a new version.',
	upToDate: () => `Scorpio ${S.view.version} is the newest version.`,
	available: u => `Scorpio ${u.version} is out.`,
	downloading: u => `Downloading Scorpio ${u.version}: ${u.percent}%.`,
	installing: u => `Installing Scorpio ${u.version}. Scorpio restarts in a moment.`,
	failed: u => u.message,
	unsupported: () => 'This copy is updated by your package manager.',
}

const updatesBlock = () => {

	const u = S.update || { state: 'idle' }
	const unsupported = u.state === 'unsupported'
	const busy = [ 'checking', 'downloading', 'installing' ].includes( u.state )
	const offer = u.state === 'available'
	const button = h( `button.btn${offer ? '.primary' : ''}`, {
		type: 'button',
		text: offer ? 'Install and restart' : 'Check for updates',
		disabled: busy || unsupported,
		onclick: async () => {

			if ( offer ) return invoke( 'install_update' )
			S.update = await invoke( 'check_updates' )
			render()

		},
	} )

	return h( 'div.slab.flush', {},
		setting( 'Install updates automatically', unsupported ? null : 'Scorpio checks every few hours. An update installs after no game has been in front for three minutes, then Scorpio restarts.', toggle( !S.settings.updateManually, v => {

			S.settings.updateManually = !v
			save()

		}, { label: 'Install updates automatically', disabled: unsupported } ), { off: unsupported } ),
		setting( 'Version', UPDATE_TEXT[u.state]( u ), button ),
	)

}

const TABS = { crosshair: crosshairTab, games: gamesTab, controls: controlsTab, system: systemTab }

const render = () => {

	const view = document.getElementById( 'view' )
	const scroll = view.scrollTop

	view.replaceChildren( TABS[S.tab]() )
	view.scrollTop = scroll
	if ( S.tab === 'crosshair' ) paintPreview()
	paintStatus()

}

const go = tab => {

	S.tab = tab
	for ( const t of document.querySelectorAll( '.tabs [role="tab"]' ) ) t.setAttribute( 'aria-selected', String( t.dataset.tab === tab ) )
	document.getElementById( 'view' ).scrollTop = 0
	render()

}

const start = async () => {

	const win = tauri.window.getCurrentWindow()

	document.getElementById( 'minimize' ).onclick = () => win.minimize()
	document.getElementById( 'maximize' ).onclick = () => win.toggleMaximize()
	document.getElementById( 'close' ).onclick = () => win.close()

	const tabs = Array.from( document.querySelectorAll( '.tabs [role="tab"]' ) )

	for ( const t of tabs ) t.addEventListener( 'click', () => go( t.dataset.tab ) )
	document.querySelector( '.tabs' ).addEventListener( 'keydown', event => {

		const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key]

		if ( !step ) return
		const next = tabs[( tabs.indexOf( document.activeElement ) + step + tabs.length ) % tabs.length]

		next.focus()
		go( next.dataset.tab )

	} )

	S.view = await invoke( 'view' )
	S.settings = S.view.settings
	S.status = S.view.status
	S.update = S.view.update
	lastSaved = JSON.stringify( S.settings )
	S.pros = ( await invoke( 'pro_crosshairs' ) )?.players || []

	await tauri.event.listen( 'settings', ( { payload } ) => {

		const text = JSON.stringify( payload )

		if ( text === lastSaved ) return
		lastSaved = text
		S.settings = payload
		render()

	} )
	await tauri.event.listen( 'update', ( { payload } ) => {

		S.update = payload
		if ( S.tab === 'system' ) render()

	} )
	await tauri.event.listen( 'status', ( { payload } ) => {

		const lockChanged = S.status?.locked !== payload.locked

		S.status = payload
		paintStatus()
		if ( lockChanged && S.tab === 'system' ) render()

	} )

	render()

}

start()

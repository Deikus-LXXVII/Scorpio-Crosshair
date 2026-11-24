// The settings window. Local state is the truth while the window is open;
// every change is saved through `save_settings`, which returns the
// validated settings. Changes made elsewhere (shortcuts) arrive as events.
'use strict'

const { t, tn, useLanguage, h, setting, toggle, segmented, select, slider, colorButton, keybind, aimbind, dialog, toast } = window.ui
const tauri = window.__TAURI__
const invoke = ( name, args, options ) => tauri.core.invoke( name, args, options )

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

		const { settings, refused, obsError } = await invoke( 'save_settings', { settings: S.settings } )

		lastSaved = JSON.stringify( settings )
		S.refused = refused
		if ( obsError ) {

			S.settings.obs = false
			toast( t( 'The OBS source could not start. {error}', { error: obsError } ), { alert: true } )
			if ( S.tab === 'system' ) render()

		}
		if ( refused.length ) {

			toast( t( 'Those keys are taken, by another program or another shortcut. Choose different keys.' ), { alert: true } )
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

// The image that goes with the crosshair being edited.
const NO_IMAGE = { on: false, name: '', source: '', rendered: '', size: 64, opacity: 255 }

const editingImage = () => {

	const game = targetGame()

	if ( game?.crosshair ) return ( game.image ||= clone( NO_IMAGE ) )

	return ( S.settings.image ||= clone( NO_IMAGE ) )

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
		const image = editingImage()

		if ( image.on && image.rendered ) {

			const url = await imageUrl( image.rendered )

			if ( asked !== previewAsked ) return
			well.replaceChildren( h( 'img.preview-image', { src: url, alt: '', style: { opacity: String( image.opacity / 255 ) } } ) )
			sizeImage( well.firstElementChild, S.zoom )

			return

		}

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

// The rendered PNG is in physical pixels too: at 1x it is what the overlay shows.
const sizeImage = ( img, zoom ) => {

	if ( !img ) return
	const set = () => {

		img.width = img.naturalWidth * zoom / window.devicePixelRatio
		img.height = img.naturalHeight * zoom / window.devicePixelRatio
		img.classList.toggle( 'zoomed', zoom !== 1 )

	}

	if ( img.complete && img.naturalWidth ) set()
	else img.addEventListener( 'load', set, { once: true } )

}

/* The player's own image (picture.js draws it) */

const imageUrls = new Map()

// A kept file as a blob address, fetched once per window.
const imageUrl = async name => {

	if ( !imageUrls.has( name ) ) {

		const bytes = await invoke( 'image_file', { name } )

		imageUrls.set( name, URL.createObjectURL( new Blob( [ bytes ], { type: picture.TYPES[picture.kindOf( name )] } ) ) )

	}

	return imageUrls.get( name )

}

const failed = error => toast( String( error?.message || error ), { alert: true } )

const storeRender = async ( bytes, kind, size ) => invoke( 'store_image', await picture.rasterise( bytes, kind, size ), { headers: { 'x-image': 'render' } } )

// Choose a file, keep it, draw it, show it.
const chooseImage = () => {

	const input = h( 'input', { type: 'file', accept: '.png,.jpg,.jpeg,.webp,.gif,.svg', hidden: true } )
	// The crosshair being edited when the player chose to, whatever is picked meanwhile.
	const image = editingImage()

	input.addEventListener( 'change', async () => {

		const file = input.files?.[0]

		input.remove()
		if ( !file ) return
		const kind = picture.kindOf( file.name )

		if ( !kind ) return toast( t( 'Scorpio takes PNG, JPG, WebP, GIF and SVG images.' ), { alert: true } )
		if ( file.size > picture.FILE_LIMIT ) return toast( t( 'That file is larger than 5 MB. Choose a smaller one.' ), { alert: true } )
		try {

			const bytes = new Uint8Array( await file.arrayBuffer() )
			const size = await picture.naturalSize( bytes, kind )
			const source = await invoke( 'store_image', bytes, { headers: { 'x-image': kind } } )
			const rendered = await storeRender( bytes, kind, size )

			Object.assign( image, { on: true, name: file.name.slice( 0, 120 ), source: source.name, rendered: rendered.name, size } )
			save()
			render()

		} catch ( error ) {

			failed( error )

		}

	} )
	input.addEventListener( 'cancel', () => input.remove() )
	document.body.append( input )
	input.click()

}

// A new size is drawn again from the original file, never from the last PNG.
let resizeTimer = 0
let resizeAsked = 0

const resizeImage = size => {

	const image = editingImage()
	const source = image.source

	clearTimeout( resizeTimer )
	resizeTimer = setTimeout( async () => {

		// Renders can finish out of order; only the newest is kept.
		const asked = ++resizeAsked

		try {

			const bytes = new Uint8Array( await invoke( 'image_file', { name: source } ) )
			const rendered = await storeRender( bytes, picture.kindOf( source ), size )

			if ( asked !== resizeAsked || image.source !== source ) return
			Object.assign( image, { size, rendered: rendered.name } )
			paintPreview()
			save()

		} catch ( error ) {

			failed( error )

		}

	}, 200 )

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
	// The words say whether the crosshair is shown, so there is no dot beside them (rules.md R16).
	let text = t( 'Hidden until a game is in front' )

	if ( !s.locked ) text = t( 'Position unlocked: drag the crosshair, then lock it' )
	else if ( s.hidden ) text = t( 'Hidden by shortcut' )
	else if ( s.exclusive ) text = t( '{game} runs in exclusive fullscreen: switch it to borderless', { game: s.exclusive } )
	else if ( s.visible ) text = s.game ? t( 'Shown over {game}', { game: s.game } ) : t( 'Shown over a detected game' )
	else if ( s.always ) text = t( 'Shown over every window' )

	node.textContent = text
	node.title = text

}

/* A newer version, shown as an arrow in the title bar */

const paintUpdate = () => {

	const button = document.getElementById( 'update' )
	const u = S.update || {}
	const pending = [ 'available', 'downloading', 'installing' ].includes( u.state )

	button.hidden = !pending
	if ( pending ) button.title = t( 'Scorpio {version} is out.', { version: u.version } )

}

/* Tabs */

const crosshairTab = () => {

	const c = editing()
	const styles = S.view.styles.map( ( [ id, name ] ) => [ id, t( name ) ] )
	const ownGames = S.settings.games.map( ( g, i ) => [ i, g ] ).filter( ( [ , g ] ) => g.crosshair )
	const targets = [ [ 'main', t( 'Main crosshair' ) ], ...ownGames.map( ( [ i, g ] ) => [ i, t( "{name}'s own crosshair", { name: g.name } ) ] ) ]
	const outlineOff = c.outline === 0

	const code = h( 'input.input#code', { spellcheck: 'false', 'aria-label': t( 'Share code' ), placeholder: t( 'Paste a CS2 share code' ) } )

	code.addEventListener( 'change', async () => {

		const read = await invoke( 'read_code', { code: code.value } )

		if ( !read ) {

			code.setAttribute( 'aria-invalid', 'true' )
			toast( t( 'That is not a crosshair share code. They start with CSGO- or CS.' ), { alert: true } )

			return

		}

		code.setAttribute( 'aria-invalid', 'false' )
		const keep = editing().outlineWidth

		change( target => Object.assign( target, read.crosshair, { outlineWidth: keep } ) )
		render()
		toast( read.codeStyle !== read.crosshair.style ? t( 'Code imported. Its dynamic style is shown at rest.' ) : t( 'Code imported.' ) )

	} )

	const copy = h( 'button.btn', { type: 'button', text: t( 'Copy' ), onclick: async () => {

		await navigator.clipboard.writeText( await invoke( 'write_code', { crosshair: editing() } ) )
		toast( t( 'Share code copied. Paste it in CS2 or send it to a friend.' ) )

	} } )

	const zoom = segmented( [ [ 1, t( 'Actual size' ) ], [ 4, '4×' ] ], S.zoom, z => {

		S.zoom = z
		const shown = document.getElementById( 'preview' ).firstElementChild

		if ( shown?.tagName === 'IMG' ) sizeImage( shown, z )
		else sizeSvg( shown, z )

	}, { label: t( 'Preview size' ) } )

	const image = editingImage()
	const useImage = image.on && Boolean( image.rendered )
	const kind = segmented( [ [ 'cs2', t( 'Classic' ) ], [ 'image', t( 'Your image' ) ] ], useImage ? 'image' : 'cs2', v => {

		if ( v === 'image' && !image.rendered ) {

			render()
			chooseImage()

			return

		}
		image.on = v === 'image'
		save()
		render()

	}, { label: t( 'Kind of crosshair' ) } )

	const left = h( 'section.stage', {},
		targets.length > 1 ? select( targets, S.target, x => {

			S.target = x
			render()

		}, { label: t( 'Crosshair to edit' ) } ) : null,
		kind,
		h( 'div.well.cramps#preview', { 'aria-label': t( 'Crosshair preview' ) } ),
		h( 'div.stage-row', {}, zoom ),
		useImage ? null : h( 'div.field', {}, h( 'label.field-label', { for: 'code', text: t( 'Share code' ) } ), h( 'div.code-row', {}, code, copy ) ),
	)

	if ( useImage ) return h( 'div.crosshair-tab', {}, left, imageControls( image ) )

	const shape = setting( t( 'Style' ), null, select( styles, c.style, v => {

		change( x => {

			x.style = v

		} )
		render()

	}, { label: t( 'Style' ) } ) )

	const sizes = h( 'div.slab.flush', {},
		shape,
		setting( t( 'Length' ), null, slider( { min: 0, max: 40, value: c.length, label: t( 'Length' ), onInput: v => change( x => {

			x.length = v

		} ) } ), { stack: true } ),
		setting( t( 'Thickness' ), null, slider( { min: 0, max: 10, step: 0.5, value: c.thickness, label: t( 'Thickness' ), format: v => v.toFixed( 1 ), onInput: v => change( x => {

			x.thickness = v

		} ) } ), { stack: true } ),
		setting( t( 'Gap' ), null, slider( { min: -10, max: 20, value: c.gap, label: t( 'Gap' ), onInput: v => change( x => {

			x.gap = v

		} ) } ), { stack: true } ),
		setting( t( 'Centre dot' ), null, toggle( c.dot, v => change( x => {

			x.dot = v

		} ), { label: t( 'Centre dot' ) } ) ),
		setting( t( 'T shape' ), null, toggle( c.tShape, v => change( x => {

			x.tShape = v

		} ), { label: t( 'T shape' ) } ) ),
	)

	const colour = h( 'div.slab.flush', {},
		setting( t( 'Colour' ), null, colorButton( c.color, v => change( x => {

			x.color = v

		} ), { label: t( 'Crosshair colour' ) } ) ),
		setting( t( 'Opacity' ), null, slider( { min: 0, max: 255, value: c.alpha, label: t( 'Opacity' ), format: v => `${Math.round( v / 2.55 )}%`, onInput: v => change( x => {

			x.alpha = v

		} ) } ), { stack: true } ),
	)

	const outline = h( 'div.slab.flush', {},
		setting( t( 'Outline' ), null, segmented( [ [ 0, t( 'None' ) ], [ 1, t( 'Full' ) ], [ 2, t( 'Top and left' ) ] ], c.outline, v => {

			change( x => {

				x.outline = v

			} )
			render()

		}, { label: t( 'Outline' ) } ) ),
		setting( t( 'Outline width' ), null, slider( { min: 1, max: 6, value: c.outlineWidth, label: t( 'Outline width' ), format: v => `${v} px`, onInput: v => change( x => {

			x.outlineWidth = v

		} ) } ), { stack: true, off: outlineOff } ),
		setting( t( 'Outline colour' ), null, colorButton( c.outlineColor, v => change( x => {

			x.outlineColor = v

		} ), { label: t( 'Outline colour' ) } ), { off: outlineOff } ),
		setting( t( 'Outline opacity' ), null, slider( { min: 0, max: 255, value: c.outlineAlpha, label: t( 'Outline opacity' ), format: v => `${Math.round( v / 2.55 )}%`, onInput: v => change( x => {

			x.outlineAlpha = v

		} ) } ), { stack: true, off: outlineOff } ),
	)

	const right = h( 'section.controls', {},
		h( 'h2.t-h4', { text: t( 'Shape' ) } ), sizes,
		h( 'h2.t-h4', { text: t( 'Colour' ) } ), colour,
		h( 'h2.t-h4', { text: t( 'Outline' ) } ), outline,
		savedSection(),
		proSection(),
	)

	return h( 'div.crosshair-tab', {}, left, right )

}

const imageControls = image => h( 'section.controls', {},
	h( 'h2.t-h4', { text: t( 'Image' ) } ),
	h( 'div.slab.flush', {},
		setting( t( 'File' ), image.name || t( 'PNG, JPG, WebP, GIF or SVG, up to 5 MB.' ), h( 'button.btn', { type: 'button', text: t( 'Choose another' ), onclick: chooseImage } ) ),
		setting( t( 'Size' ), null, slider( { min: picture.SIZE_MIN, max: picture.SIZE_LIMIT, value: image.size, label: t( 'Size' ), format: v => `${v} px`, onInput: resizeImage } ), { stack: true } ),
		setting( t( 'Opacity' ), null, slider( { min: 0, max: 255, value: image.opacity, label: t( 'Opacity' ), format: v => `${Math.round( v / 2.55 )}%`, onInput: v => {

			image.opacity = v
			const shown = document.querySelector( '#preview img' )

			if ( shown ) shown.style.opacity = String( v / 255 )
			save()

		} } ), { stack: true } ),
	),
)

const savedSection = () => {

	const saved = S.settings.saved
	const tiles = h( 'div.tiles' )

	saved.forEach( ( entry, index ) => {

		const preview = h( 'span.thumb' )

		thumb( entry.crosshair, preview )
		tiles.append( h( 'button.tile', {
			type: 'button',
			'aria-pressed': String( S.savedPick === index ),
			title: t( 'Use this crosshair' ),
			onclick: () => {

				S.savedPick = index
				editingImage().on = false
				change( x => Object.assign( x, clone( entry.crosshair ) ) )
				render()

			},
		}, preview, h( 'span', { text: entry.name } ) ) )

	} )

	const add = h( 'button.btn', { type: 'button', text: t( 'Save this crosshair' ), onclick: () => {

		const name = t( 'Crosshair {n}', { n: saved.length + 1 } )

		saved.push( { name, crosshair: clone( editing() ) } )
		S.savedPick = saved.length - 1
		save()
		render()

	} } )

	const picked = saved[S.savedPick]
	const edit = picked ? h( 'div.saved-edit', {},
		h( 'input.input', { value: picked.name, 'aria-label': t( 'Name' ), maxlength: '48', oninput: event => {

			picked.name = event.target.value
			save()

		}, onchange: render } ),
		h( 'button.btn', { type: 'button', text: t( 'Update' ), title: t( 'Replace it with the crosshair shown now' ), onclick: () => {

			picked.crosshair = clone( editing() )
			save()
			render()
			toast( t( '{name} updated.', { name: picked.name } ) )

		} } ),
		h( 'button.btn.danger', { type: 'button', text: t( 'Delete' ), onclick: () => {

			saved.splice( S.savedPick, 1 )
			S.savedPick = null
			save()
			render()

		} } ),
	) : null

	return h( 'div.saved', {},
		h( 'div.section-head', {}, h( 'h2.t-h4', { text: t( 'Your crosshairs' ) } ), add ),
		saved.length ? tiles : h( 'p.help', { text: t( 'Save a crosshair to switch back to it in one click, or with the next and previous shortcuts.' ) } ),
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
		tiles.append( h( 'button.tile', { type: 'button', title: pro.team ? t( '{name}, {team}', { name: pro.name, team: pro.team } ) : pro.name, onclick: async () => {

			const read = await invoke( 'read_code', { code: pro.code } )

			if ( !read ) return
			const keep = editing().outlineWidth

			change( x => Object.assign( x, read.crosshair, { outlineWidth: keep } ) )
			render()
			toast( t( "{name}'s crosshair applied.", { name: pro.name } ) )

		} }, preview, h( 'span', { text: pro.name } ) ) )

	}

	return h( 'div.saved', {}, h( 'div.section-head', {}, h( 'h2.t-h4', { text: t( 'Pro players' ) } ) ), tiles )

}

const gamesTab = () => {

	const games = S.settings.games
	const shortcut = S.settings.keys.addGame

	const rows = games.map( ( game, index ) => {

		const where = game.fingerprint.path || game.fingerprint.name || game.fingerprint.windowClass

		return h( 'div.game-row', {},
			h( 'div.game-name', {},
				h( 'input.input.inline', { value: game.name, 'aria-label': t( 'Game name' ), maxlength: '48', oninput: event => {

					game.name = event.target.value
					save()

				} } ),
				h( 'div.help.path', { text: where, title: where } ),
			),
			h( 'div.game-own', {},
				h( 'span.setting-name', { text: t( 'Own crosshair' ) } ),
				toggle( Boolean( game.crosshair ), on => {

					game.crosshair = on ? clone( S.settings.crosshair ) : null
					game.image = on ? clone( S.settings.image || NO_IMAGE ) : clone( NO_IMAGE )
					if ( !on && S.target === index ) S.target = 'main'
					save()
					render()

				}, { label: t( '{name} has its own crosshair', { name: game.name } ) } ),
			),
			h( 'div.game-actions', {},
				game.crosshair ? h( 'button.btn', { type: 'button', text: t( 'Edit crosshair' ), onclick: () => {

					S.target = index
					go( 'crosshair' )

				} } ) : null,
				h( 'button.btn.quiet', { type: 'button', text: t( 'Remove' ), onclick: () => {

					games.splice( index, 1 )
					if ( S.target === index ) S.target = 'main'
					else if ( typeof S.target === 'number' && S.target > index ) S.target -= 1
					save()
					render()

				} } ),
			),
		)

	} )

	const add = h( 'button.btn.primary', { type: 'button', text: t( 'Add a game' ), onclick: pickWindow } )
	const list = games.length
		? h( 'div.slab.flush.games', {}, rows )
		: h( 'div.empty', {}, h( 'h3', { text: t( 'No games added yet' ) } ), h( 'p', { text: t( 'Add a game so the crosshair shows only while it is in front, with its own crosshair if you like.' ) } ) )

	return h( 'div.page', {},
		h( 'div.section-head', {}, h( 'h2.t-h1-app', { text: t( 'Games' ) } ), add ),
		shortcut ? h( 'p.help.lede', { text: t( 'Or press {keys} while the game is in front.', { keys: shortcut.replaceAll( '+', ' + ' ) } ) } ) : null,
		list,
		h( 'h2.t-h4', { text: t( 'Without adding' ) } ),
		h( 'div.slab.flush', {},
			setting( t( 'Fullscreen windows' ), t( 'Show the crosshair over any fullscreen or borderless window, except the desktop.' ), toggle( S.settings.detect.fullscreen, v => {

				S.settings.detect.fullscreen = v
				save()

			}, { label: t( 'Fullscreen windows' ) } ) ),
			setting( t( 'Games from launchers' ), S.view.installed
				? tn( S.view.installed, 'Show the crosshair over the {n} game Steam, Epic, GOG and other launchers installed.', 'Show the crosshair over the {n} games Steam, Epic, GOG and other launchers installed.' )
				: t( 'Show the crosshair over games Steam, Epic, GOG and other launchers installed.' ), toggle( S.settings.detect.launchers, v => {

				S.settings.detect.launchers = v
				save()

			}, { label: t( 'Games from launchers' ) } ) ),
			setting( t( 'Always show' ), t( 'Over every window and the desktop.' ), toggle( S.settings.detect.always, v => {

				S.settings.detect.always = v
				save()

			}, { label: t( 'Always show' ) } ) ),
		),
	)

}

const pickWindow = async () => {

	const windows = await invoke( 'running_windows' )
	const list = h( 'div.window-list' )
	let close = () => {}

	if ( !windows.length ) list.append( h( 'p.help', { text: t( 'No windows found. Start the game, then try again.' ) } ) )
	for ( const w of windows ) {

		list.append( h( 'button.window-item', { type: 'button', onclick: async () => {

			const added = await invoke( 'add_window', { window: w } )

			close()
			if ( added ) {

				const fresh = await invoke( 'view' )

				S.settings = fresh.settings
				lastSaved = JSON.stringify( fresh.settings )
				render()
				toast( t( '{name} added.', { name: w.name || w.title } ) )

			} else {

				toast( t( 'That game is already added.' ) )

			}

		} }, h( 'b', { text: w.name || w.windowClass } ), h( 'span', { text: w.title } ) ) )

	}

	close = dialog( t( 'Add a game' ), list, [ h( 'button.btn', { type: 'button', text: t( 'Cancel' ), onclick: () => close() } ) ] )

}

const shortcuts = () => [
	[ 'lock', t( 'Unlock or lock position' ), null ],
	[ 'center', t( 'Centre exactly' ), null ],
	[ 'hide', t( 'Hide or show' ), null ],
	[ 'next', t( 'Next saved crosshair' ), null ],
	[ 'previous', t( 'Previous saved crosshair' ), null ],
	[ 'addGame', t( 'Add the game in front' ), null ],
]

const controlsTab = () => {

	const aim = S.settings.aim
	const off = !aim.hide

	return h( 'div.page', {},
		h( 'h2.t-h1-app', { text: t( 'Controls' ) } ),
		h( 'h2.t-h4', { text: t( 'Aiming' ) } ),
		h( 'div.slab.flush', {},
			setting( t( 'Hide while aiming' ), null, toggle( aim.hide, v => {

				aim.hide = v
				save()
				render()

			}, { label: t( 'Hide while aiming' ) } ) ),
			setting( t( 'Aim button' ), null, aimbind( aim.button, v => {

				aim.button = v
				save()

			}, { label: t( 'Aim button' ) } ), { off } ),
			setting( t( 'When pressed' ), null, segmented( [ [ 'hold', t( 'Hide while held' ) ], [ 'toggle', t( 'Each press hides or shows' ) ] ], aim.mode, v => {

				aim.mode = v
				save()

			}, { label: t( 'Aim mode' ) } ), { off } ),
		),
		h( 'h2.t-h4', { text: t( 'Shortcuts' ) } ),
		h( 'p.help.lede', { text: t( 'Escape cancels, Backspace clears.' ) } ),
		h( 'div.slab.flush', {}, shortcuts().map( ( [ id, name, help ] ) => {

			const refused = S.refused.includes( id )

			return setting( name, refused ? t( 'Taken, by another program or another shortcut.' ) : help, keybind( S.settings.keys[id], v => {

				S.settings.keys[id] = v
				S.refused = S.refused.filter( r => r !== id )
				save()

			}, { label: name, error: refused } ) )

		} ) ),
	)

}

const offsetText = ( dx, dy ) => {

	const values = { x: Math.abs( dx ), y: Math.abs( dy ) }

	if ( dx < 0 ) return dy < 0 ? t( '{x} px left and {y} px up from the centre.', values ) : t( '{x} px left and {y} px down from the centre.', values )

	return dy < 0 ? t( '{x} px right and {y} px up from the centre.', values ) : t( '{x} px right and {y} px down from the centre.', values )

}

const systemTab = () => {

	const s = S.settings
	const windows = S.view.platform === 'windows'
	const [ dx, dy ] = s.offset
	const centred = dx === 0 && dy === 0

	return h( 'div.page', {},
		h( 'h2.t-h1-app', { text: t( 'System' ) } ),
		h( 'h2.t-h4', { text: t( 'Position' ) } ),
		h( 'div.slab.flush', {},
			setting( t( 'Position' ), centred ? t( 'Exactly centred.' ) : offsetText( dx, dy ), h( 'div.inline-actions', {},
				h( 'button.btn', { type: 'button', text: S.status?.locked === false ? t( 'Lock' ) : t( 'Unlock' ), onclick: async () => {

					await invoke( 'set_locked', { locked: S.status?.locked === false } )

				} } ),
				h( 'button.btn', { type: 'button', text: t( 'Centre exactly' ), disabled: centred, onclick: () => {

					s.offset = [ 0, 0 ]
					save()
					render()

				} } ),
			) ),
			setting( t( 'Centre on the game window' ), null, toggle( s.centreOnWindow, v => {

				s.centreOnWindow = v
				save()

			}, { label: t( 'Centre on the game window' ) } ) ),
		),
		h( 'h2.t-h4', { text: t( 'Scorpio' ) } ),
		h( 'div.slab.flush', {},
			setting( t( 'Language' ), null, select( [ [ 'auto', t( 'Same as the system' ) ], ...S.view.languages ], s.language, async v => {

				const before = s.language

				s.language = v
				try {

					await save( { now: true } )
					location.reload()

				} catch ( error ) {

					s.language = before
					failed( error )
					render()

				}

			}, { label: t( 'Language' ) } ) ),
			setting( t( 'Start with the system' ), null, toggle( s.startWithSystem, v => {

				s.startWithSystem = v
				save()

			}, { label: t( 'Start with the system' ) } ) ),
			setting( t( 'Hide from screen capture' ), windows
				? t( 'Screenshots, recordings and streams show the game without the crosshair. Cheat overlays hide from anti-cheat screenshots the same way, so an anti-cheat could take a closer look at Scorpio. Leave it off unless you need it.' )
				: t( 'Not available on Linux: X11 has no way to keep a window out of captures.' ), toggle( s.hideFromCapture, v => {

				s.hideFromCapture = v
				save()

			}, { label: t( 'Hide from screen capture' ), disabled: !windows } ), { off: !windows } ),
		),
		h( 'h2.t-h4', { text: t( 'Stream' ) } ),
		obsBlock(),
		h( 'h2.t-h4', { text: t( 'Updates' ) } ),
		updatesBlock(),
		h( 'h2.t-h4', { text: t( 'Help' ) } ),
		h( 'div.slab.flush', {},
			setting( t( 'Report a problem' ), null, h( 'button.btn', { type: 'button', text: t( 'Show the report' ), onclick: showReport } ) ),
		),
		h( 'div.about', {},
			h( 'img.lockup', { src: 'brand/lockup.svg', alt: 'Scorpio' } ),
			h( 'p.t-h3', { text: t( 'Version {version}', { version: S.view.version } ) } ),
			h( 'p.help', { text: t( 'A crosshair overlay. It draws its own window over the game and never opens, reads or changes the game or its anti-cheat.' ) } ),
			h( 'p.help', { text: t( 'Free for non-commercial use under the PolyForm Noncommercial licence 1.0.0. Made by LXXVII.' ) } ),
			h( 'button.btn.danger', { type: 'button', text: t( 'Quit Scorpio' ), onclick: () => invoke( 'quit_app' ) } ),
		),
	)

}

// OBS's game capture never contains the overlay window, so the crosshair
// reaches the stream as a Browser source served by Scorpio itself.
const obsBlock = () => {

	const on = S.settings.obs
	const address = S.view.obsAddress
	const rows = [
		setting( t( 'Show the crosshair in OBS' ), null, toggle( on, v => {

			S.settings.obs = v
			save( { now: true } ).then( render )

		}, { label: t( 'Show the crosshair in OBS' ) } ) ),
	]

	if ( on ) {

		rows.push( h( 'div.setting.stack', {},
			h( 'div.setting-name', { text: t( 'In OBS' ) } ),
			h( 'ol.obs-steps', {},
				h( 'li', { text: t( 'Add a Browser source above your game capture.' ) } ),
				h( 'li', {}, t( 'Set its URL to the address below, and its width and height to your canvas size (Settings, Video, Base resolution).' ) ),
				h( 'li', { text: t( 'Keep it filling the whole canvas, untransformed.' ) } ),
			),
			h( 'div.code-row', {},
				h( 'input.input', { value: address, readonly: true, 'aria-label': t( 'Address for OBS' ), onfocus: event => event.target.select() } ),
				h( 'button.btn', { type: 'button', text: t( 'Copy' ), onclick: async () => {

					await navigator.clipboard.writeText( address )
					toast( t( 'Address copied. Paste it into the Browser source in OBS.' ) )

				} } ),
			),
		) )

	}

	return h( 'div.slab.flush', {}, rows )

}

const updateText = {
	idle: () => t( 'Not checked yet.' ),
	checking: () => t( 'Checking for a new version.' ),
	upToDate: () => t( 'Scorpio {version} is the newest version.', { version: S.view.version } ),
	available: u => t( 'Scorpio {version} is out.', { version: u.version } ),
	downloading: u => t( 'Downloading Scorpio {version}: {percent}%.', { version: u.version, percent: u.percent } ),
	installing: u => t( 'Installing Scorpio {version}. Scorpio restarts in a moment.', { version: u.version } ),
	failed: u => u.message,
	unsupported: () => t( 'This copy is updated by your package manager.' ),
}

const updatesBlock = () => {

	const u = S.update || { state: 'idle' }
	const unsupported = u.state === 'unsupported'
	const busy = [ 'checking', 'downloading', 'installing' ].includes( u.state )
	const offer = u.state === 'available'
	const button = h( `button.btn${offer ? '.primary' : ''}`, {
		type: 'button',
		text: offer ? t( 'Install and restart' ) : t( 'Check for updates' ),
		disabled: busy || unsupported,
		onclick: async () => {

			if ( offer ) return invoke( 'install_update' )
			S.update = await invoke( 'check_updates' )
			render()

		},
	} )

	return h( 'div.slab.flush', {},
		setting( t( 'Install updates automatically' ), null, toggle( !S.settings.updateManually, v => {

			S.settings.updateManually = !v
			save()

		}, { label: t( 'Install updates automatically' ), disabled: unsupported } ), { off: unsupported } ),
		setting( t( 'Version' ), ( updateText[u.state] || updateText.idle )( u ), button ),
	)

}

const REPORT_WHAT_LIMIT = 4000
const REPORT_CONTACT_LIMIT = 200

let reportOpening = false

// The report is shown in full before anything happens to it. Rust makes the
// text with the player's notes, so what is shown is what is copied, saved
// and sent. Send posts it to Scorpio's report service; Save and GitHub keep
// it on this computer until the player attaches it.
const showReport = async () => {

	if ( reportOpening ) return
	reportOpening = true
	let base

	try {

		base = await invoke( 'problem_report' )

	} catch ( error ) {

		failed( error )
		return

	} finally {

		reportOpening = false

	}

	const what = h( 'textarea.textarea.report-what#report-what', {
		maxlength: String( REPORT_WHAT_LIMIT ),
		rows: '3',
		placeholder: t( 'What you were doing and what went wrong.' ),
	} )
	const contact = h( 'input.input#report-contact', {
		maxlength: String( REPORT_CONTACT_LIMIT ),
		placeholder: t( 'Optional: Discord name or e-mail' ),
	} )
	const area = h( 'textarea.textarea.report', { readonly: true, spellcheck: 'false', 'aria-label': t( 'The report' ) } )
	const sentLine = h( 'p.report-sent' )
	sentLine.hidden = true
	let saved = null
	let savedText = null
	let sentId = null
	let asked = 0

	area.value = base
	// Answers can come back out of order while typing: only the newest is shown.
	const refresh = async () => {

		const mine = ++asked
		const text = await invoke( 'compose_report', { description: what.value, contact: contact.value } )

		if ( mine === asked ) area.value = text

	}
	const notes = () => ( { description: what.value, contact: contact.value } )

	what.addEventListener( 'input', refresh )
	contact.addEventListener( 'input', refresh )

	const current = async () => {

		await refresh()
		return area.value

	}
	const saveFile = async () => {

		const text = await current()

		if ( !saved || savedText !== text ) {

			saved = await invoke( 'save_report', { text } )
			savedText = text

		}

		return saved

	}

	const send = h( 'button.btn.primary', { type: 'button', text: t( 'Send' ) } )

	send.onclick = async () => {

		if ( sentId ) return
		if ( !what.value.trim() ) {

			toast( t( 'Write what happened first.' ) )
			what.focus()
			return

		}

		send.disabled = true

		try {

			await current()
			sentId = await invoke( 'send_report', notes() )
			what.readOnly = true
			contact.readOnly = true
			sentLine.textContent = t( 'Sent as {id}. Keep this number if you write to us about it.', { id: sentId } )
			sentLine.hidden = false
			send.textContent = t( 'Sent' )

		} catch ( error ) {

			send.disabled = false
			failed( error )

		}

	}

	const body = h( 'div.report-body', {},
		h( 'p', { text: t( "Version, system, hardware, displays, Scorpio's cost, settings and the end of the log. Your user folder and name are taken out. Nothing is sent until you press Send." ) } ),
		h( 'div.field', {},
			h( 'label.field-label', { for: 'report-what', text: t( 'What happened' ) } ),
			what,
		),
		h( 'div.field', {},
			h( 'label.field-label', { for: 'report-contact', text: t( 'How to reach you' ) } ),
			contact,
		),
		area,
		sentLine,
	)
	const close = dialog( t( 'Report a problem' ), body, [
		h( 'button.btn.quiet', { type: 'button', text: t( 'Close' ), onclick: () => close() } ),
		h( 'button.btn', { type: 'button', text: t( 'Copy' ), onclick: async () => {

			await navigator.clipboard.writeText( await current() )
			toast( t( 'Report copied.' ) )

		} } ),
		h( 'button.btn', { type: 'button', text: t( 'Save as file' ), onclick: async () => {

			try {

				toast( t( 'Saved to {path}.', { path: await saveFile() } ) )

			} catch ( error ) {

				failed( error )

			}

		} } ),
		h( 'button.btn', { type: 'button', text: t( 'Report on GitHub' ), onclick: async () => {

			try {

				const path = await saveFile()

				await invoke( 'open_issue', { file: path } )
				toast( t( 'Saved to {path}. Attach that file to the issue.', { path } ) )

			} catch ( error ) {

				failed( error )

			}

		} } ),
		send,
	], { wide: true } )

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
	document.getElementById( 'update' ).onclick = () => go( 'system' )

	const tabs = Array.from( document.querySelectorAll( '.tabs [role="tab"]' ) )

	// Windows can bring this window back in front by itself (when another
	// program's window closes over a game); keys meant for the game must then
	// reach no control.
	window.addEventListener( 'blur', () => document.activeElement?.blur() )

	for ( const t of tabs ) t.addEventListener( 'click', () => go( t.dataset.tab ) )
	document.querySelector( '.tabs' ).addEventListener( 'keydown', event => {

		const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key]

		if ( !step ) return
		const next = tabs[( tabs.indexOf( document.activeElement ) + step + tabs.length ) % tabs.length]

		next.focus()
		go( next.dataset.tab )

	} )

	S.view = await invoke( 'view' )
	useLanguage( S.view.language, S.view.texts )
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
		paintUpdate()
		if ( S.tab === 'system' ) render()

	} )
	await tauri.event.listen( 'status', ( { payload } ) => {

		const lockChanged = S.status?.locked !== payload.locked

		S.status = payload
		paintStatus()
		if ( lockChanged && S.tab === 'system' ) render()

	} )

	paintUpdate()
	render()

}

start()

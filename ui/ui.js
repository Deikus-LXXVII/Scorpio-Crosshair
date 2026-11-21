// Drawn controls for the settings window: switch, segmented control,
// dropdown, slider, colour picker, keybind field, dialog and toast. No
// checkbox, radio or OS list is ever shown.
'use strict'

// A block, so these names stay out of the page's global scope.
{

const h = ( tag, props = {}, ...children ) => {

	const [ head, id ] = tag.split( '#' )
	const [ name, ...classes ] = head.split( '.' )
	const node = document.createElement( name || 'div' )

	if ( classes.length ) node.className = classes.join( ' ' )
	if ( id ) node.id = id

	for ( const [ key, value ] of Object.entries( props ) ) {

		if ( value === undefined || value === null || value === false ) continue
		if ( key.startsWith( 'on' ) ) node.addEventListener( key.slice( 2 ), value )
		else if ( key === 'style' && typeof value === 'object' ) for ( const [ k, v ] of Object.entries( value ) ) node.style.setProperty( k.replace( /[A-Z]/g, c => `-${c.toLowerCase()}` ), v )
		else if ( key === 'text' ) node.textContent = value
		else if ( key in node && typeof value !== 'string' ) node[key] = value
		else node.setAttribute( key, value === true ? '' : value )

	}

	for ( const child of children.flat() ) {

		if ( child === null || child === undefined || child === false ) continue
		node.append( child instanceof Node ? child : document.createTextNode( String( child ) ) )

	}

	return node

}

// A setting row: name and help on the left, one control on the right.
// A row with no help line is `.solo`: its name is centred on the control.
const setting = ( name, help, control, { stack = false, off = false } = {} ) => h(
	`div.setting${stack ? '.stack' : ''}${off ? '.off' : ''}${help ? '' : '.solo'}`, {},
	h( 'div.setting-name', { text: name } ),
	help ? h( 'div.setting-help', { text: help } ) : null,
	control,
)

const toggle = ( value, onChange, { label, disabled } = {} ) => {

	const node = h( 'button.switch', { type: 'button', role: 'switch', 'aria-checked': String( Boolean( value ) ), 'aria-label': label, disabled } )

	node.addEventListener( 'click', () => {

		const next = node.getAttribute( 'aria-checked' ) !== 'true'

		node.setAttribute( 'aria-checked', String( next ) )
		onChange( next )

	} )

	return node

}

const segmented = ( options, value, onChange, { label } = {} ) => {

	const node = h( 'div.seg', { role: 'radiogroup', 'aria-label': label } )
	const buttons = options.map( ( [ id, text ] ) => h( 'button', {
		type: 'button',
		role: 'radio',
		'aria-checked': String( id === value ),
		text,
		onclick: () => {

			for ( const other of buttons ) other.setAttribute( 'aria-checked', String( other === buttons[options.findIndex( o => o[0] === id )] ) )
			onChange( id )

		},
	} ) )

	node.append( ...buttons )
	node.addEventListener( 'keydown', event => {

		const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key]

		if ( !step ) return
		event.preventDefault()
		const index = buttons.indexOf( document.activeElement )
		const next = buttons[( index + step + buttons.length ) % buttons.length]

		next.focus()
		next.click()

	} )

	return node

}

// A dropdown: a button and a drawn list, with arrow keys, Enter and Escape.
const select = ( options, value, onChange, { label, placeholder = 'Choose' } = {} ) => {

	const root = h( 'div.select' )
	const current = () => options.find( o => o[0] === value )
	const button = h( 'button.select-button', { type: 'button', 'aria-haspopup': 'listbox', 'aria-expanded': 'false', 'aria-label': label }, h( 'span' ) )
	const list = h( 'ul.select-list', { role: 'listbox', tabindex: '-1' } )
	let active = -1

	const paint = () => {

		const chosen = current()

		button.classList.toggle( 'placeholder', !chosen )
		button.firstChild.textContent = chosen ? chosen[1] : placeholder

	}

	const close = () => {

		root.classList.remove( 'open' )
		button.setAttribute( 'aria-expanded', 'false' )
		document.removeEventListener( 'pointerdown', outside, true )

	}

	const outside = event => {

		if ( !root.contains( event.target ) ) close()

	}

	const mark = index => {

		active = index
		Array.from( list.children ).forEach( ( item, i ) => item.classList.toggle( 'active', i === index ) )
		list.children[index]?.scrollIntoView( { block: 'nearest' } )

	}

	const choose = index => {

		value = options[index][0]
		paint()
		close()
		button.focus()
		onChange( value )

	}

	const open = () => {

		list.replaceChildren( ...options.map( ( [ id, text ], index ) => h( 'li', {
			role: 'option',
			'aria-selected': String( id === value ),
			text,
			onpointerdown: event => event.preventDefault(),
			onclick: () => choose( index ),
			onpointermove: () => mark( index ),
		} ) ) )
		const below = window.innerHeight - button.getBoundingClientRect().bottom

		root.classList.toggle( 'up', below < Math.min( 288, options.length * 34 + 8 ) )
		root.classList.add( 'open' )
		button.setAttribute( 'aria-expanded', 'true' )
		mark( Math.max( 0, options.findIndex( o => o[0] === value ) ) )
		document.addEventListener( 'pointerdown', outside, true )

	}

	button.addEventListener( 'click', () => ( root.classList.contains( 'open' ) ? close() : open() ) )
	button.addEventListener( 'keydown', event => {

		const isOpen = root.classList.contains( 'open' )

		if ( event.key === 'Escape' && isOpen ) {

			event.preventDefault()
			close()

		} else if ( event.key === 'ArrowDown' || event.key === 'ArrowUp' ) {

			event.preventDefault()
			if ( !isOpen ) open()
			else mark( ( active + ( event.key === 'ArrowDown' ? 1 : -1 ) + options.length ) % options.length )

		} else if ( ( event.key === 'Enter' || event.key === ' ' ) && isOpen ) {

			event.preventDefault()
			choose( active )

		}

	} )
	button.addEventListener( 'blur', () => setTimeout( () => {

		if ( !root.contains( document.activeElement ) ) close()

	}, 0 ) )

	paint()
	root.append( button, list )

	return root

}

// A slider with its value written beside it. `format` shapes the number.
const slider = ( { min, max, step = 1, value, onInput, label, format = v => String( v ) } ) => {

	const output = h( 'output' )
	const input = h( 'input.slider', { type: 'range', min: String( min ), max: String( max ), step: String( step ), 'aria-label': label } )

	const paint = () => {

		const v = Number( input.value )

		input.style.setProperty( '--fill', `${( ( v - min ) / ( max - min ) ) * 100}%` )
		output.textContent = format( v )

	}

	input.value = String( Math.min( max, Math.max( min, value ) ) )
	input.addEventListener( 'input', () => {

		paint()
		onInput( Number( input.value ) )

	} )
	paint()

	return h( 'div.slider-row', {}, input, output )

}

/* Colour */

const hex = rgb => `#${rgb.map( c => c.toString( 16 ).padStart( 2, '0' ) ).join( '' )}`
const parseHex = text => {

	const match = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/i.exec( text.trim() )

	if ( !match ) return null
	const digits = match[1].length === 3 ? match[1].replace( /./g, c => c + c ) : match[1]

	return [ 0, 2, 4 ].map( i => parseInt( digits.slice( i, i + 2 ), 16 ) )

}

const toHsv = ( [ r, g, b ] ) => {

	const [ rr, gg, bb ] = [ r / 255, g / 255, b / 255 ]
	const max = Math.max( rr, gg, bb )
	const d = max - Math.min( rr, gg, bb )
	let hue = 0

	if ( d ) {

		if ( max === rr ) hue = ( ( gg - bb ) / d ) % 6
		else if ( max === gg ) hue = ( ( bb - rr ) / d ) + 2
		else hue = ( ( rr - gg ) / d ) + 4

	}

	return [ ( ( hue * 60 ) + 360 ) % 360, max ? d / max : 0, max ]

}

const fromHsv = ( hue, s, v ) => {

	const f = n => {

		const k = ( n + ( hue / 60 ) ) % 6

		return Math.round( 255 * ( v - ( v * s * Math.max( 0, Math.min( k, 4 - k, 1 ) ) ) ) )

	}

	return [ f( 5 ), f( 3 ), f( 1 ) ]

}

const SWATCHES = [ [ 50, 250, 50 ], [ 0, 255, 255 ], [ 255, 255, 0 ], [ 255, 0, 255 ], [ 255, 255, 255 ], [ 255, 60, 60 ], [ 0, 0, 0 ] ]

let openPopover = null

// A colour button that opens a drawn picker. The pipette uses the system's
// own eye dropper where the web view has one (WebView2 does); it reads the
// screen once, only when pressed.
const colorButton = ( rgb, onChange, { label } = {} ) => {

	const button = h( 'button.color-button', { type: 'button', 'aria-haspopup': 'dialog', 'aria-expanded': 'false', 'aria-label': label } )
	let color = rgb.slice()

	const paint = () => {

		button.style.setProperty( '--swatch', hex( color ) )
		button.textContent = hex( color )

	}

	const set = ( next, from ) => {

		color = next
		paint()
		onChange( color.slice(), from )

	}

	button.addEventListener( 'click', () => {

		if ( openPopover?.owner === button ) return openPopover.close()
		openPopover?.close()
		const [ hue0, s0, v0 ] = toHsv( color )
		let [ hue, sat, val ] = [ hue0, s0, v0 ]
		const sv = h( 'div.sv', { tabindex: '0', 'aria-label': 'Saturation and brightness' }, h( 'i' ) )
		const hueBar = h( 'div.hue', { tabindex: '0', 'aria-label': 'Hue' }, h( 'i' ) )
		const field = h( 'input.input', { value: hex( color ), 'aria-label': 'Hex colour', spellcheck: 'false' } )
		const swatches = h( 'div.swatches', {}, SWATCHES.map( s => h( 'button', { type: 'button', style: { '--swatch': hex( s ) }, 'aria-label': hex( s ), 'aria-pressed': String( hex( s ) === hex( color ) ), onclick: () => apply( s ) } ) ) )
		const pipette = 'EyeDropper' in window ? h( 'button.btn', { type: 'button', text: 'Pick', title: 'Pick a colour anywhere on screen', onclick: async () => {

			try {

				const picked = parseHex( ( await new window.EyeDropper().open() ).sRGBHex )

				if ( picked ) apply( picked )

			} catch {}

		} } ) : null
		const pop = h( 'div.color-pop', { role: 'dialog', 'aria-label': label }, sv, hueBar, h( 'div.color-row', {}, field, pipette ), swatches )

		const paintPop = () => {

			sv.style.setProperty( '--hue', hex( fromHsv( hue, 1, 1 ) ) )
			Object.assign( sv.firstChild.style, { left: `${sat * 100}%`, top: `${( 1 - val ) * 100}%` } )
			hueBar.firstChild.style.left = `${( hue / 360 ) * 100}%`
			if ( document.activeElement !== field ) field.value = hex( color )
			for ( const b of swatches.children ) b.setAttribute( 'aria-pressed', String( b.getAttribute( 'aria-label' ) === hex( color ) ) )

		}

		const apply = next => {

			[ hue, sat, val ] = toHsv( next )
			set( next )
			paintPop()

		}

		const drag = ( node, move ) => node.addEventListener( 'pointerdown', event => {

			node.setPointerCapture( event.pointerId )
			const step = e => {

				const r = node.getBoundingClientRect()

				move( Math.min( 1, Math.max( 0, ( e.clientX - r.left ) / r.width ) ), Math.min( 1, Math.max( 0, ( e.clientY - r.top ) / r.height ) ) )
				set( fromHsv( hue, sat, val ) )
				paintPop()

			}

			step( event )
			node.onpointermove = step
			node.onpointerup = () => {

				node.onpointermove = null

			}

		} )

		drag( sv, ( x, y ) => {

			sat = x
			val = 1 - y

		} )
		drag( hueBar, x => {

			hue = Math.min( 359.9, x * 360 )

		} )
		sv.addEventListener( 'keydown', event => {

			const d = { ArrowLeft: [ -0.02, 0 ], ArrowRight: [ 0.02, 0 ], ArrowUp: [ 0, 0.02 ], ArrowDown: [ 0, -0.02 ] }[event.key]

			if ( !d ) return
			event.preventDefault()
			sat = Math.min( 1, Math.max( 0, sat + d[0] ) )
			val = Math.min( 1, Math.max( 0, val + d[1] ) )
			set( fromHsv( hue, sat, val ) )
			paintPop()

		} )
		hueBar.addEventListener( 'keydown', event => {

			const d = { ArrowLeft: -3, ArrowRight: 3 }[event.key]

			if ( !d ) return
			event.preventDefault()
			hue = ( hue + d + 360 ) % 360
			set( fromHsv( hue, sat, val ) )
			paintPop()

		} )
		field.addEventListener( 'input', () => {

			const parsed = parseHex( field.value )

			field.setAttribute( 'aria-invalid', String( !parsed ) )
			if ( parsed ) apply( parsed )

		} )

		const place = () => {

			const r = button.getBoundingClientRect()
			const top = r.bottom + 4 + pop.offsetHeight > window.innerHeight ? Math.max( 8, r.top - 4 - pop.offsetHeight ) : r.bottom + 4

			Object.assign( pop.style, { position: 'fixed', left: `${Math.min( r.left, window.innerWidth - 252 )}px`, top: `${top}px` } )

		}

		const close = () => {

			pop.remove()
			button.setAttribute( 'aria-expanded', 'false' )
			document.removeEventListener( 'pointerdown', outside, true )
			document.removeEventListener( 'keydown', escape, true )
			openPopover = null

		}

		const outside = event => {

			if ( !pop.contains( event.target ) && event.target !== button ) close()

		}

		const escape = event => {

			if ( event.key === 'Escape' ) {

				event.stopPropagation()
				close()
				button.focus()

			}

		}

		document.body.append( pop )
		place()
		paintPop()
		button.setAttribute( 'aria-expanded', 'true' )
		document.addEventListener( 'pointerdown', outside, true )
		document.addEventListener( 'keydown', escape, true )
		openPopover = { owner: button, close }

	} )

	paint()

	return button

}

/* Keybinds */

const KEY_NAMES = {
	Period: 'Period', Comma: 'Comma', Slash: 'Slash', Semicolon: 'Semicolon', Quote: 'Quote', BracketLeft: 'BracketLeft', BracketRight: 'BracketRight',
	Backslash: 'Backslash', Minus: 'Minus', Equal: 'Equal', Backquote: 'Backquote', Space: 'Space', Enter: 'Enter', Tab: 'Tab',
	ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown', Insert: 'Insert', Delete: 'Delete',
}
const SHOWN = { Period: '.', Comma: ',', Slash: '/', Semicolon: ';', Quote: '\'', BracketLeft: '[', BracketRight: ']', Backslash: '\\', Minus: '-', Equal: '=', Backquote: '`', Up: '↑', Down: '↓', Left: '←', Right: '→' }

const keyName = event => {

	const { code } = event

	if ( /^Key[A-Z]$/.test( code ) ) return code.slice( 3 )
	if ( /^Digit\d$/.test( code ) ) return code.slice( 5 )
	if ( /^F\d{1,2}$/.test( code ) ) return code
	if ( /^Numpad\d$/.test( code ) ) return code

	return KEY_NAMES[code] || null

}

// Click, then press the shortcut. Escape cancels, Backspace clears. A key
// without Ctrl, Alt or Super would fire in games too, so only F-keys may
// stand alone.
const keybind = ( accelerator, onChange, { label, error } = {} ) => {

	const node = h( 'button.keybind', { type: 'button', 'aria-label': label, 'aria-invalid': String( Boolean( error ) ) } )
	let listening = false

	const paint = text => {

		node.replaceChildren()
		if ( listening ) node.append( h( 'span.hint', { text: 'Press the shortcut' } ) )
		else if ( !text ) node.append( h( 'span.hint', { text: 'Not set' } ) )
		else node.append( ...text.split( '+' ).map( part => h( 'kbd', { text: SHOWN[part] || part } ) ) )

	}

	const stop = () => {

		listening = false
		node.classList.remove( 'listening' )
		paint( accelerator )

	}

	node.addEventListener( 'click', () => {

		listening = true
		node.classList.add( 'listening' )
		paint()

	} )
	node.addEventListener( 'blur', stop )
	node.addEventListener( 'keydown', event => {

		if ( !listening ) return
		event.preventDefault()
		event.stopPropagation()
		if ( event.key === 'Escape' ) return stop()
		const mods = [ event.ctrlKey && 'Ctrl', event.shiftKey && 'Shift', event.altKey && 'Alt', event.metaKey && 'Super' ].filter( Boolean )

		if ( ( event.key === 'Backspace' ) && !mods.length ) {

			accelerator = ''
			stop()
			onChange( '' )

			return

		}

		const key = keyName( event )

		if ( !key ) return
		if ( !/^F\d/.test( key ) && !mods.some( m => m !== 'Shift' ) ) return
		accelerator = [ ...mods, key ].join( '+' )
		stop()
		onChange( accelerator )

	} )

	paint( accelerator )

	return node

}

/* Dialog and toast */

const dialog = ( title, body, actions ) => {

	const previous = document.activeElement
	const box = h( 'div.dialog.cramps.lg', { role: 'dialog', 'aria-modal': 'true', 'aria-label': title }, h( 'h2', { text: title } ), body, h( 'div.actions', {}, actions ) )
	const scrim = h( 'div.scrim', {}, box )

	const close = () => {

		scrim.remove()
		document.removeEventListener( 'keydown', escape, true )
		previous?.focus?.()

	}

	const escape = event => {

		if ( event.key === 'Escape' ) {

			event.stopPropagation()
			close()

		}

	}

	scrim.addEventListener( 'pointerdown', event => {

		if ( event.target === scrim ) close()

	} )
	document.addEventListener( 'keydown', escape, true )
	document.body.append( scrim )
	box.querySelector( 'button, input' )?.focus()

	return close

}

let toastTimer = 0

const toast = ( text, { alert = false } = {} ) => {

	const node = document.getElementById( 'toast' )

	node.textContent = text
	node.classList.toggle( 'alert', alert )
	node.hidden = false
	clearTimeout( toastTimer )
	toastTimer = setTimeout( () => {

		node.hidden = true

	}, 3200 )

}

window.ui = { h, setting, toggle, segmented, select, slider, colorButton, keybind, dialog, toast, hex }

}

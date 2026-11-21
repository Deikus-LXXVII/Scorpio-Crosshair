 
// Scorpio's crosshair is Counter-Strike 2's: the same settings, the same share
// codes, drawn pixel for pixel on the player's screen.
//
// Share code byte layouts are the ones documented by akiver/csgo-sharecode
// (doc/code_layout.md, MIT). A code is a number in base 57 written least
// significant digit first; its bytes are big-endian and byte 0 is the sum of
// the rest, mod 256.

const ALPHABET = 'ABCDEFGHJKLMNOPQRSTUVWXYZabcdefhijkmnopqrstuvwxyz23456789'
const DIGIT = `[${ALPHABET}]`
const CODE_CS2 = new RegExp( `^CS${DIGIT}{44}$` )
const CODE_LEGACY = new RegExp( `^CSGO(?:-?${DIGIT}{5}){5}$` )

// Only styles that look the same with or without the game telling us about
// shots and movement; an overlay never knows either
const STYLES = [
	{ id: 4, name: 'Static Cross', shape: 'cross' },
	{ id: 3, name: 'Static Circle', shape: 'circle' },
	{ id: 6, name: 'Dot Only', shape: 'dot' },
	{ id: 8, name: 'Static Square', shape: 'square' },
	{ id: 9, name: 'Static Quad', shape: 'quad' },
]

// A dynamic style in an imported code becomes the static style it rests as
const AT_REST = { 0: 4, 1: 3, 2: 4, 5: 4, 7: 9 }

const DEFAULTS = Object.freeze( {
	style: 4,
	length: 8,
	thickness: 2,
	gap: 4,
	dot: false,
	tShape: false,
	color: [ 50, 250, 50 ],
	alpha: 255,
	outline: 0, // 0 none, 1 full, 2 top and left only
	outlineWidth: 1, // Scorpio's own: CS2 draws outlines 1 px wide
	outlineColor: [ 0, 0, 0 ],
	outlineAlpha: 255,
	madeFor: 1080, // the screen height the sizes were made at
	// Carried so a code survives a round trip; not drawn
	extra: { followRecoil: false, spreadLimit: 255, splitDistance: 7, innerSplitAlpha: 1, outerSplitAlpha: 0.4, splitSizeRatio: 0.3, scopeDotScale: 1, scopeDotUsesColor: false },
} )

const within = ( value, min, max, fallback ) => {

	const number = Number( value )

	return Number.isFinite( number ) ? Math.min( max, Math.max( min, number ) ) : fallback

}

const whole = ( value, min, max, fallback ) => Math.round( within( value, min, max, fallback ) )
const halves = ( value, min, max, fallback ) => Math.round( within( value, min, max, fallback ) * 2 ) / 2
const hundredths = ( value, min, max, fallback ) => Math.round( within( value, min, max, fallback ) * 100 ) / 100
const rgb = ( value, fallback ) => ( Array.isArray( value ) && value.length === 3 ? value.map( ( channel, i ) => whole( channel, 0, 255, fallback[i] ) ) : [ ...fallback ] )

const restingStyle = style => ( STYLES.some( entry => entry.id === style ) ? style : AT_REST[style] ?? DEFAULTS.style )

// Any input, made into settings Scorpio can draw and encode
const normalize = input => {

	const s = { ...DEFAULTS, ...input }
	const e = { ...DEFAULTS.extra, ...input?.extra }

	return {
		style: restingStyle( whole( s.style, 0, 31, DEFAULTS.style ) ),
		length: whole( s.length, 0, 255, DEFAULTS.length ),
		thickness: halves( s.thickness, 0, 32, DEFAULTS.thickness ),
		gap: whole( s.gap, -3840, 3840, DEFAULTS.gap ),
		dot: s.dot === true,
		tShape: s.tShape === true,
		color: rgb( s.color, DEFAULTS.color ),
		alpha: whole( s.alpha, 0, 255, DEFAULTS.alpha ),
		outline: whole( s.outline, 0, 2, DEFAULTS.outline ),
		outlineWidth: whole( s.outlineWidth, 1, 6, DEFAULTS.outlineWidth ),
		outlineColor: rgb( s.outlineColor, DEFAULTS.outlineColor ),
		outlineAlpha: whole( s.outlineAlpha, 0, 255, DEFAULTS.outlineAlpha ),
		madeFor: whole( s.madeFor, 240, 8640, DEFAULTS.madeFor ),
		extra: {
			followRecoil: e.followRecoil === true,
			spreadLimit: whole( e.spreadLimit, 0, 255, 255 ),
			splitDistance: whole( e.splitDistance, 0, 127, 7 ),
			innerSplitAlpha: hundredths( e.innerSplitAlpha, 0, 1, 1 ),
			outerSplitAlpha: hundredths( e.outerSplitAlpha, 0.3, 1, 0.4 ),
			splitSizeRatio: hundredths( e.splitSizeRatio, 0, 1, 0.3 ),
			scopeDotScale: hundredths( e.scopeDotScale, 0.1, 2, 1 ),
			scopeDotUsesColor: e.scopeDotUsesColor === true,
		},
	}

}

/* Share codes */

const toBytes = ( digits, size ) => {

	let value = 0n

	for ( let i = digits.length - 1; i >= 0; i-- ) {

		value = ( value * 57n ) + BigInt( ALPHABET.indexOf( digits[i] ) )

	}

	const bytes = []

	for ( let i = 0; i < size; i++ ) {

		bytes.unshift( Number( value & 0xffn ) )
		value >>= 8n

	}

	if ( value !== 0n ) {

		throw new Error( 'Not a crosshair code' )

	}

	return bytes

}

const toDigits = ( bytes, count ) => {

	let value = bytes.reduce( ( total, byte ) => ( total << 8n ) | BigInt( byte ), 0n )
	let digits = ''

	for ( let i = 0; i < count; i++ ) {

		digits += ALPHABET[Number( value % 57n )]
		value /= 57n

	}

	return digits

}

const sumOk = bytes => bytes[0] === bytes.slice( 1 ).reduce( ( total, byte ) => total + byte, 0 ) % 256
const signed8 = byte => ( byte > 127 ? byte - 256 : byte )
const signed16 = ( low, high ) => {

	const value = low + ( high * 256 )

	return value > 32_767 ? value - 65_536 : value

}

const uint32 = ( bytes, at ) => bytes[at] + ( bytes[at + 1] * 2 ** 8 ) + ( bytes[at + 2] * 2 ** 16 ) + ( bytes[at + 3] * 2 ** 24 )
const bits = ( value, from, count ) => Math.floor( value / 2 ** from ) % 2 ** count

// Round half to even, as CS:GO did when it scaled sizes
const roundEven = value => {

	const floor = Math.floor( value )

	return Math.abs( value - floor - 0.5 ) < 1e-9 ? floor + ( floor % 2 ) : Math.round( value )

}

// cs2-v1, CS2 since 30/09/2026: 32 bytes behind "CS"
const readCs2 = bytes => {

	const field = uint32( bytes, 18 )

	return {
		madeFor: bytes[2] + ( bytes[3] * 256 ),
		style: bytes[4] & 0x1f,
		dot: ( bytes[4] & 0x40 ) !== 0,
		tShape: ( bytes[4] & 0x80 ) !== 0,
		color: [ bytes[5], bytes[6], bytes[7] ],
		alpha: bytes[8],
		outlineColor: [ bytes[9], bytes[10], bytes[11] ],
		outlineAlpha: bytes[12],
		thickness: bytes[13] & 0x3f,
		outline: bytes[13] >> 6,
		gap: signed16( bytes[14], bytes[15] ),
		length: bytes[16],
		extra: {
			followRecoil: ( bytes[4] & 0x20 ) !== 0,
			spreadLimit: bytes[17],
			splitDistance: bits( field, 0, 7 ),
			innerSplitAlpha: bits( field, 7, 7 ) / 100,
			outerSplitAlpha: ( bits( field, 14, 7 ) + 30 ) / 100,
			splitSizeRatio: bits( field, 21, 7 ) / 100,
			scopeDotUsesColor: bits( field, 28, 1 ) === 1,
			scopeDotScale: ( bytes[22] + 10 ) / 100,
		},
	}

}

// legacy-v3 (23/09/2026) and legacy-v4 (24/09/2026): pixels at madeFor
const readPixelEra = ( bytes, version ) => {

	const field = uint32( bytes, 10 )

	return {
		style: bytes[2] & 0x0f,
		dot: ( bytes[2] & 0x40 ) !== 0,
		tShape: ( bytes[2] & 0x80 ) !== 0,
		outline: version === 3 ? Number( ( bytes[2] & 0x20 ) !== 0 ) : bits( field, 28, 2 ),
		color: [ bytes[3], bytes[4], bytes[5] ],
		alpha: bytes[6],
		gap: bytes[7],
		length: bytes[8],
		thickness: bits( field, 23, 5 ),
		madeFor: bytes[14] + ( bytes[15] * 256 ),
		extra: {
			followRecoil: ( bytes[2] & 0x10 ) !== 0,
			spreadLimit: bytes[9],
			splitDistance: bits( field, 0, 7 ),
			innerSplitAlpha: bits( field, 7, 5 ) * 0.05,
			outerSplitAlpha: 0.3 + ( bits( field, 12, 4 ) * 0.05 ),
			splitSizeRatio: bits( field, 16, 7 ) / 100,
		},
	}

}

// legacy-v1, CS:GO and CS2 before 23/09/2026: sizes in tenths, drawn by the old
// renderer. Placed at `screenHeight` with the community's reading of that
// renderer: length and thickness scale by height / 480, the gap is 4 + gap.
const readTenths = ( bytes, screenHeight ) => {

	const scale = screenHeight / 480

	return {
		style: ( bytes[13] & 0x0e ) >> 1,
		dot: ( bytes[13] & 0x10 ) !== 0,
		tShape: ( bytes[13] & 0x80 ) !== 0,
		color: [ bytes[4], bytes[5], bytes[6] ],
		alpha: ( bytes[13] & 0x40 ) !== 0 ? bytes[7] : 255,
		outline: Number( ( bytes[10] & 0x08 ) !== 0 ),
		outlineWidth: Math.max( 1, Math.round( bytes[3] / 2 ) ),
		gap: Math.trunc( 4 + ( signed8( bytes[2] ) / 10 ) ),
		length: roundEven( ( bytes[14] / 10 ) * scale ),
		thickness: Math.max( 1, roundEven( ( bytes[12] / 10 ) * scale ) ),
		madeFor: screenHeight,
		extra: {
			followRecoil: ( bytes[8] & 0x80 ) !== 0,
			splitDistance: bytes[8] & 0x07,
			innerSplitAlpha: ( bytes[10] >> 4 ) / 10,
			outerSplitAlpha: Math.max( 0.3, ( bytes[11] & 0x0f ) / 10 ),
			splitSizeRatio: ( bytes[11] >> 4 ) / 10,
		},
	}

}

// Reads any CS2 or CS:GO crosshair code. The raw style is kept as `codeStyle`
// so a caller can tell a dynamic style was set to rest.
const readCode = ( text, { screenHeight = 1080 } = {} ) => {

	const code = String( text ?? '' ).trim()
	let raw
	let format

	if ( CODE_CS2.test( code ) && !code.startsWith( 'CSGO' ) ) {

		const bytes = toBytes( code.slice( 2 ), 32 )

		if ( sumOk( bytes ) && bytes[1] === 1 ) {

			raw = readCs2( bytes )
			format = 'cs2-v1'

		}

	} else if ( CODE_LEGACY.test( code ) ) {

		const bytes = toBytes( code.replace( /CSGO|-/g, '' ), 18 )

		if ( sumOk( bytes ) && bytes[1] === 1 ) {

			raw = readTenths( bytes, screenHeight )
			format = 'legacy-v1'

		} else if ( sumOk( bytes ) && ( bytes[1] === 3 || bytes[1] === 4 ) ) {

			raw = readPixelEra( bytes, bytes[1] )
			format = `legacy-v${bytes[1]}`

		}

	}

	if ( !raw ) {

		throw new Error( 'Not a crosshair code' )

	}

	return { format, codeStyle: raw.style, settings: normalize( raw ) }

}

// The cs2-v1 code CS2 imports today
const writeCode = input => {

	const s = normalize( input )
	const bytes = new Array( 32 ).fill( 0 )
	const field = s.extra.splitDistance
		+ ( Math.round( s.extra.innerSplitAlpha * 100 ) * 2 ** 7 )
		+ ( ( Math.round( s.extra.outerSplitAlpha * 100 ) - 30 ) * 2 ** 14 )
		+ ( Math.round( s.extra.splitSizeRatio * 100 ) * 2 ** 21 )
		+ ( Number( s.extra.scopeDotUsesColor ) * 2 ** 28 )
	const gap = s.gap < 0 ? s.gap + 65_536 : s.gap

	bytes[1] = 1
	bytes[2] = s.madeFor % 256
	bytes[3] = Math.floor( s.madeFor / 256 )
	bytes[4] = s.style + ( s.extra.followRecoil ? 0x20 : 0 ) + ( s.dot ? 0x40 : 0 ) + ( s.tShape ? 0x80 : 0 )
	bytes.splice( 5, 8, ...s.color, s.alpha, ...s.outlineColor, s.outlineAlpha )
	// CS2 keeps whole pixels; a half step rounds
	bytes[13] = Math.round( s.thickness ) + ( s.outline * 64 )
	bytes[14] = gap % 256
	bytes[15] = Math.floor( gap / 256 )
	bytes[16] = s.length
	bytes[17] = s.extra.spreadLimit
	bytes.splice( 18, 4, field % 256, Math.floor( field / 2 ** 8 ) % 256, Math.floor( field / 2 ** 16 ) % 256, Math.floor( field / 2 ** 24 ) % 256 )
	bytes[22] = Math.round( s.extra.scopeDotScale * 100 ) - 10
	bytes[0] = bytes.reduce( ( total, byte ) => total + byte, 0 ) % 256

	return `CS${toDigits( bytes, 44 )}`

}

/* Drawing */

// CS2 scales every size by the screen's height over the height it was made at.
// Thickness keeps quarter pixels, drawn as a soft edge.
const pixels = ( input, screenHeight ) => {

	const s = normalize( input )
	const k = screenHeight / s.madeFor

	return {
		length: Math.round( s.length * k ),
		thickness: Math.round( s.thickness * k * 4 ) / 4,
		gap: Math.round( s.gap * k ),
		outline: Math.max( 1, Math.round( s.outlineWidth * k ) ),
	}

}

const css = ( [ r, g, b ], alpha ) => `rgba(${r},${g},${b},${( alpha / 255 ).toFixed( 3 )})`

// An SVG whose units are physical pixels, `size` units square, the crosshair's
// centre on the line between the two middle pixels
const render = ( input, { screenHeight = 1080 } = {} ) => {

	const s = normalize( input )
	const p = pixels( s, screenHeight )
	const shape = STYLES.find( entry => entry.id === s.style ).shape
	const t = shape === 'dot' ? Math.max( 1, p.thickness ) : p.thickness
	// Bars start on whole pixels; a fractional thickness adds a soft far edge
	const base = Math.floor( t )
	const reach = Math.max( 0, p.gap ) + p.length + Math.ceil( t ) + p.outline + 2
	const size = 2 * ( reach + 2 )
	const c = size / 2
	const from = c - Math.floor( base / 2 )
	const bars = []
	const bar = ( x, y, w, h ) => {

		if ( w > 0 && h > 0 ) {

			bars.push( { x, y, w, h } )

		}

	}

	if ( shape === 'cross' && t > 0 ) {

		bar( from + base + p.gap, from, p.length, t )
		bar( from - p.gap - p.length, from, p.length, t )
		bar( from, from + base + p.gap, t, p.length )

		if ( !s.tShape ) {

			bar( from, from - p.gap - p.length, t, p.length )

		}

	}

	if ( ( shape === 'square' || shape === 'quad' ) && t > 0 ) {

		// A frame around the gap; the quad keeps only its corners
		const w = Math.max( 1, Math.round( t ) )
		const inner = from - Math.max( 0, p.gap )
		const span = w + ( 2 * Math.max( 0, p.gap ) )
		const outer = span + ( 2 * w )

		if ( shape === 'square' ) {

			bar( inner - w, inner - w, outer, w )
			bar( inner - w, inner + span, outer, w )
			bar( inner - w, inner, w, span )
			bar( inner + span, inner, w, span )

		} else {

			const arm = Math.min( outer, Math.max( 2 * w, p.length ) )

			for ( const [ x, y ] of [ [ inner - w, inner - w ], [ inner - w + outer - arm, inner - w ], [ inner - w, inner + span ], [ inner - w + outer - arm, inner + span ] ] ) {

				bar( x, y, arm, w )

			}

			for ( const [ x, y ] of [ [ inner - w, inner - w ], [ inner + span, inner - w ], [ inner - w, inner - w + outer - arm ], [ inner + span, inner - w + outer - arm ] ] ) {

				bar( x, y, w, arm )

			}

		}

	}

	if ( s.dot || shape === 'dot' ) {

		const d = Math.max( 1, t )

		bar( from, from, d, d )

	}

	const o = p.outline
	const edges = s.outline === 0 ? [] : bars.flatMap( b => ( s.outline === 1
		? [ { x: b.x - o, y: b.y - o, w: b.w + ( 2 * o ), h: b.h + ( 2 * o ) } ]
		: [ { x: b.x - o, y: b.y - o, w: b.w + o, h: o }, { x: b.x - o, y: b.y, w: o, h: b.h } ] ) )
	const rect = b => `<rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}"/>`
	const ring = shape === 'circle' && t > 0
		? `<circle cx="${c}" cy="${c}" r="${Math.max( 1, p.gap + ( p.length / 2 ) )}" fill="none" stroke="${css( s.color, s.alpha )}" stroke-width="${t}"/>`
		: ''

	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">`
		+ `<g fill="${css( s.outlineColor, s.outlineAlpha )}">${edges.map( rect ).join( '' )}</g>`
		+ `<g fill="${css( s.color, s.alpha )}">${bars.map( rect ).join( '' )}</g>${ring}</svg>`

	return { svg, size, pixels: p, bars }

}

const dataUrl = svg => `data:image/svg+xml;charset=utf-8,${encodeURIComponent( svg )}`

module.exports = { DEFAULTS, STYLES, dataUrl, normalize, pixels, readCode, render, restingStyle, writeCode }

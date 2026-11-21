// Where a window goes so its middle sits on the display's middle: between the
// two centre pixels, where a game draws its own crosshair.
//
// Electron places and sizes windows in whole DIPs. At 100% and 200% that is
// whole pixels and the fit is exact; at 125% or 150% one DIP is 1.25 or 1.5
// pixels, so each axis also tries one and two DIPs smaller and keeps the size
// whose middle lands closest.

const fitAxis = ( start, length, size, scale ) => {

	const middle = ( Math.round( start * scale ) * 2 + Math.round( length * scale ) ) / 2
	let best = null

	for ( const candidate of [ size, size - 1, size - 2 ] ) {

		const span = Math.round( candidate * scale )
		const position = Math.round( ( middle - ( span / 2 ) ) / scale )
		const miss = Math.abs( ( position * scale ) + ( span / 2 ) - middle )

		if ( best === null || miss < best.miss - 1e-9 ) {

			best = { position, size: candidate, miss }

		}

	}

	return best

}

// `area` and `size` in DIPs; returns whole-DIP bounds and how far off they are, in pixels
const centred = ( { area, size, scale = 1 } ) => {

	const x = fitAxis( area.x, area.width, size.width, scale )
	const y = fitAxis( area.y, area.height, size.height, scale )

	return { x: x.position, y: y.position, width: x.size, height: y.size, miss: Math.max( x.miss, y.miss ) }

}

module.exports = { centred }

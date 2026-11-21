// The player's own image as a crosshair: the web view reads the file and
// draws it on a canvas at the exact physical size wanted, so Scorpio itself
// needs no image library beyond a small PNG decoder. SVG stays crisp because
// its vectors are drawn at that size, never scaled from a bitmap.
'use strict'

{

const TYPES = { png: 'image/png', jpg: 'image/jpeg', webp: 'image/webp', gif: 'image/gif', svg: 'image/svg+xml' }
const FILE_LIMIT = 5 * 1024 * 1024
const SIZE_LIMIT = 512
const SIZE_MIN = 4

const kindOf = name => {

	const ext = ( /\.([a-z0-9]+)$/i.exec( name )?.[1] || '' ).toLowerCase()

	if ( ext === 'jpeg' ) return 'jpg'

	return ext in TYPES ? ext : null

}

const unreadable = () => new Error( 'That image could not be read. Choose a PNG, JPG, WebP, GIF or SVG file.' )

const svgRoot = text => {

	const root = new DOMParser().parseFromString( text, 'image/svg+xml' ).documentElement

	if ( root.nodeName !== 'svg' ) throw unreadable()

	return root

}

// The SVG's proportions, from its width and height or its viewBox, and
// whether it states a size of its own.
const svgShape = text => {

	const root = svgRoot( text )
	const w = parseFloat( root.getAttribute( 'width' ) )
	const ht = parseFloat( root.getAttribute( 'height' ) )
	const relative = /%/.test( `${root.getAttribute( 'width' )}${root.getAttribute( 'height' )}` )

	if ( w > 0 && ht > 0 && !relative ) return { width: w, height: ht, intrinsic: true }
	const box = ( root.getAttribute( 'viewBox' ) || '' ).trim().split( /[\s,]+/ ).map( Number )

	if ( box.length === 4 && box[2] > 0 && box[3] > 0 ) return { width: box[2], height: box[3], intrinsic: false }

	return { width: 1, height: 1, intrinsic: false }

}

// The SVG with the exact pixel size written into it, so every web view
// draws its vectors at that size. It is parsed as data and shown through an
// image, where no script in it runs.
const svgAt = ( text, width, height ) => {

	const doc = new DOMParser().parseFromString( text, 'image/svg+xml' )
	const root = doc.documentElement

	if ( root.nodeName !== 'svg' ) throw unreadable()
	if ( !root.getAttribute( 'viewBox' ) ) {

		const shape = svgShape( text )

		root.setAttribute( 'viewBox', `0 0 ${shape.width} ${shape.height}` )

	}
	root.setAttribute( 'width', String( width ) )
	root.setAttribute( 'height', String( height ) )
	root.setAttribute( 'preserveAspectRatio', 'xMidYMid meet' )

	return new XMLSerializer().serializeToString( doc )

}

const load = async blob => {

	const url = URL.createObjectURL( blob )
	const img = new Image()

	img.src = url
	try {

		await img.decode()

	} catch {

		throw unreadable()

	} finally {

		URL.revokeObjectURL( url )

	}

	return img

}

// The size to start at: a raster image at 1:1, an SVG at its own size, or
// 64 px when it states none.
const naturalSize = async ( bytes, kind ) => {

	let size = 64

	if ( kind === 'svg' ) {

		const shape = svgShape( new TextDecoder().decode( bytes ) )

		if ( shape.intrinsic ) size = Math.round( Math.max( shape.width, shape.height ) )

	} else {

		const img = await load( new Blob( [ bytes ], { type: TYPES[kind] } ) )

		size = Math.max( img.naturalWidth, img.naturalHeight )

	}

	return Math.min( SIZE_LIMIT, Math.max( SIZE_MIN, size ) )

}

// PNG bytes of the file drawn `size` physical pixels on its longer side. A
// GIF gives its first frame. A raster image enlarged by a whole number keeps
// its hard pixels; anything else is smoothed.
const rasterise = async ( bytes, kind, size ) => {

	let img
	let width
	let height
	let smooth = true

	if ( kind === 'svg' ) {

		const text = new TextDecoder().decode( bytes )
		const shape = svgShape( text )
		const k = size / Math.max( shape.width, shape.height )

		width = Math.max( 1, Math.round( shape.width * k ) )
		height = Math.max( 1, Math.round( shape.height * k ) )
		img = await load( new Blob( [ svgAt( text, width, height ) ], { type: TYPES.svg } ) )

	} else {

		img = await load( new Blob( [ bytes ], { type: TYPES[kind] } ) )
		const k = size / Math.max( img.naturalWidth, img.naturalHeight )

		width = Math.max( 1, Math.round( img.naturalWidth * k ) )
		height = Math.max( 1, Math.round( img.naturalHeight * k ) )
		smooth = !( Number.isInteger( k ) && k >= 1 )

	}

	const canvas = document.createElement( 'canvas' )

	canvas.width = width
	canvas.height = height
	const context = canvas.getContext( '2d' )

	context.imageSmoothingEnabled = smooth
	context.imageSmoothingQuality = 'high'
	context.drawImage( img, 0, 0, width, height )
	const blob = await new Promise( done => canvas.toBlob( done, 'image/png' ) )

	if ( !blob ) throw new Error( 'The image could not be drawn.' )

	return new Uint8Array( await blob.arrayBuffer() )

}

window.picture = { TYPES, FILE_LIMIT, SIZE_LIMIT, SIZE_MIN, kindOf, naturalSize, rasterise }

}

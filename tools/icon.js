// Draws Scorpio's 1024 px source icon: a static cross on basalt with a
// porphyry centre. `npx tauri icon tools/icon.png` makes every size from it.
// Run: node tools/icon.js
const fs = require( 'fs' )
const path = require( 'path' )
const zlib = require( 'zlib' )

const SIZE = 1024
const BASALT = [ 11, 10, 9 ]
const MARBLE = [ 235, 229, 218 ]
const PORPHYRY = [ 200, 67, 90 ]
const pixels = Buffer.alloc( SIZE * SIZE * 4 )

const fill = ( x, y, w, h, [ r, g, b ] ) => {

	for ( let row = y; row < y + h; row++ ) {

		for ( let col = x; col < x + w; col++ ) {

			const i = ( ( row * SIZE ) + col ) * 4

			pixels[i] = r
			pixels[i + 1] = g
			pixels[i + 2] = b
			pixels[i + 3] = 255

		}

	}

}

const mid = SIZE / 2
const bar = 64
const gap = 112
const arm = 272

fill( 0, 0, SIZE, SIZE, BASALT )
fill( mid + gap, mid - ( bar / 2 ), arm, bar, MARBLE )
fill( mid - gap - arm, mid - ( bar / 2 ), arm, bar, MARBLE )
fill( mid - ( bar / 2 ), mid + gap, bar, arm, MARBLE )
fill( mid - ( bar / 2 ), mid - gap - arm, bar, arm, MARBLE )
fill( mid - ( bar / 2 ), mid - ( bar / 2 ), bar, bar, PORPHYRY )

const crcTable = Array.from( { length: 256 }, ( _, n ) => {

	let c = n

	for ( let k = 0; k < 8; k++ ) {

		c = c & 1 ? 0xedb88320 ^ ( c >>> 1 ) : c >>> 1

	}

	return c >>> 0

} )

const crc = buffer => {

	let c = 0xffffffff

	for ( const byte of buffer ) {

		c = crcTable[( c ^ byte ) & 0xff] ^ ( c >>> 8 )

	}

	return ( c ^ 0xffffffff ) >>> 0

}

const chunk = ( type, data ) => {

	const length = Buffer.alloc( 4 )
	const sum = Buffer.alloc( 4 )
	const body = Buffer.concat( [ Buffer.from( type ), data ] )

	length.writeUInt32BE( data.length )
	sum.writeUInt32BE( crc( body ) )

	return Buffer.concat( [ length, body, sum ] )

}

const header = Buffer.alloc( 13 )

header.writeUInt32BE( SIZE, 0 )
header.writeUInt32BE( SIZE, 4 )
header[8] = 8
header[9] = 6

const raw = Buffer.alloc( SIZE * ( ( SIZE * 4 ) + 1 ) )

for ( let row = 0; row < SIZE; row++ ) {

	pixels.copy( raw, ( row * ( ( SIZE * 4 ) + 1 ) ) + 1, row * SIZE * 4, ( row + 1 ) * SIZE * 4 )

}

const png = Buffer.concat( [
	Buffer.from( [ 0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a ] ),
	chunk( 'IHDR', header ),
	chunk( 'IDAT', zlib.deflateSync( raw, { level: 9 } ) ),
	chunk( 'IEND', Buffer.alloc( 0 ) ),
] )

fs.writeFileSync( path.join( __dirname, 'icon.png' ), png )
console.log( 'tools/icon.png', png.length, 'bytes' )

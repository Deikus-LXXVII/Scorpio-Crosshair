const assert = require( 'node:assert/strict' )
const { test } = require( 'node:test' )
const { normalize, pixels, readCode, render, restingStyle, writeCode } = require( '../../src/main/crosshair' )
const { vectors } = require( './fixtures/cs2-sharecodes.json' )

// The fixture uses akiver/csgo-sharecode's field names; these are Scorpio's
const view = ( { format, settings } ) => ( {
	format,
	style: settings.style,
	dot: settings.dot,
	tShape: settings.tShape,
	color: settings.color,
	alpha: settings.alpha,
	gap: settings.gap,
	length: settings.length,
	thickness: settings.thickness,
	madeFor: settings.madeFor,
	outline: settings.outline,
} )

const expectedView = e => ( {
	format: e.format,
	style: restingStyle( e.style ),
	dot: e.centerDotEnabled,
	tShape: e.tStyleEnabled,
	color: [ e.red, e.green, e.blue ],
	alpha: e.alpha,
	gap: e.gap,
	length: e.length,
	thickness: e.thickness,
	madeFor: e.screenHeight,
	outline: e.outlineMode ?? Number( e.outlineEnabled ),
} )

test( 'every pixel-era reference code reads to its published values', () => {

	const pixelEra = vectors.filter( vector => vector.expected.format !== 'legacy-v1' )

	assert.equal( pixelEra.length, 12 )

	for ( const { code, expected } of pixelEra ) {

		assert.deepEqual( view( readCode( code ) ), expectedView( expected ), code )

	}

} )

test( 'a CS2 code comes back character for character', () => {

	for ( const { code } of vectors.filter( vector => vector.expected.format === 'cs2-v1' && [ 3, 4, 6, 8, 9 ].includes( vector.expected.style ) ) ) {

		assert.equal( writeCode( readCode( code ).settings ), code )

	}

	const dynamic = vectors.find( vector => vector.expected.format === 'cs2-v1' && vector.expected.style === 2 )
	const { codeStyle, settings } = readCode( dynamic.code )

	assert.deepEqual( [ codeStyle, settings.style ], [ 2, 4 ], 'a dynamic style comes in at rest' )

} )

test( 'the split, recoil and scope values survive a round trip', () => {

	const { expected } = vectors.find( vector => vector.code.startsWith( 'CSAB6' ) )
	const { settings } = readCode( writeCode( readCode( vectors.find( vector => vector.code.startsWith( 'CSAB6' ) ).code ).settings ) )

	assert.deepEqual( settings.extra, {
		followRecoil: expected.followRecoil,
		spreadLimit: expected.dynamicSpreadLimit,
		splitDistance: expected.splitDistance,
		innerSplitAlpha: expected.innerSplitAlpha,
		outerSplitAlpha: expected.outerSplitAlpha,
		splitSizeRatio: expected.splitSizeRatio,
		scopeDotScale: expected.scopeDotScale,
		scopeDotUsesColor: expected.scopeDotUseCrosshairColor,
	} )

} )

test( 'an old CS:GO code is placed at the player\'s screen as the old renderer drew it', () => {

	const { code } = vectors.find( vector => vector.code === 'CSGO-WsnnD-eHaMw-QNDf9-oxuDh-ydOUD' )
	const at480 = readCode( code, { screenHeight: 480 } )

	assert.equal( at480.format, 'legacy-v1' )
	assert.deepEqual( [ at480.settings.length, at480.settings.thickness, at480.settings.gap, at480.settings.alpha, at480.settings.outline ], [ 10, 1, 1, 200, 1 ] )
	assert.equal( readCode( code, { screenHeight: 1080 } ).settings.length, 22, 'size 10 at 1080p is 22.5, which rounds to even' )

} )

test( 'anything else is refused', () => {

	for ( const bad of [ '', 'hello', 'CSGO-12345-12345-12345-12345-12345', vectors[0].code.replace( /.$/, 'A' ), `CS${'A'.repeat( 44 )}` ] ) {

		assert.throws( () => readCode( bad ), bad )

	}

} )

test( 'settings are kept in range', () => {

	const s = normalize( { length: 999, thickness: 1.3, gap: -9999, color: [ 300, -4, 'x' ], outline: 7, outlineWidth: 0, madeFor: 10, style: 2 } )

	assert.deepEqual( [ s.length, s.thickness, s.gap, s.color, s.outline, s.outlineWidth, s.madeFor, s.style ], [ 255, 1.5, -3840, [ 255, 0, 50 ], 2, 1, 240, 4 ] )

} )

test( 'sizes scale with the screen; thickness keeps quarter pixels', () => {

	assert.deepEqual( pixels( { length: 8, thickness: 2, gap: 4, madeFor: 720 }, 1440 ), { length: 16, thickness: 4, gap: 8, outline: 2 } )
	assert.deepEqual( pixels( { length: 8, thickness: 1, gap: 4, madeFor: 1080 }, 1440 ), { length: 11, thickness: 1.25, gap: 5, outline: 1 } )

} )

test( 'a static cross: four bars beyond the gap, all starting on whole pixels', () => {

	const { bars, size } = render( { style: 4, length: 8, thickness: 2, gap: 4, dot: true, madeFor: 1080 }, { screenHeight: 1080 } )
	const c = size / 2

	assert.equal( size % 2, 0 )
	assert.deepEqual( bars[0], { x: c - 1 + 2 + 4, y: c - 1, w: 8, h: 2 } )
	assert.deepEqual( bars.at( -1 ), { x: c - 1, y: c - 1, w: 2, h: 2 }, 'the dot is as wide as the lines' )
	assert.equal( bars.length, 5 )

	const half = render( { style: 4, length: 8, thickness: 1.5, gap: 4, madeFor: 1080 }, { screenHeight: 1080 } )

	assert.ok( half.bars.every( bar => Number.isInteger( bar.x ) && Number.isInteger( bar.y ) ), 'a half step only softens the far edge' )
	assert.equal( half.bars[0].h, 1.5 )

} )

test( 'the T shape drops the top bar, not another one', () => {

	const { bars, size } = render( { style: 4, length: 6, thickness: 2, gap: 3, tShape: true, madeFor: 1080 }, { screenHeight: 1080 } )
	const c = size / 2

	assert.equal( bars.length, 3 )
	assert.ok( bars.every( bar => bar.y + bar.h > c - 1 ), 'nothing above the centre block' )

} )

test( 'outline width grows the full outline and the half outline\'s two edges', () => {

	const full = render( { style: 6, thickness: 4, outline: 1, outlineWidth: 3, madeFor: 1080 }, { screenHeight: 1080 } )
	const halfEdge = render( { style: 6, thickness: 4, outline: 2, outlineWidth: 2, madeFor: 1080 }, { screenHeight: 1080 } )

	assert.match( full.svg, /<rect x="(\d+)" y="\1" width="10" height="10"\/>/, 'a 4 px dot with 3 px all round' )
	assert.match( halfEdge.svg, /height="2"\/>/, 'a 2 px top edge' )
	assert.equal( writeCode( { outlineWidth: 5 } ), writeCode( {} ), 'not part of a share code' )

} )

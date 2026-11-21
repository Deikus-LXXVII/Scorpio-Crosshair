const assert = require( 'node:assert/strict' )
const { test } = require( 'node:test' )
const { battleNetGames, epicGame, gameAt, heroicGames, registryFolders, riotGames, steamGame, steamLibraries, xboxFolders } = require( '../../src/main/launchers' )

test( 'Steam: libraries, then games from app manifests, tools left out', () => {

	assert.deepEqual( steamLibraries( '"0" { "path"  "C:\\\\Program Files (x86)\\\\Steam" } "1" { "path" "/home/me/.local/share/Steam" }' ), [ 'C:\\Program Files (x86)\\Steam', '/home/me/.local/share/Steam' ] )

	const acf = ( name, dir ) => `"AppState" { "name" "${name}" "installdir" "${dir}" }`

	assert.deepEqual( steamGame( acf( 'THE FINALS', 'The Finals' ), 'C:\\Steam' ), { source: 'Steam', name: 'THE FINALS', key: 'c:/steam/steamapps/common/the finals/' } )
	assert.equal( steamGame( acf( 'Proton 9.0', 'Proton 9.0' ), '/s' ), null )
	assert.equal( steamGame( acf( 'Steam Linux Runtime - Sniper', 'SteamLinuxRuntime_sniper' ), '/s' ), null )
	assert.equal( steamGame( acf( 'Wallpaper Engine', 'wallpaper_engine' ), '/s' ), null )

} )

test( 'Epic counts only games; GOG and Ubisoft come from the registry', () => {

	assert.equal( epicGame( JSON.stringify( { DisplayName: 'Fortnite', InstallLocation: 'C:\\Epic\\Fortnite', AppCategories: [ 'games' ] } ) ).name, 'Fortnite' )
	assert.equal( epicGame( JSON.stringify( { DisplayName: 'Unreal Engine', InstallLocation: 'C:\\UE', AppCategories: [ 'engines' ] } ) ), null )
	assert.equal( epicGame( '{' ), null )
	assert.deepEqual( registryFolders( '    path    REG_SZ    C:\\GOG Games\\Witcher 3\r\n', 'path', 'GOG' ), [ { source: 'GOG', name: 'Witcher 3', key: 'c:/gog games/witcher 3/' } ] )

} )

test( 'Battle.net keeps install folders and drops their parents and its own', () => {

	const db = '\u0002C:/Program Files (x86)\u0003C:/Program Files (x86)/StarCraft II\u0004C:/ProgramData/Battle.net/Agent\u0005D:/Games\u0006D:/Games/Overwatch\u0007'
	const found = battleNetGames( db )

	assert.deepEqual( found.map( entry => entry.name ), [ 'StarCraft II', 'Overwatch' ] )
	assert.equal( gameAt( 'C:\\Program Files (x86)\\Google\\Chrome\\chrome.exe', found ), null )

} )

test( 'Xbox folders from .GamingRoot; Riot games from its install list', () => {

	assert.deepEqual( xboxFolders( Buffer.from( '5247425801000000580062006f007800470061006d00650073000000', 'hex' ) ), [ 'XboxGames' ] )
	assert.deepEqual( xboxFolders( Buffer.from( 'nope' ) ), [] )

	const key = 'associated_client'

	assert.deepEqual( riotGames( JSON.stringify( { [key]: { 'D:/Riot Games/VALORANT/live/': 'x' } } ) ).map( entry => entry.name ), [ 'VALORANT' ] )

} )

test( 'Heroic lists its installed games on Linux', () => {

	const legendary = JSON.stringify( { Fortnite: { title: 'Fortnite', install_path: '/home/me/Games/Heroic/Fortnite' } } )
	const gog = JSON.stringify( { installed: [ { appName: '1', install_path: '/home/me/Games/Heroic/Cyberpunk 2077', title: 'Cyberpunk 2077' } ] } )

	assert.deepEqual( heroicGames( legendary, 'Heroic' ), [ { source: 'Heroic', name: 'Fortnite', key: '/home/me/games/heroic/fortnite/' } ] )
	assert.equal( heroicGames( gog, 'Heroic' )[0].name, 'Cyberpunk 2077' )

} )

test( 'an executable belongs to the deepest folder that holds it, and never to a sibling', () => {

	const games = [ { source: 'Steam', name: 'Deadlock', key: 'c:/steam/steamapps/common/deadlock/' }, { source: 'X', name: 'Outer', key: 'd:/games/' }, { source: 'Y', name: 'Inner', key: 'd:/games/inner/' } ]

	assert.equal( gameAt( 'C:\\Steam\\steamapps\\common\\Deadlock\\bin\\deadlock.exe', games ).name, 'Deadlock' )
	assert.equal( gameAt( 'D:/Games/Inner/x.exe', games ).name, 'Inner' )
	assert.equal( gameAt( 'C:\\Steam\\steamapps\\common\\Deadlock-other\\x.exe', games ), null )
	assert.equal( gameAt( 'C:\\Steam\\steam.exe', games ), null )

} )

// Games installed through launchers, found in the launchers' own files. A window
// whose executable is inside one of these install folders is a game.
// Parsers are pure (test/unit/launchers.test.js); `scan` reads the disk.

const fs = require( 'fs' )
const os = require( 'os' )
const path = require( 'path' )
const { execFile } = require( 'child_process' )

// Steam installs tools and runtimes as apps too
const NOT_GAMES = new Set( [ 'steamworks shared', 'steamworks common redistributables', 'wallpaper engine', 'wallpaper_engine', 'lossless scaling', 'borderless gaming', 'steamvr', 'steam linux runtime', 'steam linux runtime - soldier', 'steam linux runtime - sniper', 'proton experimental', 'proton hotfix', 'proton easyanticheat runtime', 'proton battleye runtime' ] )
const tool = ( ...names ) => names.some( name => NOT_GAMES.has( String( name || '' ).trim().toLowerCase() ) || /^proton \d/i.test( String( name || '' ) ) )

// A folder as a comparable prefix: lower case, one separator style, trailing separator
const folderKey = folder => {

	const key = String( folder || '' ).trim().replaceAll( '\\', '/' ).replace( /\/+$/, '' ).toLowerCase()

	return key ? `${key}/` : ''

}

const game = ( source, name, folder ) => ( { source, name: String( name ).trim(), key: folderKey( folder ) } )
const lastPart = folder => String( folder ).replaceAll( '\\', '/' ).replace( /\/+$/, '' ).split( '/' ).pop()

/* Parsers */

const vdfStrings = ( text, key ) => [ ...String( text || '' ).matchAll( new RegExp( `"${key}"\\s+"((?:[^"\\\\]|\\\\.)*)"`, 'gi' ) ) ].map( match => match[1].replaceAll( '\\\\', '\\' ) )

const steamLibraries = vdf => vdfStrings( vdf, 'path' )

const steamGame = ( acf, library ) => {

	const [ name = '' ] = vdfStrings( acf, 'name' )
	const [ dir = '' ] = vdfStrings( acf, 'installdir' )

	return dir && !tool( name, dir ) ? game( 'Steam', name || dir, `${library}/steamapps/common/${dir}` ) : null

}

const epicGame = text => {

	try {

		const item = JSON.parse( text )

		return item?.InstallLocation && ( item.AppCategories || [] ).includes( 'games' ) && !tool( item.DisplayName )
			? game( 'Epic', item.DisplayName || lastPart( item.InstallLocation ), item.InstallLocation )
			: null

	} catch {

		return null

	}

}

// `reg query <key> /s /v <value>`: one folder per subkey
const registryFolders = ( text, value, source ) => [ ...String( text || '' ).matchAll( new RegExp( `^\\s+${value}\\s+REG_SZ\\s+(.+?)\\s*$`, 'gim' ) ) ]
	.map( match => match[1] )
	.filter( folder => !tool( lastPart( folder ) ) )
	.map( folder => game( source, lastPart( folder ), folder ) )

// Battle.net's product.db is protobuf with the install folders as plain text;
// it also names their parents and its own folders, which are not games
const battleNetGames = text => {

	const found = new Map()

	for ( const match of String( text || '' ).matchAll( /[a-z]:[\\/][ -~]{2,200}?(?=[^ -~]|$)/gi ) ) {

		const key = folderKey( match[0] )

		if ( !/battle\.net|programdata/.test( key ) && !/^[a-z]:\/((program files( \(x86\))?|games|users|windows)\/)?$/.test( key ) && !found.has( key ) ) {

			found.set( key, lastPart( match[0] ) )

		}

	}

	const keys = [ ...found.keys() ]

	return keys.filter( key => !keys.some( other => other !== key && other.startsWith( key ) ) ).map( key => ( { source: 'Battle.net', name: found.get( key ), key } ) )

}

// X:\.GamingRoot: "RGBX", a uint32 count, then UTF-16 folder names on that drive
const xboxFolders = buffer => {

	if ( !Buffer.isBuffer( buffer ) || buffer.length < 8 || buffer.toString( 'latin1', 0, 4 ) !== 'RGBX' ) {

		return []

	}

	return buffer.toString( 'utf16le', 8 ).split( '\0' ).map( name => name.trim() ).filter( Boolean ).slice( 0, buffer.readUInt32LE( 4 ) )

}

// RiotClientInstalls.json: each game's folder is a key of associated_client
const riotGames = text => {

	try {

		return Object.keys( JSON.parse( text )?.associated_client || {} ).map( folder => {

			const parts = folder.replaceAll( '\\', '/' ).split( '/' ).filter( Boolean )
			const name = /^(live|pbe)$/i.test( parts.at( -1 ) ) ? parts.at( -2 ) : parts.at( -1 )

			return game( 'Riot', name, folder )

		} )

	} catch {

		return []

	}

}

// Heroic (Linux): legendaryConfig/installed.json and gog_store/installed.json
const heroicGames = ( text, source ) => {

	try {

		const data = JSON.parse( text )
		const entries = Array.isArray( data?.installed ) ? data.installed : Object.values( data || {} )

		return entries
			.map( entry => ( { name: entry.title || entry.app_name || lastPart( entry.install_path || '' ), folder: entry.install_path } ) )
			.filter( entry => entry.folder && !tool( entry.name ) )
			.map( entry => game( source, entry.name, entry.folder ) )

	} catch {

		return []

	}

}

// The installed game an executable belongs to: the deepest folder that holds it
const gameAt = ( executable, games ) => {

	const file = String( executable || '' ).trim().replaceAll( '\\', '/' ).toLowerCase()
	let best = null

	if ( !file ) {

		return null

	}

	for ( const candidate of games || [] ) {

		if ( candidate.key && file.startsWith( candidate.key ) && ( !best || candidate.key.length > best.key.length ) ) {

			best = candidate

		}

	}

	return best

}

/* Reading the disk */

const read = file => fs.promises.readFile( file, 'utf8' ).catch( () => '' )
const readRaw = file => fs.promises.readFile( file ).catch( () => null )
const folders = dir => fs.promises.readdir( dir, { withFileTypes: true } ).then( entries => entries.filter( entry => entry.isDirectory() ).map( entry => entry.name ) ).catch( () => [] )
const files = dir => fs.promises.readdir( dir ).catch( () => [] )
const regQuery = args => new Promise( resolve => {

	execFile( 'reg.exe', [ 'query', ...args ], { windowsHide: true, timeout: 5000 }, ( _error, stdout ) => resolve( stdout || '' ) )

} )

const steamFrom = async roots => {

	const libraries = new Map()

	for ( const root of roots ) {

		for ( const library of [ root, ...steamLibraries( await read( path.join( root, 'steamapps', 'libraryfolders.vdf' ) ) ) ] ) {

			libraries.set( folderKey( library ), library )

		}

	}

	const found = await Promise.all( [ ...libraries.values() ].map( async library => {

		const steamapps = path.join( library, 'steamapps' )
		const manifests = ( await files( steamapps ) ).filter( name => /^appmanifest_\d+\.acf$/i.test( name ) )

		return Promise.all( manifests.map( async name => steamGame( await read( path.join( steamapps, name ) ), library ) ) )

	} ) )

	return found.flat().filter( Boolean )

}

const DRIVES = 'CDEFGH'.split( '' )

const subfolderGames = async ( source, parents, skip = [] ) => {

	const found = await Promise.all( parents.map( async parent => ( await folders( parent ) )
		.filter( name => !skip.includes( name.toLowerCase() ) && !tool( name ) )
		.map( name => game( source, name, path.join( parent, name ) ) ) ) )

	return found.flat()

}

const windowsScans = () => [
	( async () => {

		const registered = /SteamPath\s+REG_SZ\s+(.+?)\s*$/im.exec( await regQuery( [ 'HKCU\\Software\\Valve\\Steam', '/v', 'SteamPath' ] ) )?.[1]

		return steamFrom( [ registered ? path.win32.normalize( registered ) : 'C:\\Program Files (x86)\\Steam' ] )

	} )(),
	( async () => {

		const dir = 'C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests'
		const items = ( await files( dir ) ).filter( name => name.toLowerCase().endsWith( '.item' ) )

		return ( await Promise.all( items.map( async name => epicGame( await read( path.join( dir, name ) ) ) ) ) ).filter( Boolean )

	} )(),
	regQuery( [ 'HKLM\\SOFTWARE\\WOW6432Node\\GOG.com\\Games', '/s', '/v', 'path' ] ).then( text => registryFolders( text, 'path', 'GOG' ) ),
	regQuery( [ 'HKLM\\SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs', '/s', '/v', 'InstallDir' ] ).then( text => registryFolders( text, 'InstallDir', 'Ubisoft' ) ),
	( async () => {

		const parents = await Promise.all( DRIVES.map( async drive => xboxFolders( await readRaw( `${drive}:\\.GamingRoot` ) ).map( name => path.win32.join( `${drive}:\\`, name ) ) ) )

		return subfolderGames( 'Xbox', parents.flat(), [ 'gamesave' ] )

	} )(),
	( async () => [ ...riotGames( await read( 'C:\\ProgramData\\Riot Games\\RiotClientInstalls.json' ) ), ...await subfolderGames( 'Riot', DRIVES.map( drive => `${drive}:\\Riot Games` ), [ 'riot client' ] ) ] )(),
	subfolderGames( 'EA', [ 'C:\\Program Files\\EA Games', 'C:\\Program Files (x86)\\Origin Games', ...DRIVES.map( drive => `${drive}:\\EA Games` ) ] ),
	readRaw( 'C:\\ProgramData\\Battle.net\\Agent\\product.db' ).then( buffer => battleNetGames( buffer ? buffer.toString( 'latin1' ) : '' ) ),
]

const linuxScans = () => {

	const home = os.homedir()

	return [
		steamFrom( [ path.join( home, '.steam', 'steam' ), path.join( home, '.local', 'share', 'Steam' ), path.join( home, '.var', 'app', 'com.valvesoftware.Steam', '.local', 'share', 'Steam' ) ] ),
		read( path.join( home, '.config', 'heroic', 'legendaryConfig', 'legendary', 'installed.json' ) ).then( text => heroicGames( text, 'Heroic' ) ),
		read( path.join( home, '.config', 'heroic', 'gog_store', 'installed.json' ) ).then( text => heroicGames( text, 'Heroic' ) ),
		// Lutris keeps its games under ~/Games by default
		subfolderGames( 'Lutris', [ path.join( home, 'Games' ) ] ),
	]

}

const scan = async () => {

	const scans = process.platform === 'win32' ? windowsScans() : ( process.platform === 'linux' ? linuxScans() : [] )
	const results = await Promise.all( scans.map( pending => Promise.resolve( pending ).catch( () => [] ) ) )
	const unique = new Map()

	for ( const found of results.flat() ) {

		if ( found.key && !unique.has( found.key ) ) {

			unique.set( found.key, found )

		}

	}

	return [ ...unique.values() ]

}

module.exports = { battleNetGames, epicGame, folderKey, gameAt, heroicGames, registryFolders, riotGames, scan, steamGame, steamLibraries, xboxFolders }

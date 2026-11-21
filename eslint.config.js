const js = require( '@eslint/js' )
const globals = require( 'globals' )

module.exports = [
	js.configs.recommended,
	{
		files: [ 'src/main/**/*.js', 'test/**/*.js', 'eslint.config.js', 'src/renderer/preload*.js' ],
		languageOptions: { sourceType: 'commonjs', globals: { ...globals.node } },
	},
	{
		files: [ 'src/renderer/**/*.js' ],
		ignores: [ 'src/renderer/preload*.js' ],
		languageOptions: { sourceType: 'script', globals: { ...globals.browser } },
	},
	{
		rules: {
			'no-unused-vars': [ 'error', { argsIgnorePattern: '^_', caughtErrors: 'none' } ],
		},
	},
]

const standard = require('@neutrinojs/standardjs')
const react = require('@neutrinojs/react')
const jest = require('@neutrinojs/jest')

module.exports = {
  options: {
    root: __dirname,
    mains: {
      dash: {
        entry: 'dash',
        title: 'denViews dashboard'
      },
      init: {
        entry: 'init',
        title: 'denViews init'
      }
    }
  },
  use: [
    standard(),
    react({
      publicPath: '/_denViews_dash',
      html: {
        title: 'dashboard'
      }
    }),
    jest()
  ]
}

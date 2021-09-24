import { hot } from 'react-hot-loader'
import React from 'react'
import PropTypes from 'prop-types'
import FolderDisplayer from './Folders'
import ViewDisplayer from './Pages'
import './App.css'

class FolderChooser extends React.Component {
  constructor (props) {
    super(props)
    this.state = { folderID: this.props.folderID }

    this.handleChangeFolder = this.handleChangeFolder.bind(this)
  }

  handleChangeFolder (event) {
    this.setState({ folderID: event.target.value })
  }

  render () {
    return (
      <div>
        {this.state.folderID === '' || this.state.folderID === null
          ? <p>You need to input a folder ID. (Hint: 0 is the root folder)</p>
          : <FolderDisplayer folderID={this.state.folderID} />}
        <form>
          <label>
            Folder ID:
            <input type='number' onChange={this.handleChangeFolder} value={this.state.folderID} />
          </label>
        </form>
      </div>
    )
  }
}

FolderChooser.propTypes = {
  folderID: PropTypes.number
}

function App () {
  const uri = new URL(document.URL)
  const folderID = uri.searchParams.get('folder_id') === null ? 0 : uri.searchParams.get('folder_id')
  const pageName = uri.searchParams.get('page_name')
  return (
    <div className='App'>
      {
        pageName === null
          ? <FolderChooser folderID={folderID} />
          : <ViewDisplayer folderID={folderID} pageName={pageName} />
      }
    </div>
  )
}

export default hot(module)(App)

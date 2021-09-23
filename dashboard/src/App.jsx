import { hot } from 'react-hot-loader'
import React from 'react'
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
            <input type='text' onChange={this.handleChangeFolder} value={this.state.folderID} />
          </label>
        </form>
      </div>
    )
  }
}

function App () {
  const uri = new URL(document.URL)
  return (
    <div className='App'>
      <FolderChooser folderID={uri.searchParams.get('folder_id')} />
    </div>
  )
}

export default hot(module)(App)

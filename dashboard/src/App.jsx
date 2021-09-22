import { hot } from 'react-hot-loader'
import React from 'react'
import './App.css'

async function fetchViews () {
  const res = await fetch('https://127.0.0.1/')
  return res.json()
}

function futureWrapper (Component, future) {
  return class extends React.Component {
    constructor (props) {
      super(props)
      this.future = future
      this.state = {
        res: null
      }
    }

    async awaitFuture () {
      const res = await this.future()
      this.setState({
        res: res
      })
    }

    componentDidMount () {
      this.awaitFuture()
    }

    render () {
      console.log('rendering future now!')
      return (
        <Component data={this.state.res} {...this.props} />
      )
    }
  }
}

function ViewConsumer (props) {
  console.log(props.data)
  return props.data === undefined || props.data === null
    ? (<p>Loading...</p>)
    : (
      <ul>
        <li>Page: {props.data.page}</li>
        <li>Views: {props.data.views}</li>
        <li>Hits: {props.data.hits}</li>
      </ul>
      )
}

function ViewDisplayer () {
  const Component = futureWrapper(
    ViewConsumer,
    fetchViews
  )

  return <Component />
}

function FolderConsumer (props) {
  return props.data === undefined || props.data === null
    ? (<p>Loading...</p>)
    : (
      <div>
        <h1>[#{props.data.id}] Folder {props.data.name}</h1>
        <p>Folders:</p>
        <ul>
          {
          props.data.folders.map((e) => <li key={e.name + e.id}>{e.name}</li>)
        }
        </ul>
        <p>Pages:</p>
        <ul>
          {
          props.data.pages.map((e) => <li key={e.name}>{e.page} | Views: {e.views} | Hits: {e.hits}</li>)
        }
        </ul>
      </div>
      )
}

function FolderDisplayer (props) {
  const Component = futureWrapper(
    FolderConsumer,
    async () => {
      const resp = await fetch('https://127.0.0.1/_denViews_dash/api/folder?folder_id=0')
      return resp.json()
    }
  )

  return <Component />
}

// const message = 'Testing'
function App () {
  return (
    <div className='App'>
      <ViewDisplayer />
      <FolderDisplayer />
    </div>
  )
}

export default hot(module)(App)

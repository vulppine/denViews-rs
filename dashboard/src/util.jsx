import React from 'react'
import 'tailwindcss/tailwind.css'

const DenViewsHeader = () =>
  <div className='border-b-2 mb-4'>
    <h1 className='text-xl'>denViews</h1>
  </div>

function fetchFromApi (route, searchParams) {
  return async () => {
    const currentUrl = new URL(document.URL)
    const url = new URL([currentUrl.protocol, currentUrl.hostname].join('//') + '/_denViews_dash/api/' + route)
    if (searchParams !== null) {
      url.search = '?' + searchParams.toString()
    }

    const res = await fetch(url.toString())
    return res.json()
  }
}

const Loading = () => <p>Loading...</p>

function futureWrapper (Component, Loader, future) {
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
      return this.state.res === null
        ? <Loader />
        : <Component data={this.state.res} {...this.props} />
    }
  }
}

export { DenViewsHeader, fetchFromApi, futureWrapper, Loading }

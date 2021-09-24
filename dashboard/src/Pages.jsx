import React from 'react'
import PropTypes from 'prop-types'
import * as utils from './util'

const ViewConsumer = (props) => {
  return (
    <div>
      <h1>[#{props.data.id}] {props.data.page}</h1>
      <p>In folder: <a href={'?folder_id=' + props.data.folder_id}>{props.data.folder_id}</a></p>
      <ul>
        <li>Views: {props.data.views}</li>
        <li>Hits: {props.data.hits}</li>
      </ul>
    </div>
  )
}

ViewConsumer.propTypes = {
  data: {
    id: PropTypes.number,
    folder_id: PropTypes.number,
    page: PropTypes.string,
    views: PropTypes.number,
    hits: PropTypes.number
  }
}

function ViewDisplayer (props) {
  const searchParams = new URLSearchParams()
  searchParams.append('folder_id', props.folderID)
  searchParams.append('name', props.pageName)

  const Component = utils.futureWrapper(
    ViewConsumer,
    utils.Loading,
    utils.fetchFromApi('page', searchParams)
  )

  return <Component />
}

ViewDisplayer.propTypes = {
  folderID: PropTypes.number,
  pageName: PropTypes.string
}

export default ViewDisplayer

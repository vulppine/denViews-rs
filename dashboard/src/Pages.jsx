import React from 'react'
import PropTypes from 'prop-types'
import * as utils from './util'

const ViewConsumer = (props) => {
  return (
    <ul>
      <li>Page: {props.data.page}</li>
      <li>Views: {props.data.views}</li>
      <li>Hits: {props.data.hits}</li>
    </ul>
  )
}

ViewConsumer.propTypes = {
  data: {
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

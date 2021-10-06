import React from 'react'
import PropTypes from 'prop-types'
import * as utils from './util'
import 'tailwindcss/tailwind.css'

const ViewConsumer = (props) => {
  return (
    <div>
      <h1 className='text-3xl font-bold pb-4'>[#{props.data.id}] {props.data.page}</h1>
      <p>In folder: <a href={'?folder_id=' + props.data.folder_id} className='text-blue-500'>{props.data.folder_id}</a></p>
      <table>
        <thead>
          <th className='border px-2'>Views</th>
          <th className='border px-2'>Hits</th>
        </thead>
        <tbody>
          <tr>
            <td className='border px-2'>{props.data.views}</td>
            <td className='border px-2'>{props.data.hits}</td>
          </tr>
        </tbody>
      </table>
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

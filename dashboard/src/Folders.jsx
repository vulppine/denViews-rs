import React from 'react'
import PropTypes from 'prop-types'
import * as utils from './util'
import 'tailwindcss/tailwind.css'

const FolderConsumer = (props) => {
  return (
    <div>
      <h1 className='text-3xl font-bold pb-4'>[#{props.data.id}] Folder {props.data.name}</h1>
      {
        props.data.parent_id === null
          ? <p>No parent folder.</p>
          : <p>Parent folder: <FolderLink folderID={props.data.parent_id} /></p>
      }
      <PartialFolderTable folders={props.data.folders} />
      <PageInfoList pages={props.data.pages} folderID={props.data.id} />
    </div>
  )
}

const PartialFolderTable = (props) =>
  <table className='border mb-4'>
    <thead>
      <tr><th className='border px-2'>Folders</th></tr>
      <tr>
        <th className='border px-2'>ID</th>
        <th className='border px-2'>Name</th>
      </tr>
    </thead>
    <tbody>
      {props.folders.map((e) =>
        <PartialFolderInfo key={e.name + e.id} id={e.id} name={e.name} />)}
    </tbody>
  </table>

PartialFolderTable.propTypes = { folders: PropTypes.array }

const PartialFolderInfo = (props) =>
  <tr>
    <td className='border px-2'><FolderLink folderID={props.id} /></td>
    <td className='border px-2'>{props.name}</td>
  </tr>

PartialFolderInfo.propTypes = {
  id: PropTypes.number,
  name: PropTypes.string
}

const PageInfoList = (props) =>
  <table className='border mb-4'>
    <thead>
      <tr><th className='border px-2'>Pages</th></tr>
      <tr>
        <th className='border px-2'>Name</th>
        <th className='border px-2'>Views</th>
        <th className='border px-2'>Hits</th>
      </tr>
    </thead>
    <tbody>
      {props.pages.map((e) =>
        <PageInfo key={e.name} page={e.page} views={e.views} hits={e.hits} folderID={props.folderID} />)}
    </tbody>
  </table>

PageInfoList.propTypes = { pages: PropTypes.array, folderID: PropTypes.number }

const PageInfo = (props) =>
  <tr>
    <td className='border px-2'><PageLink pageName={props.page} folderID={props.folderID} /></td>
    <td className='border px-2'>{props.views}</td>
    <td className='border px-2'>{props.hits}</td>
  </tr>

PageInfo.propTypes = {
  folderID: PropTypes.number,
  page: PropTypes.string,
  views: PropTypes.number,
  hits: PropTypes.number
}

const PageLink = (props) => {
  return props.pageName === '###self###'
    ? <span>self</span>
    : <a href={'?folder_id=' + props.folderID + '&page_name=' + props.pageName} className='text-blue-500'>{props.pageName}</a>
}

PageLink.propTypes = {
  folderID: PropTypes.number,
  pageName: PropTypes.string
}

const FolderLink = (props) =>
  <a href={'?folder_id=' + props.folderID} className='text-blue-500'>{props.folderID}</a>

FolderLink.propTypes = {
  folderID: PropTypes.number
}

FolderConsumer.propTypes = {
  data: {
    id: PropTypes.number,
    name: PropTypes.string,
    folders: PropTypes.array,
    pages: PropTypes.array
  }
}

function FolderDisplayer (props) {
  const searchParams = new URLSearchParams()
  searchParams.append('folder_id', props.folderID)
  const Component = utils.futureWrapper(
    FolderConsumer,
    utils.Loading,
    utils.fetchFromApi('folder', searchParams)
  )

  return <Component />
}

FolderDisplayer.propTypes = {
  folderID: PropTypes.number
}

export default FolderDisplayer

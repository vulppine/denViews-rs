import React from 'react'
import PropTypes from 'prop-types'
import * as utils from './util'

const FolderConsumer = (props) => {
  return (
    <div>
      <h1>[#{props.data.id}] Folder {props.data.name}</h1>
      {
        props.data.parent_id === null
          ? <p>No parent folder.</p>
          : <p>Parent folder: <FolderLink folderID={props.data.parent_id} /></p>
      }
      <p>Folders:</p>
      <PartialFolderList folders={props.data.folders} />
      <p>Pages:</p>
      <PageInfoList pages={props.data.pages} folderID={props.data.id} />
    </div>
  )
}

const PartialFolderList = (props) =>
  <ul>
    {props.folders.map((e) =>
      <li key={e.name + e.id}><PartialFolderInfo id={e.id} name={e.name} /></li>)}
  </ul>

PartialFolderList.propTypes = { folders: PropTypes.array }

const PartialFolderInfo = (props) =>
  <span><FolderLink folderID={props.id} /> | {props.name}</span>

PartialFolderInfo.propTypes = {
  id: PropTypes.number,
  name: PropTypes.string
}

const PageInfoList = (props) =>
  <ul>
    {props.pages.map((e) =>
      <li key={e.name}><PageInfo page={e.page} views={e.views} hits={e.hits} folderID={props.folderID} /></li>)}
  </ul>

PageInfoList.propTypes = { pages: PropTypes.array, folderID: PropTypes.number }

const PageInfo = (props) =>
  <span><PageLink pageName={props.page} folderID={props.folderID} /> | Views: {props.views} | Hits: {props.hits}</span>

PageInfo.propTypes = {
  folderID: PropTypes.number,
  page: PropTypes.string,
  views: PropTypes.number,
  hits: PropTypes.number
}

const PageLink = (props) => {
  return props.pageName === '###self###'
    ? <span>self</span>
    : <a href={'?folder_id=' + props.folderID + '&page_name=' + props.pageName}>{props.pageName}</a>
}

PageLink.propTypes = {
  folderID: PropTypes.number,
  pageName: PropTypes.string
}

const FolderLink = (props) =>
  <a href={'?folder_id=' + props.folderID}>{props.folderID}</a>

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

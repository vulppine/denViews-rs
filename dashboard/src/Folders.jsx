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
      <PageInfoList pages={props.data.pages} />
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

const PageInfoList = (props) =>
  <ul>
    {props.pages.map((e) =>
      <li key={e.name}><PageInfo page={e.page} views={e.views} hits={e.hits} /></li>)}
  </ul>

PageInfoList.propTypes = { pages: PropTypes.array }

const PageInfo = (props) =>
  <span>{props.page} | Views: {props.views} | Hits: {props.hits}</span>

const FolderLink = (props) =>
  <a href={'?folder_id=' + props.folderID}>{props.folderID}</a>

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

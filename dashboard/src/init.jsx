import React from 'react'
import { render } from 'react-dom'
import * as utils from './util'
import JSONForm from './Forms'

const InitForm = (props) => {
  const Form = utils.futureWrapper(
    JSONForm,
    utils.Loading,
    utils.fetchFromApi('settings', null)
  )

  return <Form method='POST' action='/_denViews_dash/api/init' />
}

const Init = () => <InitForm />

render(<Init />, document.getElementById('root'))

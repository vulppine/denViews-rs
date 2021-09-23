import React from 'react'
import PropTypes from 'prop-types'

function JSONForm (props) {
  const components = Object.entries(props.data).map((e) => {
    return (
      <div key={e[0]}>
        {elementToFormComponent(e)}
      </div>
    )
  })

  components.push(<input key='formSubmit' type='submit' />)

  return (
    <form method={props.method} action={props.method === null ? '_self' : props.action}>
      {components}
    </form>
  )
}

JSONForm.propTypes = {
  data: PropTypes.object,
  method: PropTypes.string,
  action: PropTypes.string
}

function elementToFormComponent (element) {
  switch (typeof element[1]) {
    case 'string':
      return <StringInput name={element[0]} input={element[1]} />
    case 'number':
      return <NumberInput name={element[0]} input={element[1]} />
    case 'boolean':
      return <BoolInput name={element[0]} input={element[1]} />
    default:
      return null
  }
}

const StringInput = (props) =>
  <label>{props.name}: <input type='text' name={props.name} defaultValue={props.input} /></label>

StringInput.propTypes = {
  name: PropTypes.string,
  input: PropTypes.string
}

const NumberInput = (props) =>
  <label>{props.name}: <input type='number' name={props.name} defaultValue={props.input} /></label>

NumberInput.propTypes = {
  name: PropTypes.string,
  input: PropTypes.number
}

const BoolInput = (props) => {
  return (
    <label>{props.name}:
      <BoolRadio name={props.name} value='true' currentValue={props.input} />
      <BoolRadio name={props.name} value='false' currentValue={props.input} />
    </label>
  )
}

BoolInput.propTypes = {
  name: PropTypes.string,
  input: PropTypes.bool
}

const BoolRadio = (props) => {
  switch (props.value) {
    case 'true':
      if (props.currentValue === true) {
        return (
          <>
            <input type='radio' name={props.name} id={props.name + 'true'} value='true' defaultChecked />
            <label htmlFor={props.name + 'true'}>true</label>
          </>
        )
      } else {
        return (
          <>
            <input type='radio' name={props.name} id={props.name + 'true'} value='true' />
            <label htmlFor={props.name + 'true'}>true</label>
          </>
        )
      }
    case 'false':
      if (props.currentValue === false) {
        return (
          <>
            <input type='radio' name={props.name} id={props.name + 'false'} value='false' defaultChecked />
            <label htmlFor={props.name + 'false'}>false</label>
          </>
        )
      } else {
        return (
          <>
            <input type='radio' name={props.name} id={props.name + 'false'} value='false' />
            <label htmlFor={props.name + 'false'}>false</label>
          </>
        )
      }
  }
}

BoolRadio.propTypes = {
  name: PropTypes.string,
  value: PropTypes.string,
  currentValue: PropTypes.bool
}

export default JSONForm

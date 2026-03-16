# CDP Real Input

## MODIFIED Requirements

### Requirement: Click dispatches real CDP mouse events

The click action SHALL use chromiumoxide's native Element API to dispatch real CDP `Input.dispatchMouseEvent` events instead of JavaScript `el.click()`. This produces the full mouse event sequence (mouseMoved, mousePressed, mouseReleased) that frameworks like React and Vue require.

#### Scenario: Click triggers full mouse event sequence
Given a page with a button element
When click is called on the button
Then the browser dispatches mouseMoved, mousePressed, and mouseReleased CDP events
And the element receives mousedown, mouseup, and click DOM events

### Requirement: Type dispatches real CDP keyboard events

The type action SHALL use chromiumoxide's native Element API to dispatch real CDP `Input.dispatchKeyEvent` events instead of JavaScript value assignment. The element MUST be focused via a real click before typing, and each character MUST produce individual keyDown/keyUp events.

#### Scenario: Type triggers keystroke events per character
Given a page with an input element
When type_text is called with text "hello"
Then the browser dispatches keyDown, keyUp CDP events for each character
And the input element receives keydown, keypress, keyup DOM events
And the input value reflects the typed text

#### Scenario: Clear before type selects all and deletes
Given a page with an input element containing existing text
When type_text is called with clear=true
Then existing text is selected and deleted before new text is typed

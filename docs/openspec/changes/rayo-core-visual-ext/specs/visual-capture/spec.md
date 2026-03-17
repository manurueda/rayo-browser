# Visual Capture Extensions

## ADDED Requirements

### Requirement: Viewport dimensions are configurable

RayoBrowser SHALL accept optional width and height parameters at construction time. The navigate "goto" action SHALL accept optional viewport dimensions. Default remains 1280x720.

#### Scenario: Custom viewport on browser launch
Given a RayoBrowser configured with width=1920 and height=1080
When a page is loaded
Then Chrome's viewport is 1920x1080
And screenshots capture at that resolution

#### Scenario: Default viewport when no dimensions specified
Given a RayoBrowser with no viewport configuration
When a page is loaded
Then the viewport is 1280x720

### Requirement: Page maps include element bounding boxes

The page_map JavaScript extraction SHALL call getBoundingClientRect() for each interactive element and include x, y, width, height in the InteractiveElement struct.

#### Scenario: Page map elements have bounding boxes
Given a page with a button at position (100, 200) with size 150x40
When page_map is requested
Then the button element includes x=100, y=200, width=150, height=40

### Requirement: Element-level screenshots clip to element bounding box

A new element screenshot mode SHALL use the element's bounding box as CDP clip parameter to capture only that element.

#### Scenario: Screenshot of a specific element
Given a page with a header element
When an element screenshot is requested for that header
Then the returned image contains only the header element
And the image dimensions match the element's bounding box

### Requirement: Animation freeze disables CSS animations and transitions

Before screenshot capture for visual testing, rayo-core SHALL inject CSS that sets all animation-duration and transition-duration to 0s. The injection SHALL be removed after capture.

#### Scenario: Animated element is frozen for screenshot
Given a page with a CSS animation (e.g., spinning loader)
When animation freeze is applied and a screenshot is captured
Then the screenshot shows the element in a static state
And the injected CSS is removed after capture

### Requirement: Screenshots support PNG format

capture_screenshot SHALL accept a format parameter supporting "jpeg" and "png". Visual testing uses PNG (lossless). Default remains JPEG for backward compatibility.

#### Scenario: PNG screenshot for visual testing
Given a loaded page
When a screenshot is captured with format="png"
Then the returned bytes are valid PNG data
And there are no compression artifacts

# rayo-core visual extensions: Tasks

## Tasks

- [ ] Add viewport width/height params to RayoBrowser::new() and launch config
- [ ] Add viewport params to rayo_navigate "goto" action in rayo-mcp
- [ ] Extend page_map JavaScript to extract getBoundingClientRect() per element
- [ ] Add x, y, width, height fields to InteractiveElement struct
- [ ] Implement element-level screenshot using CDP clip parameter
- [ ] Add element screenshot mode to rayo_observe (selector + id targeting)
- [ ] Implement animation freeze/unfreeze via CSS injection
- [ ] Add format param to capture_screenshot (png/jpeg)
- [ ] Add screenshot dimension cap with configurable max
- [ ] Integration tests: viewport resize, bounding boxes, element screenshots, animation freeze

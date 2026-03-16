# Screenshot Performance

## MODIFIED Requirements

### Requirement: Screenshot capture uses JPEG format with quality control

The screenshot implementation SHALL use JPEG format at 80% quality by default instead of PNG. When `full_page` is false, the capture MUST clip to the current viewport dimensions, avoiding unnecessary full-document rendering.

#### Scenario: Default screenshot returns JPEG-encoded image
Given a page is loaded
When screenshot is captured with default settings
Then the returned bytes begin with JPEG magic bytes (0xFF 0xD8)
And the capture completes in under 100ms

#### Scenario: Viewport-only screenshot clips to visible area
Given a page is loaded with content extending below the fold
When screenshot is captured with full_page=false
Then only the viewport area is captured
And the image dimensions match the viewport dimensions

#### Scenario: Full-page screenshot captures entire document
Given a page is loaded with content extending below the fold
When screenshot is captured with full_page=true
Then the entire document height is captured

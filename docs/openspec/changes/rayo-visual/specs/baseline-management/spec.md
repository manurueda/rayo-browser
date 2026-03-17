# Baseline Management

## ADDED Requirements

### Requirement: Baselines are stored as PNG files with metadata

The BaselineManager SHALL save PNG images to `{baselines_dir}/{name}.png` and metadata to `{name}.meta.json`. Metadata includes dimensions, perceptual hash, and creation timestamp.

#### Scenario: Save and load a baseline round-trip
Given a BaselineManager with a writable baselines directory
When save("login-page", png_bytes) is called
Then a file is created at {baselines_dir}/login-page.png
And a metadata file is created at {baselines_dir}/login-page.meta.json
And load("login-page") returns the same PNG bytes

#### Scenario: List baselines returns all saved items
Given a baselines directory with 3 saved baselines
When list() is called
Then it returns 3 BaselineInfo items with names, dimensions, and timestamps

#### Scenario: Delete removes baseline and metadata
Given a baseline named "old-test" exists
When delete("old-test") is called
Then the PNG file and metadata file are both removed
And exists("old-test") returns false

### Requirement: Baseline names are sanitized to prevent path traversal

The BaselineManager SHALL reject baseline names containing path separators, `..`, or characters outside `[a-zA-Z0-9_-]`.

#### Scenario: Path traversal attempt is rejected
Given a BaselineManager
When save("../../etc/passwd", bytes) is called
Then a VisualError::InvalidName is returned
And no file is written

#### Scenario: Valid name with hyphens and underscores is accepted
Given a BaselineManager
When save("login-page_v2", bytes) is called
Then the baseline is saved successfully

### Requirement: Missing baseline returns BaselineNotFound error

Loading a non-existent baseline SHALL return a VisualError::BaselineNotFound error with the requested name.

#### Scenario: Load non-existent baseline returns error
Given a BaselineManager with no baseline named "nonexistent"
When load("nonexistent") is called
Then a VisualError::BaselineNotFound is returned
And the error message includes the name "nonexistent"

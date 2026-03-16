# Profile Format

## MODIFIED Requirements

### Requirement: rayo_profile respects the format parameter

The handle_profile handler SHALL parse the `format` parameter from tool arguments and dispatch to the corresponding Profiler export method. When no format is specified, it MUST default to `ai_summary`.

#### Scenario: Default format returns ai_summary
Given no format parameter is provided
When rayo_profile is called
Then the response contains the AI summary format starting with "RAYO PROFILE"

#### Scenario: JSON format returns structured data
Given format is "json"
When rayo_profile is called
Then the response contains valid JSON array of profile spans

#### Scenario: Markdown format returns table
Given format is "markdown"
When rayo_profile is called
Then the response contains a markdown table with category statistics

#### Scenario: Chrome trace format returns trace events
Given format is "chrome_trace"
When rayo_profile is called
Then the response contains valid JSON with a traceEvents array

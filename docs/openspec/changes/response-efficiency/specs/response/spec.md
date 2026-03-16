# Response Efficiency

## MODIFIED Requirements

### Requirement: MCP responses use compact JSON serialization

All MCP tool responses SHALL use compact JSON serialization (`to_string()`) instead of pretty-printed format. This eliminates 20-40% unnecessary whitespace from every response without changing the data structure.

#### Scenario: Tool responses contain no unnecessary whitespace
Given any MCP tool is called
When the response is serialized
Then the JSON output uses compact format (no indentation or extra newlines)

### Requirement: Navigation goto returns page_map data without redundant CDP calls

The navigation goto handler SHALL extract title and URL from the page_map result instead of making separate CDP evaluate calls. This eliminates 2 redundant CDP round-trips per navigation.

#### Scenario: Goto response includes title and URL from page_map
Given a navigation goto is executed
When the response is assembled
Then title and URL are extracted from the page_map result
And no separate CDP evaluate calls are made for title or URL

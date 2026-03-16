# Batch Pipelining

## MODIFIED Requirements

### Requirement: Batch defers cache invalidation to end of batch

The batch executor SHALL defer all cache invalidation until the entire batch has completed. Individual actions within a batch MUST use internal no-invalidate variants, with a single invalidation pass performed at the end of the batch.

#### Scenario: Multiple clicks in batch do single cache invalidation
Given a batch of 3 click actions
When the batch executes
Then selector cache is invalidated once at the end
And page_map cache is invalidated once at the end
Not 3 times during execution

### Requirement: Batch results are unchanged

Deferred cache invalidation SHALL NOT change the results of batch execution. A batch with deferred invalidation MUST produce identical results to the same actions executed sequentially with per-action invalidation.

#### Scenario: Batch produces same results regardless of invalidation strategy
Given a batch of mixed actions (click, type, screenshot)
When the batch executes with deferred invalidation
Then the results are identical to sequential execution

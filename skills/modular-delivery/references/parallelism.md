# Parallelism

Use parallel execution for:

- independent file reads
- isolated audits
- disjoint spec work
- disjoint code work
- validation across isolated modules

Avoid parallel execution for:

- overlapping edits in one file
- shared index or manifest updates
- archive or finalize steps
- dependency chains where one task's output changes another task's input

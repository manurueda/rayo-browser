-- Architect analysis database
-- Populated by collect.sh, queried by surveyor/auditor subagents

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- Logical regions of the codebase
CREATE TABLE IF NOT EXISTS regions (
  name        TEXT PRIMARY KEY,
  description TEXT,
  root_paths  TEXT  -- JSON array of directory prefixes
);

-- Every source file
CREATE TABLE IF NOT EXISTS files (
  path            TEXT PRIMARY KEY,
  lines           INTEGER NOT NULL,
  exports         INTEGER NOT NULL DEFAULT 0,
  internal_fns    INTEGER NOT NULL DEFAULT 0,
  classification  TEXT,  -- logic, data, types, component, hook, store, route
  region          TEXT REFERENCES regions(name),
  layer           TEXT,  -- server, client, shared, types, route
  prop_count      INTEGER,  -- max props on any component in this file (null = not a component)
  child_component_count INTEGER,  -- number of distinct child components rendered (null = not a component)
  has_local_derivation  BOOLEAN   -- true if file has useMemo/build* derivation alongside JSX composition
);

-- Function-level metrics (from TS compiler API)
CREATE TABLE IF NOT EXISTS functions (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  file_path       TEXT NOT NULL REFERENCES files(path),
  name            TEXT NOT NULL,
  exported        BOOLEAN NOT NULL DEFAULT 0,
  line_start      INTEGER,
  line_count      INTEGER,
  param_count     INTEGER,
  branching_depth INTEGER,  -- max nesting of if/switch/for/while/ternary
  is_async        BOOLEAN NOT NULL DEFAULT 0,
  prop_count      INTEGER DEFAULT -1  -- React component prop count (-1 = not a component)
);

-- Import graph (from madge)
CREATE TABLE IF NOT EXISTS imports (
  source_path TEXT NOT NULL REFERENCES files(path),
  target_path TEXT NOT NULL REFERENCES files(path),
  type_only   BOOLEAN NOT NULL DEFAULT 0,
  PRIMARY KEY (source_path, target_path)
);

-- Export consumer counts (from knip + grep)
CREATE TABLE IF NOT EXISTS exports (
  file_path   TEXT NOT NULL REFERENCES files(path),
  name        TEXT NOT NULL,
  consumers   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (file_path, name)
);

-- Git churn (last 3 months)
CREATE TABLE IF NOT EXISTS churn (
  file_path     TEXT PRIMARY KEY REFERENCES files(path),
  changes_3m    INTEGER NOT NULL DEFAULT 0,
  last_changed  TEXT  -- ISO date
);

-- Temporal coupling (files that change together)
CREATE TABLE IF NOT EXISTS temporal_coupling (
  file_a      TEXT NOT NULL,
  file_b      TEXT NOT NULL,
  co_changes  INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (file_a, file_b)
);

-- Pre-computed module-level coupling
CREATE TABLE IF NOT EXISTS module_coupling (
  module_a    TEXT NOT NULL,  -- directory path
  module_b    TEXT NOT NULL,  -- directory path
  imports_ab  INTEGER NOT NULL DEFAULT 0,  -- A imports from B
  imports_ba  INTEGER NOT NULL DEFAULT 0,  -- B imports from A
  type_only_ab INTEGER NOT NULL DEFAULT 0,
  type_only_ba INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (module_a, module_b)
);

-- Architectural violations (from dependency-cruiser)
CREATE TABLE IF NOT EXISTS violations (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  rule_name   TEXT NOT NULL,
  severity    TEXT NOT NULL,  -- error, warn, info
  source_path TEXT,
  target_path TEXT,
  message     TEXT
);

-- Metadata
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT
);

-- Useful views

CREATE VIEW IF NOT EXISTS v_file_metrics AS
SELECT
  f.path,
  f.lines,
  f.exports,
  f.internal_fns,
  f.classification,
  f.region,
  f.layer,
  COALESCE(c.changes_3m, 0) AS churn,
  (SELECT COUNT(*) FROM imports WHERE target_path = f.path) AS fan_in,
  (SELECT COUNT(*) FROM imports WHERE source_path = f.path) AS fan_out,
  CASE WHEN f.exports > 0 AND (f.exports + f.internal_fns) > 0
    THEN ROUND(CAST(f.exports AS REAL) / (f.exports + f.internal_fns), 2)
    ELSE 0 END AS exposure_ratio
FROM files f
LEFT JOIN churn c ON c.file_path = f.path;

CREATE VIEW IF NOT EXISTS v_god_modules AS
SELECT path, fan_in, lines, classification, region
FROM v_file_metrics
WHERE fan_in > 10
ORDER BY fan_in DESC;

CREATE VIEW IF NOT EXISTS v_orphan_files AS
SELECT path, lines, classification, region
FROM v_file_metrics
WHERE fan_in = 0 AND classification NOT IN ('route', 'data')
ORDER BY lines DESC;

CREATE VIEW IF NOT EXISTS v_dead_exports AS
SELECT e.file_path, e.name, f.region
FROM exports e
JOIN files f ON f.path = e.file_path
WHERE e.consumers = 0
ORDER BY f.region, e.file_path;

CREATE VIEW IF NOT EXISTS v_risk_hotspots AS
SELECT
  f.path,
  f.lines,
  f.region,
  c.changes_3m AS churn,
  MAX(fn.branching_depth) AS max_complexity,
  COUNT(CASE WHEN fn.branching_depth >= 3 THEN 1 END) AS complex_fn_count
FROM files f
JOIN churn c ON c.file_path = f.path
JOIN functions fn ON fn.file_path = f.path
WHERE c.changes_3m >= 5
GROUP BY f.path
HAVING max_complexity >= 3
ORDER BY c.changes_3m * max_complexity DESC;

CREATE VIEW IF NOT EXISTS v_region_summary AS
SELECT
  f.region,
  COUNT(*) AS files,
  SUM(f.lines) AS total_lines,
  ROUND(AVG(f.lines), 0) AS avg_lines,
  MAX(f.lines) AS max_lines,
  SUM(f.exports) AS total_exports,
  ROUND(AVG(COALESCE(c.changes_3m, 0)), 1) AS avg_churn
FROM files f
LEFT JOIN churn c ON c.file_path = f.path
GROUP BY f.region
ORDER BY total_lines DESC;

-- Components with too many props or shell+derivation mixing
CREATE VIEW IF NOT EXISTS v_god_views AS
SELECT
  path,
  lines,
  region,
  prop_count,
  child_component_count,
  has_local_derivation,
  CASE
    WHEN prop_count >= 15 AND child_component_count >= 3 AND has_local_derivation THEN 'god-view: high props + multi-child + derivation'
    WHEN prop_count >= 15 AND child_component_count >= 3 THEN 'shell-bloat: high props + multi-child'
    WHEN prop_count >= 15 THEN 'prop-heavy: too many props'
    WHEN child_component_count >= 3 AND has_local_derivation THEN 'mixed-shell: composing + deriving'
    ELSE 'warning'
  END AS smell
FROM files
WHERE classification = 'component'
  AND (prop_count >= 15 OR (child_component_count >= 3 AND has_local_derivation))
ORDER BY prop_count DESC;

CREATE VIEW IF NOT EXISTS v_violations_by_rule AS
SELECT rule_name, severity, COUNT(*) AS count
FROM violations
GROUP BY rule_name, severity
ORDER BY count DESC;

CREATE VIEW IF NOT EXISTS v_module_instability AS
SELECT
  m.module_a AS module,
  SUM(m.imports_ba) AS afferent,   -- others depend on me
  SUM(m.imports_ab) AS efferent,   -- I depend on others
  CASE WHEN SUM(m.imports_ab) + SUM(m.imports_ba) > 0
    THEN ROUND(CAST(SUM(m.imports_ab) AS REAL) / (SUM(m.imports_ab) + SUM(m.imports_ba)), 2)
    ELSE 0 END AS instability
FROM module_coupling m
GROUP BY m.module_a
ORDER BY instability DESC;

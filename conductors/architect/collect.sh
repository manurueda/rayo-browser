#!/bin/bash
# Architect data collection pipeline
# Runs real static analysis tools and populates SQLite database.
#
# Usage:
#   .architect/collect.sh              — full collection (all steps)
#   .architect/collect.sh --region X   — collect only region X
#   .architect/collect.sh --step N     — run only step N (1-6)

set -euo pipefail

PROJECT_DIR="$(git rev-parse --show-toplevel)"
ARCHITECT_DIR="$PROJECT_DIR/.architect"
DB="$ARCHITECT_DIR/architect.db"

cd "$PROJECT_DIR"

# Parse args
REGION=""
STEP=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --region) REGION="$2"; shift 2 ;;
    --step)   STEP="$2"; shift 2 ;;
    *)        echo "Unknown arg: $1"; exit 1 ;;
  esac
done

should_run() {
  [ -z "$STEP" ] || [ "$STEP" = "$1" ]
}

echo "=== Architect Data Collection ==="
echo "Database: $DB"
[ -n "$REGION" ] && echo "Region filter: $REGION"
[ -n "$STEP" ] && echo "Step filter: $STEP"
echo ""

# --- Step 0: Initialize database ---
if should_run 0 || [ ! -f "$DB" ]; then
  echo "[0/6] Initializing database..."
  rm -f "$DB"
  sqlite3 "$DB" < "$ARCHITECT_DIR/schema.sql"

  # Define regions
  sqlite3 "$DB" <<'SQL'
INSERT OR REPLACE INTO regions (name, description, root_paths) VALUES
  ('simulator',  'Company simulator core domain',    '["lib/server/companySimulator","lib/hooks/companySimulator","components/companySimulator","types/companySimulator"]'),
  ('auth',       'Authentication and authorization',  '["lib/server/auth","components/auth","types/auth"]'),
  ('billing',    'Stripe, pricing, subscriptions',    '["lib/server/stripe","lib/server/pricing","components/pricing","types/pricing"]'),
  ('landing',    'Marketing and landing pages',       '["components/landing","components/pricing"]'),
  ('terminal',   'Company terminal feature',          '["lib/server/companyTerminal","lib/hooks/companyTerminal","components/companyTerminal","types/companyTerminal"]'),
  ('intelligence','Company intelligence feature',     '["lib/server/companyIntelligence","components/companyIntelligence","types/companyIntelligence"]'),
  ('shared',     'Shared utilities and constants',    '["lib/utils","lib/constants","lib/stores","types"]'),
  ('ui',         'UI components and primitives',      '["components/ui","components/common"]'),
  ('app',        'Routes, pages, API handlers',       '["app"]');

INSERT OR REPLACE INTO meta (key, value) VALUES
  ('collected_at', datetime('now')),
  ('project_dir', '$PROJECT_DIR');
SQL
  echo "  Database created with schema and regions."
fi

# --- Step 1: File metrics + function inventory (TS compiler API) ---
if should_run 1; then
  echo "[1/6] Extracting file and function metrics (TypeScript compiler API)..."

  node "$ARCHITECT_DIR/collect-metrics.mjs" "$PROJECT_DIR/tsconfig.json" > /tmp/architect_metrics.ndjson

  # Batch insert with Python (avoids per-line sqlite3 calls + fixes region detection)
  python3 -c "
import json, sqlite3

conn = sqlite3.connect('$DB')

# Load region mappings
regions = {}
for name, paths_json in conn.execute('SELECT name, root_paths FROM regions').fetchall():
    for prefix in json.loads(paths_json):
        regions[prefix] = name

def get_region(path):
    best = ('shared', 0)
    for prefix, name in regions.items():
        if path.startswith(prefix) and len(prefix) > best[1]:
            best = (name, len(prefix))
    return best[0]

def get_layer(path):
    if path.startswith('lib/server/'):    return 'server'
    if path.startswith('lib/hooks/'):     return 'client'
    if path.startswith('lib/stores/'):    return 'client'
    if path.startswith('lib/utils/'):     return 'shared'
    if path.startswith('lib/constants/'): return 'shared'
    if path.startswith('components/'):    return 'client'
    if path.startswith('types/'):         return 'types'
    if path.startswith('app/api/'):       return 'route'
    if path.startswith('app/'):           return 'route'
    return 'shared'

region_filter = '$REGION' or None
file_count = 0
fn_count = 0

with open('/tmp/architect_metrics.ndjson') as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        d = json.loads(line)

        if d['type'] == 'file':
            region = get_region(d['path'])
            layer = get_layer(d['path'])
            if region_filter and region != region_filter:
                continue
            conn.execute('''INSERT OR REPLACE INTO files
              (path, lines, exports, internal_fns, classification, region, layer, prop_count, child_component_count, has_local_derivation)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)''',
              (d['path'], d['lines'], d['exports'], d['internal_fns'],
               d['classification'], region, layer,
               d.get('prop_count'), d.get('child_component_count'), d.get('has_local_derivation')))
            file_count += 1

        elif d['type'] == 'function':
            conn.execute('''INSERT INTO functions
              (file_path, name, exported, line_start, line_count, param_count, branching_depth, is_async, prop_count)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)''',
              (d['file_path'], d['name'], d['exported'], d['line_start'],
               d['line_count'], d['param_count'], d['branching_depth'], d['is_async'],
               d.get('prop_count', -1)))
            fn_count += 1

conn.commit()
conn.close()
print(f'  Indexed {file_count} files, {fn_count} functions.')
"
  rm -f /tmp/architect_metrics.ndjson

fi

# --- Step 2: Dependency graph + violations (dependency-cruiser) ---
if should_run 2; then
  echo "[2/7] Building dependency graph (dependency-cruiser)..."

  REPORT="$ARCHITECT_DIR/depcruiser-report.json"
  npm run arch:report 2>/dev/null

  if [ ! -f "$REPORT" ]; then
    echo "  ERROR: dependency-cruiser failed to produce report."
    exit 1
  fi

  # Parse dependency-cruiser JSON → imports table + violations table
  python3 -c "
import json, sqlite3, os

conn = sqlite3.connect('$DB')
project_files = set(r[0] for r in conn.execute('SELECT path FROM files').fetchall())

with open('$REPORT') as f:
    data = json.load(f)

# Populate imports table from dependency-cruiser modules
import_count = 0
type_count = 0
for module in data.get('modules', []):
    source = module.get('source', '')
    if source not in project_files:
        continue
    for dep in module.get('dependencies', []):
        target = dep.get('resolved', '')
        if not target or target not in project_files or target == source:
            continue
        # dependency-cruiser marks type-only deps via dependencyTypes
        dep_types = dep.get('dependencyTypes', [])
        is_type = 1 if 'type-only' in dep_types else 0
        conn.execute('INSERT OR IGNORE INTO imports (source_path, target_path, type_only) VALUES (?, ?, ?)',
                    (source, target, is_type))
        import_count += 1
        if is_type:
            type_count += 1

# Populate violations table from summary
violation_count = 0
for v in data.get('summary', {}).get('violations', []):
    rule = v.get('rule', {})
    conn.execute('INSERT INTO violations (rule_name, severity, source_path, target_path, message) VALUES (?, ?, ?, ?, ?)',
                (rule.get('name', ''), rule.get('severity', ''), v.get('from', ''), v.get('to', ''), v.get('comment', '')))
    violation_count += 1

conn.commit()
conn.close()
print(f'  {import_count} import edges ({type_count} type-only), {violation_count} violations.')
"
fi

# --- Step 3: Dead exports (knip) ---
if should_run 3; then
  echo "[3/6] Detecting dead exports (knip)..."

  # knip JSON format: { "issues": [ { "file": "...", "exports": [...], "types": [...], "files": [...] } ] }
  npx knip --reporter json 2>/dev/null | python3 -c "
import sys, json, sqlite3

data = json.load(sys.stdin)
conn = sqlite3.connect('$DB')

dead_count = 0

for issue in data.get('issues', []):
    file_path = issue.get('file', '')
    if not file_path:
        continue

    # Unused exports
    for exp in issue.get('exports', []):
        name = exp.get('name', 'default')
        conn.execute('INSERT OR REPLACE INTO exports (file_path, name, consumers) VALUES (?, ?, 0)',
                    (file_path, name))
        dead_count += 1

    # Unused types
    for typ in issue.get('types', []):
        name = typ.get('name', 'default')
        conn.execute('INSERT OR REPLACE INTO exports (file_path, name, consumers) VALUES (?, ?, 0)',
                    (file_path, name))
        dead_count += 1

    # Entire unused files
    for f in issue.get('files', []):
        conn.execute('INSERT OR REPLACE INTO exports (file_path, name, consumers) VALUES (?, ?, 0)',
                    (file_path, '<entire-file>'))
        dead_count += 1

    # Duplicates
    for dup in issue.get('duplicates', []):
        if isinstance(dup, list):
            for d in dup:
                name = d.get('name', '')
                if name:
                    conn.execute('INSERT OR REPLACE INTO exports (file_path, name, consumers) VALUES (?, ?, -1)',
                                (file_path, f'duplicate:{name}'))
                    dead_count += 1

conn.commit()
conn.close()
print(f'  {dead_count} dead exports/types/duplicates found.')
" || echo "  knip failed or no dead exports found."
fi

# --- Step 4: Git churn + temporal coupling ---
if should_run 4; then
  echo "[4/6] Analyzing git history (3 months)..."

  # File churn — batch with Python to avoid SIGPIPE
  git log --since="3 months ago" --format=format: --name-only -- '*.ts' '*.tsx' 2>/dev/null \
    | grep -v '^$' | sort | uniq -c | sort -rn | head -100 > /tmp/architect_churn.txt || true

  python3 -c "
import sqlite3, subprocess

conn = sqlite3.connect('$DB')
project_files = set(r[0] for r in conn.execute('SELECT path FROM files').fetchall())

count = 0
with open('/tmp/architect_churn.txt') as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        changes, filepath = int(parts[0]), parts[1]
        if filepath in project_files:
            try:
                result = subprocess.run(['git', 'log', '-1', '--format=%aI', '--', filepath],
                                       capture_output=True, text=True, timeout=5)
                last = result.stdout.strip()
            except:
                last = ''
            conn.execute('INSERT OR REPLACE INTO churn (file_path, changes_3m, last_changed) VALUES (?, ?, ?)',
                        (filepath, changes, last))
            count += 1

conn.commit()
conn.close()
print(f'  Churn data for {count} files.')
"
  rm -f /tmp/architect_churn.txt

  # Temporal coupling — find files that change together
  echo "  Computing temporal coupling..."
  git log --since="3 months ago" --format="COMMIT" --name-only -- '*.ts' '*.tsx' > /tmp/architect_gitlog.txt 2>/dev/null || true
  python3 -c "
import sys, sqlite3
from collections import defaultdict

conn = sqlite3.connect('$DB')
project_files = set(r[0] for r in conn.execute('SELECT path FROM files').fetchall())

# Parse git log output into commits
commits = []
current = []
for line in open('/tmp/architect_gitlog.txt'):
    line = line.strip()
    if line == 'COMMIT':
        if current:
            commits.append(current)
        current = []
    elif line and line in project_files:
        current.append(line)
if current:
    commits.append(current)

# Count co-changes
pairs = defaultdict(int)
for files in commits:
    files = sorted(set(files))
    for i in range(len(files)):
        for j in range(i + 1, len(files)):
            # Only count cross-directory pairs
            dir_a = '/'.join(files[i].split('/')[:2])
            dir_b = '/'.join(files[j].split('/')[:2])
            if dir_a != dir_b:
                pairs[(files[i], files[j])] += 1

# Insert pairs with 3+ co-changes
count = 0
for (a, b), n in pairs.items():
    if n >= 3:
        conn.execute('INSERT OR REPLACE INTO temporal_coupling (file_a, file_b, co_changes) VALUES (?, ?, ?)', (a, b, n))
        count += 1

conn.commit()
conn.close()
print(f'  {count} temporal coupling pairs (3+ co-changes).')
"
  rm -f /tmp/architect_gitlog.txt
fi

# --- Step 5: Module-level coupling (from dependency-cruiser metrics) ---
if should_run 5; then
  echo "[5/7] Computing module-level coupling (dependency-cruiser metrics)..."

  METRICS="$ARCHITECT_DIR/depcruiser-metrics.json"
  npm run arch:metrics 2>/dev/null

  if [ -f "$METRICS" ]; then
    python3 -c "
import json, sqlite3
from collections import defaultdict

conn = sqlite3.connect('$DB')

# Also compute from imports table for region/layer aggregation (backwards compat)
imports = conn.execute('''
  SELECT
    fs.region || '/' || fs.layer AS src_module,
    ft.region || '/' || ft.layer AS tgt_module,
    i.type_only
  FROM imports i
  JOIN files fs ON fs.path = i.source_path
  JOIN files ft ON ft.path = i.target_path
  WHERE src_module != tgt_module
''').fetchall()

coupling = defaultdict(lambda: {'ab': 0, 'type_ab': 0})
for src_mod, tgt_mod, type_only in imports:
    key = (src_mod, tgt_mod)
    coupling[key]['ab'] += 1
    if type_only:
        coupling[key]['type_ab'] += 1

for (mod_a, mod_b), counts in coupling.items():
    conn.execute('''INSERT OR REPLACE INTO module_coupling
      (module_a, module_b, imports_ab, imports_ba, type_only_ab, type_only_ba)
      VALUES (?, ?, ?, 0, ?, 0)''',
      (mod_a, mod_b, counts['ab'], counts['type_ab']))

for (mod_a, mod_b), counts in coupling.items():
    conn.execute('''UPDATE module_coupling
      SET imports_ba = ?, type_only_ba = ?
      WHERE module_a = ? AND module_b = ?''',
      (counts['ab'], counts['type_ab'], mod_b, mod_a))

conn.commit()
count = len(coupling)
conn.close()
print(f'  {count} cross-module coupling pairs.')
"
  else
    echo "  WARNING: dependency-cruiser metrics not available, skipping."
  fi
fi

# --- Step 6: Violations summary ---
if should_run 6 || [ -z "$STEP" ]; then
  echo "[6/7] Violations summary..."
  sqlite3 "$DB" <<'SQL'
.mode column
.headers on

SELECT '=== Architectural Violations ===' AS '';
SELECT rule_name, severity, COUNT(*) AS count FROM violations
GROUP BY rule_name, severity ORDER BY count DESC;
SQL
fi

# --- Step 7: Summary ---
if should_run 7 || [ -z "$STEP" ]; then
  echo "[7/7] Collection complete."
  echo ""
  echo "=== Database Summary ==="
  sqlite3 "$DB" <<'SQL'
.mode column
.headers on

SELECT 'Files' AS metric, COUNT(*) AS count FROM files
UNION ALL SELECT 'Functions', COUNT(*) FROM functions
UNION ALL SELECT 'Import edges', COUNT(*) FROM imports
UNION ALL SELECT 'Violations', COUNT(*) FROM violations
UNION ALL SELECT 'Dead exports', COUNT(*) FROM exports WHERE consumers = 0
UNION ALL SELECT 'Churn entries', COUNT(*) FROM churn
UNION ALL SELECT 'Temporal pairs', COUNT(*) FROM temporal_coupling
UNION ALL SELECT 'Module couplings', COUNT(*) FROM module_coupling;

SELECT '';
SELECT '=== Region Summary ===' AS '';
SELECT region, COUNT(*) AS files, SUM(lines) AS lines, ROUND(AVG(lines),0) AS avg
FROM files GROUP BY region ORDER BY lines DESC;
SQL
  echo ""
  echo "Database: $DB"
  echo "Query with: sqlite3 $DB \"SELECT * FROM v_risk_hotspots LIMIT 10;\""
  echo "Violations: sqlite3 $DB \"SELECT * FROM v_violations_by_rule;\""
fi

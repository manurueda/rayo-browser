#!/usr/bin/env node
/**
 * Extract function-level metrics from TypeScript source files using the TS compiler API.
 * Outputs NDJSON (one JSON object per function) to stdout.
 *
 * Usage: node collect-metrics.mjs tsconfig.json
 */

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const ts = require('typescript');
const path = require('path');

const tsconfigPath = process.argv[2] || 'tsconfig.json';
const configFile = ts.readConfigFile(tsconfigPath, ts.sys.readFile);
const parsedConfig = ts.parseJsonConfigFileContent(configFile.config, ts.sys, path.dirname(tsconfigPath));

const program = ts.createProgram(parsedConfig.fileNames, parsedConfig.options);
const checker = program.getTypeChecker();

const projectRoot = process.cwd();

function isProjectFile(filePath) {
  const rel = path.relative(projectRoot, filePath);
  return !rel.startsWith('node_modules') && !rel.startsWith('.') && (rel.startsWith('lib/') || rel.startsWith('components/') || rel.startsWith('types/') || rel.startsWith('app/'));
}

function getMaxBranchingDepth(node, depth = 0) {
  let maxDepth = depth;
  const branchingKinds = [
    ts.SyntaxKind.IfStatement,
    ts.SyntaxKind.SwitchStatement,
    ts.SyntaxKind.ForStatement,
    ts.SyntaxKind.ForInStatement,
    ts.SyntaxKind.ForOfStatement,
    ts.SyntaxKind.WhileStatement,
    ts.SyntaxKind.DoStatement,
    ts.SyntaxKind.ConditionalExpression,
  ];

  ts.forEachChild(node, child => {
    const newDepth = branchingKinds.includes(child.kind) ? depth + 1 : depth;
    const childMax = getMaxBranchingDepth(child, newDepth);
    if (childMax > maxDepth) maxDepth = childMax;
  });

  return maxDepth;
}

function isExported(node) {
  if (node.modifiers) {
    return node.modifiers.some(m => m.kind === ts.SyntaxKind.ExportKeyword);
  }
  return false;
}

function getParamCount(node) {
  if (node.parameters) return node.parameters.length;
  return 0;
}

function isAsync(node) {
  if (node.modifiers) {
    return node.modifiers.some(m => m.kind === ts.SyntaxKind.AsyncKeyword);
  }
  return false;
}

function getLineCount(node, sourceFile) {
  const start = sourceFile.getLineAndCharacterOfPosition(node.getStart());
  const end = sourceFile.getLineAndCharacterOfPosition(node.getEnd());
  return end.line - start.line + 1;
}

/**
 * Count the number of properties on a React component's props parameter.
 * Resolves the type of the first parameter and counts its members.
 * Returns -1 if not a component or props type cannot be resolved.
 */
function getPropCount(node, sourceFile) {
  // Must have exactly one parameter (props)
  const params = node.parameters || (node.declarationList?.declarations?.[0]?.initializer?.parameters);
  if (!params || params.length !== 1) return -1;

  const param = params[0];

  // Try to get the type from the checker
  try {
    const paramType = checker.getTypeAtLocation(param);
    if (!paramType) return -1;

    // Get declared properties from the parameter's type symbol, not apparent properties.
    // Apparent properties include inherited HTML element props (280+ for div),
    // which inflates the count. We want only the user-declared props interface members.
    const symbol = paramType.getSymbol() || paramType.aliasSymbol;
    if (symbol && symbol.declarations && symbol.declarations.length > 0) {
      const decl = symbol.declarations[0];
      // For interfaces/type literals, count own members
      if (ts.isInterfaceDeclaration(decl) || ts.isTypeLiteralNode(decl)) {
        // Count own members + inherited from extends (but not HTML element types)
        let count = 0;
        if (decl.members) count += decl.members.length;
        // If it extends other interfaces, count those too (but skip HTML/React types)
        if (decl.heritageClauses) {
          for (const clause of decl.heritageClauses) {
            for (const typeExpr of clause.types) {
              const baseType = checker.getTypeAtLocation(typeExpr);
              const baseSym = baseType.getSymbol();
              const baseName = baseSym?.name || '';
              // Skip HTML element props, React intrinsics
              if (/^(HTML|SVG|React\.|Aria|DOM)/.test(baseName)) continue;
              const baseProps = baseType.getProperties();
              if (baseProps) count += baseProps.length;
            }
          }
        }
        return count;
      }
    }

    // Fallback: for intersection types and mapped types, count apparent properties
    // but filter out HTML/React/DOM props
    const props = paramType.getApparentProperties();
    if (!props || props.length === 0) return -1;

    // If the count is suspiciously high (>100), it's likely inheriting from HTML element types
    if (props.length > 100) return -1;

    const REACT_INTERNALS = new Set(['key', 'ref', 'children']);
    const userProps = props.filter(p => !REACT_INTERNALS.has(p.name));

    return userProps.length;
  } catch {
    return -1;
  }
}

/**
 * Detect if a file is a "shell component" — a component that:
 * 1. Renders multiple child components (composing subfeatures)
 * 2. Also contains local data derivation (useMemo, computed values)
 *
 * Returns: { child_component_count, has_local_derivation }
 */
function detectShellPattern(sourceFile) {
  const text = sourceFile.getFullText();

  // Count distinct component imports (PascalCase default/named imports from project files)
  const componentImports = new Set();
  const importRe = /import\s+(?:(?:type\s+)?{[^}]*}|(\w+))\s+from\s+['"]@?\//g;
  // Simpler: count PascalCase JSX elements used in the file
  const jsxRe = /<([A-Z][A-Za-z0-9]+)/g;
  let match;
  while ((match = jsxRe.exec(text)) !== null) {
    componentImports.add(match[1]);
  }

  // Detect local derivation: useMemo, inline computations assigned to variables
  const hasLocalDerivation = /useMemo\s*\(/.test(text) ||
    /const\s+\w+\s*=\s*(?!use[A-Z])\w+\.\w+\s*\+/.test(text) || // e.g., activeRoleIds.length + 1
    /const\s+\w+\s*=\s*build[A-Z]/.test(text); // buildSomething() calls

  return {
    child_component_count: componentImports.size,
    has_local_derivation: hasLocalDerivation,
  };
}

function extractFunctions(sourceFile) {
  const functions = [];
  const filePath = path.relative(projectRoot, sourceFile.fileName);

  function visit(node) {
    // Function declarations
    if (ts.isFunctionDeclaration(node) && node.name) {
      functions.push({
        file_path: filePath,
        name: node.name.text,
        exported: isExported(node),
        line_start: sourceFile.getLineAndCharacterOfPosition(node.getStart()).line + 1,
        line_count: getLineCount(node, sourceFile),
        param_count: getParamCount(node),
        branching_depth: getMaxBranchingDepth(node.body || node),
        is_async: isAsync(node),
        prop_count: getPropCount(node, sourceFile),
      });
    }

    // Arrow functions and function expressions assigned to variables
    if (ts.isVariableStatement(node)) {
      const decls = node.declarationList.declarations;
      for (const decl of decls) {
        if (decl.initializer && (ts.isArrowFunction(decl.initializer) || ts.isFunctionExpression(decl.initializer))) {
          const name = decl.name && ts.isIdentifier(decl.name) ? decl.name.text : '<anonymous>';
          functions.push({
            file_path: filePath,
            name,
            exported: isExported(node),
            line_start: sourceFile.getLineAndCharacterOfPosition(node.getStart()).line + 1,
            line_count: getLineCount(node, sourceFile),
            param_count: getParamCount(decl.initializer),
            branching_depth: getMaxBranchingDepth(decl.initializer.body || decl.initializer),
            is_async: isAsync(decl.initializer),
            prop_count: getPropCount(decl.initializer, sourceFile),
          });
        }
      }
    }

    // Methods in object literals or classes
    if (ts.isMethodDeclaration(node) && node.name) {
      const name = ts.isIdentifier(node.name) ? node.name.text : ts.isStringLiteral(node.name) ? node.name.text : '<computed>';
      functions.push({
        file_path: filePath,
        name,
        exported: false,
        line_start: sourceFile.getLineAndCharacterOfPosition(node.getStart()).line + 1,
        line_count: getLineCount(node, sourceFile),
        param_count: getParamCount(node),
        branching_depth: getMaxBranchingDepth(node.body || node),
        is_async: isAsync(node),
      });
    }

    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  return functions;
}

// File-level metrics
const fileMetrics = [];

for (const sourceFile of program.getSourceFiles()) {
  if (sourceFile.isDeclarationFile) continue;
  if (!isProjectFile(sourceFile.fileName)) continue;

  const filePath = path.relative(projectRoot, sourceFile.fileName);
  const lineCount = sourceFile.getLineAndCharacterOfPosition(sourceFile.getEnd()).line + 1;

  const fns = extractFunctions(sourceFile);
  const exportedCount = fns.filter(f => f.exported).length;
  const internalCount = fns.filter(f => !f.exported).length;

  // Classify file
  let classification = 'logic';
  const text = sourceFile.getFullText();
  if (/^(export\s+)?(type|interface|enum)\s/m.test(text) && !fns.length) {
    classification = 'types';
  } else if (/create\s*\(/.test(text) && /zustand|create/.test(text)) {
    classification = 'store';
  } else if (/^(export\s+default\s+)?function\s+use[A-Z]|const\s+use[A-Z].*=/.test(text)) {
    classification = 'hook';
  } else if (/return\s*\(?\s*<|React\.createElement|jsx/.test(text)) {
    classification = 'component';
  } else if (/export\s+(async\s+)?function\s+(GET|POST|PUT|DELETE|PATCH)\b/.test(text) || /export\s+default\s+async\s+function\s+Page/.test(text)) {
    classification = 'route';
  } else if (fns.length === 0 && /^(export\s+)?const\s/m.test(text)) {
    classification = 'data';
  }

  // Detect shell pattern for component files
  const shellInfo = classification === 'component' ? detectShellPattern(sourceFile) : null;

  // Get max prop count across all functions in this file
  const maxPropCount = Math.max(-1, ...fns.map(f => f.prop_count));

  // Output file record
  console.log(JSON.stringify({
    type: 'file',
    path: filePath,
    lines: lineCount,
    exports: exportedCount,
    internal_fns: internalCount,
    classification,
    prop_count: maxPropCount > 0 ? maxPropCount : null,
    child_component_count: shellInfo?.child_component_count ?? null,
    has_local_derivation: shellInfo?.has_local_derivation ?? null,
  }));

  // Output function records
  for (const fn of fns) {
    console.log(JSON.stringify({ type: 'function', ...fn }));
  }
}

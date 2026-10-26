import path from 'node:path';

const ROOTS = ['features', 'shared', 'pages', 'lib', 'stores', 'api', 'test', 'types'];
const ROOT_FILES = ['App.tsx', 'main.tsx', 'vite-env.d.ts'];

function srcPath(context, file) {
  return path.relative(path.join(context.cwd, 'src'), file).split(path.sep).join('/');
}

function target(context, importer, spec) {
  if (spec.startsWith('@/')) return spec.slice(2);
  if (!spec.startsWith('.')) return null;
  return srcPath(context, path.resolve(path.dirname(importer), spec));
}

function featureOf(file) {
  const [root, feature] = file.split('/');
  return root === 'features' ? feature : null;
}

export default {
  meta: {
    type: 'problem',
    messages: {
      placement: 'Frontend code lives under src/{{roots}}; "{{file}}" belongs in one of them.',
      internals: 'Import "{{spec}}" through @/features/{{feature}}, its index, not from inside the feature.',
    },
    schema: [],
  },
  create(context) {
    const file = srcPath(context, context.filename);
    if (file.startsWith('..')) return {};
    const [root] = file.split('/');
    if (!file.includes('/') ? !ROOT_FILES.includes(file) : !ROOTS.includes(root)) {
      return {
        Program(node) {
          context.report({ node, messageId: 'placement', data: { file, roots: ROOTS.join(', ') } });
        },
      };
    }
    const own = featureOf(file);
    if (!own || /\.(test|bench)\.tsx?$/.test(file)) return {};

    const check = (source) => {
      if (!source || typeof source.value !== 'string') return;
      const resolved = target(context, context.filename, source.value);
      if (!resolved) return;
      const feature = featureOf(resolved);
      if (!feature || feature === own) return;
      const rest = resolved.split('/').slice(2);
      if (rest.length === 0 || (rest.length === 1 && rest[0] === 'index')) return;
      context.report({ node: source, messageId: 'internals', data: { spec: source.value, feature } });
    };
    return {
      ImportDeclaration: (node) => check(node.source),
      ExportNamedDeclaration: (node) => check(node.source),
      ExportAllDeclaration: (node) => check(node.source),
      ImportExpression: (node) => check(node.source),
    };
  },
};

import js from '@eslint/js';
import globals from 'globals';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import tseslint from 'typescript-eslint';
import prettier from 'eslint-config-prettier';
import { defineConfig, globalIgnores } from 'eslint/config';
import iconButtonLabel from './eslint-rules/icon-button-label.js';

export default defineConfig([
  globalIgnores([
    'dist',
    'release',
    'playwright-report',
    'test-results',
    'src/api/schema.d.ts',
    'src/api/serverFrames.ts',
  ]),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      prettier,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    rules: {
      '@typescript-eslint/no-explicit-any': 'error',
      'no-console': 'error',
    },
  },
  {
    files: ['src/**/*.tsx'],
    ignores: ['src/shared/components/**'],
    plugins: { local: { rules: { 'icon-button-label': iconButtonLabel } } },
    rules: {
      'local/icon-button-label': 'error',
      'no-restricted-syntax': [
        'error',
        {
          selector: "JSXAttribute[name.name='className'] Literal[value=/hover:bg-purple-500/]",
          message: 'Use <Button variant="primary"> (or buttonStyles) from shared/components/Button.',
        },
        {
          selector: "JSXAttribute[name.name='className'] TemplateElement[value.raw=/hover:bg-purple-500/]",
          message: 'Use <Button variant="primary"> (or buttonStyles) from shared/components/Button.',
        },
        {
          selector: "JSXAttribute[name.name='className'] Literal[value=/rounded-full animate-spin/]",
          message: 'Use <Spinner /> from shared/components/Spinner.',
        },
        {
          selector:
            "JSXOpeningElement[name.name=/^(input|textarea|select)$/]:not(:has(JSXAttribute[name.name='type'][value.value='file']))",
          message: 'Use TextInput, TextArea, Select, Checkbox or RadioCards from shared/components/Form.',
        },
        {
          selector: "CallExpression[callee.property.name='click'][callee.object.property.name='current']",
          message:
            'A hidden file input opened with .click() does nothing in an iOS home-screen app; use FilePicker from shared/components/Form.',
        },
      ],
    },
  },
  {
    // The logger is the one place allowed to touch the console; config files run
    // in Node and may log build diagnostics.
    files: ['src/lib/logger.ts', '*.config.ts', '*.config.js'],
    rules: {
      'no-console': 'off',
    },
  },
  {
    // The tiptap editor is configured once and kept, so the mention lookup has
    // to reach the current list through a ref. React Compiler cannot see that
    // the callback only runs while somebody is typing, and rebuilding the
    // extension on every members change would drop the editor's state.
    files: ['src/features/messaging/MessageInput.tsx'],
    rules: {
      'react-hooks/refs': 'warn',
    },
  },
  {
    // Benchmarks define throwaway components to measure against; they are never
    // part of a bundle, so fast refresh has nothing to say about them.
    files: ['**/*.bench.tsx'],
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
]);

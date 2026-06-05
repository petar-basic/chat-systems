import '@testing-library/jest-dom/vitest';
import { configure } from '@testing-library/dom';

configure({ testIdAttribute: 'data-qa' });

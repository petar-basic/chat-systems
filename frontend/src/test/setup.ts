import '@testing-library/jest-dom/vitest';
import { configure } from '@testing-library/dom';

configure({ testIdAttribute: 'data-qa' });

// jsdom implements neither of these; both are layout APIs the message list uses.
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = function scrollTo(this: Element, options?: ScrollToOptions | number) {
    const top = typeof options === 'number' ? options : (options?.top ?? 0);
    (this as HTMLElement).scrollTop = top;
  };
}
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

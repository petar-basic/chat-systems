export default {
  meta: {
    type: 'problem',
    messages: {
      missing: 'An icon-only <button> needs an aria-label — or use <IconButton label="…">.',
    },
    schema: [],
  },
  create(context) {
    return {
      JSXElement(node) {
        const opening = node.openingElement;
        if (opening.name.type !== 'JSXIdentifier' || opening.name.name !== 'button') return;
        const named = opening.attributes.some(
          (attr) =>
            attr.type === 'JSXSpreadAttribute' ||
            (attr.type === 'JSXAttribute' && ['aria-label', 'aria-labelledby'].includes(attr.name.name)),
        );
        if (named) return;
        const content = node.children.filter((child) => !(child.type === 'JSXText' && !child.value.trim()));
        if (content.length === 0) return;
        const iconsOnly = content.every(
          (child) =>
            child.type === 'JSXElement' &&
            child.openingElement.selfClosing &&
            child.openingElement.name.type === 'JSXIdentifier' &&
            /^[A-Z]/.test(child.openingElement.name.name),
        );
        if (iconsOnly) context.report({ node: opening, messageId: 'missing' });
      },
    };
  },
};

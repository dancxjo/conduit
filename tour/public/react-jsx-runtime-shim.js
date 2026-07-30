const React = window.React;

export const Fragment = React.Fragment;
export const jsx = (type, props, key) =>
  React.createElement(type, key === undefined ? props : { ...props, key });
export const jsxs = jsx;


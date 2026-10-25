// Exact enacted matrix; null captures are checked through serial custody.
export const NATIVE_LOOK_MATRIX = Object.freeze([
  ['f7', 'compact', 'editorial', 100, 'editorial', 'creche-editorial-compact', true],
  ['f6', 'spacious', 'editorial', 125, 'editorial', 'creche-editorial-spacious', false],
  ['f7', 'spacious', 'phosphor', 125, 'editorial', 'creche-phosphor-spacious', false],
  ['f6', 'compact', 'phosphor', 100, 'editorial', 'creche-phosphor-compact', false],
  ['f8', 'compact', 'phosphor', 100, 'engineering', 'creche-engineering-phosphor-compact', true],
  ['f7', 'compact', 'editorial', 100, 'engineering', 'creche-engineering-editorial-compact', true],
  ['f6', 'spacious', 'editorial', 125, 'engineering', 'creche-engineering-editorial-spacious', false],
  ['f7', 'spacious', 'phosphor', 125, 'engineering', 'creche-engineering-phosphor-spacious', true],
  ['f6', 'compact', 'phosphor', 100, 'engineering', 'creche-engineering-phosphor-restored', false],
  ['f8', 'compact', 'phosphor', 100, 'editorial', 'creche-editorial-type-restored', false],
].map(Object.freeze));

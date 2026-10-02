// Rendering and validation share the application's canonical Rust/WASM syntax
// projection adapter. The SDK introduces no separate tokenizer or parser.
export {
  attachConduitSyntaxEditor as attachBrowserSyntaxEditor,
  projectConduitSyntax as projectBrowserSyntax,
} from '../host/assets/application-syntax-presentation.mjs';

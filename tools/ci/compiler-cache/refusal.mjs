export class CacheRefusal extends Error {
  constructor(code, message = code) { super(message); this.code = code; }
}

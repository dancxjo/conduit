import base from './playwright.config.mjs';
export default { ...base, testMatch: ['browser-lifetime.spec.mjs'], workers: 1, retries: 0, projects: [{ name: 'chromium', use: { browserName: 'chromium' } }] };

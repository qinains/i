import {defineConfig} from '@playwright/test';
export default defineConfig({
    testDir: './tests', testMatch: '**/*.spec.mjs', workers: 1,
    use: {baseURL: 'http://127.0.0.1:4173', acceptDownloads: true,
        trace:'retain-on-failure',screenshot:'only-on-failure'},
    projects: [{name:'chromium',use:{browserName:'chromium'}},{name:'firefox',use:{browserName:'firefox'}},{name:'webkit',use:{browserName:'webkit'}}],
    webServer: {command: 'node tests/mock-server.mjs', url: 'http://127.0.0.1:4173', reuseExistingServer: false},
});

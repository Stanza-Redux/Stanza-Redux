// Synthetic legacy data; uses newly created temporary browser profiles only.
// NODE_PATH=/path/to/node_modules node tests/persistence-browser.cjs http://127.0.0.1:PORT
const { chromium, webkit } = require('playwright');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { mkdtempSync, readFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const http = require('node:http');
const upstream = new URL(process.argv[2]);
const temp = mkdtempSync(join(tmpdir(), 'stanza-migration-'));
const path = join(temp, 'legacy.sqlite');
execFileSync('python3', ['-c', `
import sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    db.executescript('''
        CREATE TABLE books(id TEXT PRIMARY KEY, title TEXT, author TEXT, cover TEXT, added INTEGER, chapter INTEGER, progress REAL);
        INSERT INTO books VALUES ('legacy-fixture','Preserved title','Creator',NULL,10,3,0.5);
        CREATE TABLE book_metadata(id TEXT PRIMARY KEY,json TEXT);
        INSERT INTO book_metadata VALUES ('legacy-fixture','{"authors":["Creator"],"subjects":["Astronomy"]}');
        CREATE TABLE reading_history(book TEXT PRIMARY KEY,opened INTEGER);
        INSERT INTO reading_history VALUES ('legacy-fixture',100);
        CREATE TABLE book_facets(id TEXT PRIMARY KEY,book TEXT,kind TEXT,name TEXT);
        CREATE TABLE imports(key TEXT PRIMARY KEY);
        INSERT INTO imports VALUES ('starter-book-v1');
    ''')
`, path]);
const bytes = [...readFileSync(path)];
rmSync(temp, { recursive: true });
(async () => {
  for (const engine of (process.env.BROWSER === 'all' ? [chromium, webkit] : process.env.BROWSER === 'webkit' ? [webkit] : [chromium])) {
    // Give each test a fresh origin as well: some WebKit hosts share website data stores.
    const proxy = http.createServer((req, res) => {
      const forward = http.request(new URL(req.url, upstream), { method: req.method, headers: req.headers }, reply => {
        res.writeHead(reply.statusCode, reply.headers); reply.pipe(res);
      });
      forward.on('error', error => { res.writeHead(502); res.end(String(error)); });
      req.pipe(forward);
    });
    await new Promise(resolve => proxy.listen(0, '127.0.0.1', resolve));
    const origin = 'http://127.0.0.1:' + proxy.address().port;
    // WebKit disables OPFS in private contexts. Use a new temporary profile, never a user profile.
    const profile = mkdtempSync(join(tmpdir(), 'stanza-browser-fixture-'));
    const context = await engine.launchPersistentContext(profile, { viewport: { width: 1200, height: 850 } });
    try {
      const page = await context.newPage();
      page.setDefaultTimeout(30000);
      const errors = [];
      page.on('pageerror', error => { errors.push(error.message); console.error(engine.name(), error.message); });
      page.on('console', message => { if (process.env.DEBUG || ['warning', 'error'].includes(message.type())) console.log(engine.name(), message.text()); });
      await page.route(origin + '/', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Fixture</title>' }));
      await page.goto(origin);
      await page.evaluate(async bytes => {
        const root = await navigator.storage.getDirectory();
        for await (const name of root.keys()) {
          throw new Error('Refusing to seed nonempty OPFS: ' + name);
        }
        const dir = await root.getDirectoryHandle('.day-sql', { create: true });
        for (const [name, data] of [['map.json', JSON.stringify({ 'stanza-library.sqlite': 0 })], ['pool-0', new Uint8Array(bytes)]]) {
          const file = await dir.getFileHandle(name, { create: true });
          const writer = await file.createWritable();
          await writer.write(data);
          await writer.close();
        }
      }, bytes);
      await page.unroute(origin + '/');
      await page.reload();
      for (let pass = 0; pass < 2; pass++) {
        await page.locator('#library-folders').waitFor();
        if (process.env.DEBUG) console.log('Before recents', pass, await page.locator('body').innerText());
        await page.locator('#library-folders [role=option]').nth(0).click();
        if (process.env.DEBUG) console.log('After recents', pass, await page.locator('body').innerText());
        await page.locator('#library-list [role=option]').filter({ hasText: 'Preserved title' }).waitFor();
        await page.locator('#library-search').fill('Astronomy');
        await page.locator('#library-list [role=option]').filter({ hasText: 'Preserved title' }).click();
        await page.locator('#local-book-title').waitFor();
        assert.equal(await page.locator('#local-book-title').innerText(), 'Preserved title');
        assert.match(await page.locator('#book-metadata').innerText(), /Astronomy/);
        if (pass === 0) await page.goto(origin + '/');
      }
      assert.deepEqual(errors, []);
      console.log(engine.name() + ': PASS legacy OPFS migration, FTS metadata, recents, detail navigation and reload');
    } catch (error) {
      console.error(await context.pages()[0]?.locator('body').innerText());
      throw error;
    } finally {
      await context.close();
      rmSync(profile, { recursive: true });
      proxy.closeAllConnections();
      await new Promise(resolve => proxy.close(resolve));
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });

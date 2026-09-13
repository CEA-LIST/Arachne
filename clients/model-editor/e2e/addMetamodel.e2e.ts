/// <reference types="node" />
/**
 * A language is given to a running replica from the browser, and the replica
 * next door gets it from a model rather than from a person.
 *
 * The claim under test is decision D8 in the demonstrable form: the Metamodel
 * tab's **Add to this replica** posts a descriptor's text to
 * `POST /api/metamodels` on alice, alice serves it from that moment, a model
 * is created in that language and edited — and bob, who was never given the
 * descriptor and whose own listing did not have it, joins that model by id,
 * converges on it, and ends up listing the language too. The descriptor
 * reached bob inside the `Install` operation that opened the model's log and
 * by no other route, which is what the success line in the UI says out loud.
 *
 * A fourth step is the refusal: a file the node will not serve comes back 422
 * and the panel prints the node's own sentence.
 *
 * The replicas are the ones `docker/compose/stack_interpreter.sh` starts:
 *
 *     ./stack_interpreter.sh infra up
 *     ./stack_interpreter.sh up alice --port 8081
 *     ./stack_interpreter.sh up bob   --port 8082
 *
 * `MOIRAI_LIVE_NODES` names them (two comma-separated base URLs, default
 * those two ports) and Chrome is the system binary (`CHROME_BIN`, default
 * `/usr/bin/google-chrome`). With either missing the scenario follows the
 * suite's skip contract: an `E2E-SKIP` line and a skip.
 *
 * # Where the descriptor comes from
 *
 * It must be one no replica holds, or the first assertion is vacuous. The
 * rig's image ships `arachne/examples/` as `/metamodels`, so a descriptor
 * checked in beside the others would be in every replica from start-up and
 * this scenario would be testing nothing. So: `MOIRAI_DEMO_DESCRIPTOR` names
 * a descriptor file when there is one to hand (`arachne describe
 * examples/class_diagram.ecore`), and without it the scenario builds one by
 * retagging a descriptor alice already serves under an nsURI unique to the
 * run. The digest is taken over the parsed descriptor, so a retag is a
 * different metamodel, and it is a metamodel nobody could have preloaded.
 */

import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer, { type Browser, type Page } from 'puppeteer-core';
import { describe, expect, it } from 'vitest';
import { getHealth, getMetamodels } from '../src/api/client';
import type { MetamodelId, MetamodelListing, ModelId, PlainJson } from '../src/api/types';
import { servedHereLine } from '../src/explorer/addMetamodel';
import { freePort, scratchDir, waitFor } from '../src/testing/liveNodes';

const EDITOR_ROOT = fileURLToPath(new URL('../', import.meta.url));
const CHROME = process.env['CHROME_BIN'] ?? '/usr/bin/google-chrome';
const NODES = (process.env['MOIRAI_LIVE_NODES'] ?? 'http://127.0.0.1:8081,http://127.0.0.1:8082')
  .split(',')
  .map((url) => url.trim());

/* ---------- the stack, and the reasons this cannot run ---------- */

async function unreachable(): Promise<string | null> {
  if (!existsSync(CHROME)) {
    const reason = `E2E-SKIP add-metamodel: no Chrome at ${CHROME}; set CHROME_BIN`;
    console.warn(reason);
    return reason;
  }
  for (const url of NODES) {
    try {
      await getHealth(url);
    } catch (err) {
      const reason = `E2E-SKIP add-metamodel: ${url} does not answer /api/health (${
        err instanceof Error ? err.message : String(err)
      }); start the stack with docker/compose/stack_interpreter.sh, or name other replicas in MOIRAI_LIVE_NODES`;
      console.warn(reason);
      return reason;
    }
  }
  return null;
}

/* ---------- processes ---------- */

type Teardown = () => Promise<void> | void;

function vite(args: string[], logPath: string): Promise<void> {
  const fd = openSync(logPath, 'w');
  const child = spawn(join(EDITOR_ROOT, 'node_modules', '.bin', 'vite'), args, {
    cwd: EDITOR_ROOT,
    stdio: ['ignore', fd, fd],
  });
  return new Promise((resolve, reject) => {
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`vite ${args[0]} exited ${code}; see ${logPath}`)),
    );
    child.on('error', (err: Error) => reject(err));
  });
}

async function servePreview(logPath: string): Promise<{ url: string; child: ChildProcess }> {
  const port = await freePort();
  const fd = openSync(logPath, 'w');
  const child = spawn(
    join(EDITOR_ROOT, 'node_modules', '.bin', 'vite'),
    ['preview', '--host', '127.0.0.1', '--port', String(port), '--strictPort'],
    { cwd: EDITOR_ROOT, stdio: ['ignore', fd, fd] },
  );
  const url = `http://127.0.0.1:${port}/`;
  await waitFor(`the editor at ${url}`, async () => {
    if (child.exitCode !== null) throw new Error(`vite preview exited ${child.exitCode}; see ${logPath}`);
    const response = await fetch(url).catch(() => null);
    return response !== null && response.ok && (await response.text()).includes('<div id="root">') ? true : null;
  });
  return { url, child };
}

function killProcess(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  const exited = new Promise<void>((resolve) => child.once('exit', () => resolve()));
  child.kill('SIGKILL');
  return exited;
}

async function launchChrome(profileDir: string): Promise<Browser> {
  const base = {
    executablePath: CHROME,
    headless: true as const,
    userDataDir: profileDir,
    defaultViewport: { width: 1400, height: 900 },
  };
  const args = ['--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-background-networking'];
  try {
    return await puppeteer.launch({ ...base, args });
  } catch (err: unknown) {
    console.warn(`chrome did not launch with its sandbox (${err instanceof Error ? err.message : String(err)})`);
    return puppeteer.launch({ ...base, args: [...args, '--no-sandbox'] });
  }
}

/* ---------- the editor, driven through its own controls ---------- */

async function openEditor(browser: Browser, url: string, node: string): Promise<Page> {
  const context = await browser.createBrowserContext();
  const page = await context.newPage();
  page.on('pageerror', (err: unknown) => console.warn(`[pageerror] ${err instanceof Error ? err.message : String(err)}`));
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForSelector('.me-topbar');
  await page.click('header button[aria-haspopup="dialog"]');
  const input = await page.waitForSelector('.me-connect input[type="text"]');
  if (input === null) throw new Error('no URL field');
  await input.click({ count: 3 });
  await input.type(node);
  await page.click('.me-connect button[type="submit"]');
  await page.waitForSelector('.me-syncchip--live', { timeout: 20_000 });
  await page.waitForSelector('ul[aria-label="Hosted models"]', { timeout: 20_000 });
  return page;
}

async function showExplorerTab(page: Page, tab: 'models' | 'model' | 'metamodel'): Promise<void> {
  await page.click(`#tab-${tab}`);
  await page.waitForSelector(`#panel-${tab}`);
}

/** Hand `path` to the Metamodel tab's add control, and answer with the line it leaves. */
async function addInUi(page: Page, path: string): Promise<{ tone: 'ok' | 'bad'; line: string }> {
  await showExplorerTab(page, 'metamodel');
  const picker = await page.waitForSelector('input[data-testid="add-metamodel-file"]');
  if (picker === null) throw new Error('no add-metamodel picker');
  await picker.uploadFile(path);
  const note = await page.waitForSelector('.me-meta__note--ok, .me-meta__note--bad', { timeout: 30_000 });
  if (note === null) throw new Error('the add left no line');
  return note.evaluate((el) => ({
    tone: el.className.includes('--ok') ? ('ok' as const) : ('bad' as const),
    line: el.textContent ?? '',
  }));
}

/** The digests the New model dropdown offers right now. */
function offeredDigests(page: Page): Promise<string[]> {
  return page.$$eval('select[aria-label="Metamodel for the new model"] option', (options) =>
    options.map((option) => (option as HTMLOptionElement).value),
  );
}

function openTabIds(page: Page): Promise<ModelId[]> {
  return page.$$eval('.me-doctabs__tab[data-model-id]', (tabs) => tabs.map((tab) => tab.getAttribute('data-model-id') ?? ''));
}

/** Create a model through the New model form; the id of the tab that opens, which the node minted. */
async function createInUi(page: Page, digest: string): Promise<ModelId> {
  await showExplorerTab(page, 'models');
  const before = new Set(await openTabIds(page));
  await page.select('select[aria-label="Metamodel for the new model"]', digest);
  await page.click('form[aria-label="New model"] button[type="submit"]');
  return waitFor("the created model's tab", async () => {
    const fresh = (await openTabIds(page)).filter((id) => !before.has(id));
    return fresh.length === 1 ? fresh[0] : null;
  });
}

/** Create the model's root through the tree's own control, choosing `className` when it offers a menu. */
async function createRootInUi(page: Page, className: string): Promise<void> {
  await showExplorerTab(page, 'model');
  const button = await page.waitForSelector('::-p-xpath(//button[contains(., "Create root")])', { timeout: 30_000 });
  if (button === null) throw new Error('no Create root control');
  await button.click();
  const menu = await page.$('div[role="menu"]');
  if (menu !== null) {
    const item = await page.waitForSelector(`::-p-xpath(//div[@role="menu"]//button[normalize-space(text())="${className}"])`);
    if (item === null) throw new Error(`no menu item for ${className}`);
    await item.click();
  }
  await page.waitForSelector('[role="treeitem"]', { timeout: 30_000 });
}

/** Type `value` into the properties form's input for the attribute `name`. */
async function setAttributeInUi(page: Page, attribute: string, value: string): Promise<void> {
  await page.click('[role="treeitem"]');
  const selector = `::-p-xpath(//div[contains(@class,"me-field")][.//span[normalize-space(text())="${attribute}"]]//input)`;
  const input = await page.waitForSelector(selector, { timeout: 30_000 });
  if (input === null) throw new Error(`no input for ${attribute}`);
  await page.waitForFunction((el: Element) => !(el as HTMLInputElement).readOnly, { timeout: 30_000 }, input);
  await input.click({ count: 3 });
  await input.type(value);
  await page.keyboard.press('Tab');
}

/* ---------- the nodes, read straight ---------- */

async function stateOf(url: string, id: ModelId): Promise<PlainJson> {
  const response = await fetch(`${url}/api/model/${id}/state`);
  if (!response.ok) throw new Error(`${url} /api/model/${id}/state: ${response.status}`);
  return (await response.json()) as PlainJson;
}

/** POST /api/models on `url`: the raw status and body, since a join's status is part of the claim. */
async function joinOnNode(url: string, id: ModelId, metamodelId: MetamodelId): Promise<{ status: number; body: string }> {
  const response = await fetch(`${url}/api/models`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model_id: id, metamodel_id: metamodelId }),
  });
  return { status: response.status, body: await response.text() };
}

function mentions(value: PlainJson, needle: string): boolean {
  return JSON.stringify(value).includes(needle);
}

/* ---------- the scenario ---------- */

describe('add a metamodel to a running replica, from the browser', () => {
  it('a_language_added_in_the_ui_reaches_one_replica_and_the_next_one_through_a_model', { timeout: 600_000 }, async (ctx) => {
    const skip = await unreachable();
    if (skip !== null) return ctx.skip(skip);
    const [aliceUrl, bobUrl] = NODES;
    const run = scratchDir('add-metamodel');
    const teardown: Teardown[] = [];

    try {
      /* The descriptor: one to hand, or one nobody could have preloaded. */
      const named = process.env['MOIRAI_DEMO_DESCRIPTOR'];
      let text: string;
      if (named !== undefined && named.length > 0) {
        text = readFileSync(named, 'utf8');
      } else {
        const held = await (await fetch(`${aliceUrl}/api/metamodel`)).text();
        const parsed = JSON.parse(held) as Record<string, unknown>;
        const tag = `run${Date.now()}`;
        parsed['nsURI'] = `http://www.example.org/${tag}`;
        parsed['package'] = tag;
        text = `${JSON.stringify(parsed, null, 2)}\n`;
      }
      const descriptorPath = join(run, 'descriptor.json');
      writeFileSync(descriptorPath, text);
      const nsURI = (JSON.parse(text) as { nsURI: string }).nsURI;
      const rootClass = (JSON.parse(text) as { rootClasses: string[] }).rootClasses[0];
      console.log(`add-metamodel: the language is ${nsURI}, root ${rootClass}`);

      /* 1a. Neither replica holds it. */
      const before = await Promise.all(NODES.map((url) => getMetamodels(url)));
      for (const [index, listing] of before.entries()) {
        expect(listing.map((entry) => entry.nsURI), `${NODES[index]} must not hold ${nsURI} yet`).not.toContain(nsURI);
      }

      /* The editor, built and served, and Chrome. */
      await vite(['build', '--logLevel', 'warn'], join(run, 'vite-build.log'));
      const preview = await servePreview(join(run, 'vite-preview.log'));
      teardown.push(() => killProcess(preview.child));
      const browser = await launchChrome(join(run, 'chrome-profile'));
      teardown.push(() => browser.close());
      const alice = await openEditor(browser, preview.url, aliceUrl);

      /* 1b. Added from the UI; alice lists it where it did not before. */
      const added = await addInUi(alice, descriptorPath);
      expect(added.tone, `the add left: ${added.line}`).toBe('ok');
      const listed = await getMetamodels(aliceUrl);
      const entry = listed.find((candidate) => candidate.nsURI === nsURI);
      expect(entry, `alice must list ${nsURI} after the add: ${JSON.stringify(listed)}`).toBeDefined();
      const language = entry as MetamodelListing;
      // The line is the architecture's, word for word: one replica now, the
      // others through a model written in the language and nothing else.
      expect(added.line).toBe(servedHereLine(language, true));

      /* And the dropdowns have it, with no reload. */
      await showExplorerTab(alice, 'models');
      expect(await offeredDigests(alice)).toContain(language.digest);

      /* 2. A model in that language, created and edited from the editor. */
      const modelId = await createInUi(alice, language.digest);
      console.log(`add-metamodel: alice minted ${modelId}`);
      await createRootInUi(alice, rootClass);
      await setAttributeInUi(alice, 'name', 'Door');
      await waitFor(`alice's ${modelId.slice(0, 8)}… to carry the edit`, async () =>
        mentions(await stateOf(aliceUrl, modelId), 'Door') ? true : null,
      );

      /* 3. Bob, who was never given the descriptor, joins by id. */
      const bobBefore = await getMetamodels(bobUrl);
      expect(bobBefore.map((e) => e.nsURI)).not.toContain(nsURI);
      const joined = await joinOnNode(bobUrl, modelId, { nsURI: language.nsURI, digest: language.digest });
      expect(joined.status, `bob refused the join: ${joined.body}`).toBe(200);
      const onBob = await waitFor(`bob to converge on ${modelId.slice(0, 8)}…`, async () => {
        const state = await stateOf(bobUrl, modelId).catch(() => null);
        return state !== null && mentions(state, 'Door') ? state : null;
      });
      expect(JSON.stringify(onBob)).toBe(JSON.stringify(await stateOf(aliceUrl, modelId)));
      // The descriptor arrived with the model, and bob now serves the language.
      const bobAfter = await waitFor('bob to list the language it was never given', async () => {
        const listing = await getMetamodels(bobUrl);
        return listing.some((candidate) => candidate.digest === language.digest) ? listing : null;
      });
      expect(bobAfter.map((e) => e.digest)).toContain(language.digest);

      /* 4. A file this node will not serve: the node's own sentence, in the UI. */
      const badPath = join(run, 'not-a-descriptor.ecore');
      writeFileSync(badPath, '<?xml version="1.0" encoding="UTF-8"?>\n<ecore:EPackage name="nope"/>\n');
      const refused = await addInUi(alice, badPath);
      expect(refused.tone, `a malformed descriptor left: ${refused.line}`).toBe('bad');
      expect(refused.line).toContain('not a descriptor this node can serve');
      console.log(`add-metamodel: the node's refusal, as the panel printed it: ${refused.line}`);
    } finally {
      for (const step of teardown.reverse()) await step();
    }
  });
});

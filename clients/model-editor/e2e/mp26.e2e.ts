/// <reference types="node" />
/**
 * mp26, the level-4 user scenario, in headless Chrome against two live nodes:
 * two models of two metamodels, edited in two tabs on each of two editors,
 * converge per model with no cross-talk (M-A9), and mp29's user-visible half
 * on the way: the list, a create whose id the node minted, a join by id that
 * appears in the other node's list, and a tab bound to its own descriptor.
 *
 * What runs. The editor is built with `vite build` and served by `vite
 * preview` on the loopback interface (a secure context, which the store's
 * OPFS and the digest's `crypto.subtle` need). Two `network_node` processes,
 * editor-a and editor-b, peer with each other and both hold the `bt` and
 * `uml` descriptors. Chrome is the system binary (`CHROME_BIN`, default
 * /usr/bin/google-chrome) driven through puppeteer-core, with a throwaway
 * profile under the run directory; each editor is a page in its own browser
 * context, so each has its own origin-private file system. Everything is
 * killed at the end, whichever way the test ends.
 *
 * The steps, as the validation plan writes them: create a behaviour-tree
 * model on editor-a and join it by id on editor-b; create a SimpleUML model
 * on editor-b and join it by id on editor-a; four tabs; rename a Sequence in
 * the behaviour-tree tab, add a Class named Door in the UML tab. Asserted:
 * each model's two tabs show the same document, which is the document both
 * nodes serve; the behaviour-tree document holds no Class and the UML
 * document no Sequence; each tab was served the descriptor its header names;
 * and each browser context's store holds exactly one file per model, whose
 * bytes are the canonical JSON of that model's state.
 *
 * Skips, with an `E2E-SKIP` line, when there is no node binary or no Chrome.
 */

import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, openSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer, { type Browser, type BrowserContext, type ElementHandle, type Page } from 'puppeteer-core';
import { describe, expect, it } from 'vitest';
import type { Descriptor, MetamodelId, ModelId, PlainJson } from '../src/api/types';
import { addChildOps, createRootOps, createSingleContainmentOps, setStringOps } from '../src/crdt/ops';
import { canonicalJson } from '../src/model/digest';
import { modelHeaderOf } from '../src/model/instance';
import {
  applyOps,
  freePort,
  HARNESS_LOG_ID,
  hostedIds,
  modelState,
  readExampleDescriptor,
  scratchDir,
  skipReason,
  sortKeys,
  startNodes,
  waitFor,
  waitForState,
  writeMetamodelDir,
  type LiveNode,
} from '../src/testing/liveNodes';

const EDITOR_ROOT = fileURLToPath(new URL('../', import.meta.url));
const CHROME = process.env['CHROME_BIN'] ?? '/usr/bin/google-chrome';

const bt = readExampleDescriptor('bt.metamodel.json') as Descriptor;
const uml = readExampleDescriptor('uml.metamodel.json') as Descriptor;
const digests = readExampleDescriptor('fixtures/metamodel-digests.json') as Record<string, MetamodelId>;
const btId = digests['bt.metamodel.json'];
const umlId = digests['uml.metamodel.json'];

/* ---------- processes: the editor's server and Chrome ---------- */

type Teardown = () => Promise<void> | void;

/** Run `vite <args>` to completion, its output to `logPath`. */
function vite(args: string[], logPath: string): Promise<void> {
  const fd = openSync(logPath, 'w');
  const child = spawn(join(EDITOR_ROOT, 'node_modules', '.bin', 'vite'), args, {
    cwd: EDITOR_ROOT,
    stdio: ['ignore', fd, fd],
  });
  return new Promise((resolve, reject) => {
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`vite ${args[0]} exited ${code}; see ${logPath}`))));
    child.on('error', (err: Error) => reject(err));
  });
}

/** `vite preview` on a free loopback port; the URL it answers on. */
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

/** Headless Chrome with a throwaway profile; the sandbox is tried first and dropped only if the launch fails. */
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
    const why = err instanceof Error ? err.message : String(err);
    console.warn(`chrome did not launch with its sandbox (${why}); retrying with --no-sandbox`);
    return puppeteer.launch({ ...base, args: [...args, '--no-sandbox'] });
  }
}

/* ---------- the editor, driven through its own controls ---------- */

interface Editor {
  name: string;
  node: LiveNode;
  context: BrowserContext;
  page: Page;
  /** The page's console and errors, for the report on a failure. */
  console: string[];
}

async function openEditor(name: string, node: LiveNode, browser: Browser, url: string): Promise<Editor> {
  const context = await browser.createBrowserContext();
  const page = await context.newPage();
  const console: string[] = [];
  page.on('console', (message) => console.push(`[${message.type()}] ${message.text()}`));
  page.on('pageerror', (err: unknown) => console.push(`[pageerror] ${err instanceof Error ? err.message : String(err)}`));
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForSelector('.me-topbar');
  return { name, node, context, page, console };
}

/** The Connect popover: the node's URL typed in, Connect pressed, the chip live. */
async function connect(editor: Editor): Promise<void> {
  const { page, node } = editor;
  await page.click('header button[aria-haspopup="dialog"]');
  const input = await page.waitForSelector('.me-connect input[type="text"]');
  if (input === null) throw new Error('no URL field');
  await input.click({ count: 3 });
  await input.type(node.url);
  await page.click('.me-connect button[type="submit"]');
  await page.waitForSelector('.me-syncchip--live', { timeout: 20_000 });
  await page.waitForSelector('ul[aria-label="Hosted models"]', { timeout: 20_000 });
}

async function showExplorerTab(page: Page, tab: 'models' | 'model' | 'metamodel'): Promise<void> {
  await page.click(`#tab-${tab}`);
  await page.waitForSelector(`#panel-${tab}`);
}

/** The ids the Models tab lists, sorted. */
async function listedIds(page: Page): Promise<ModelId[]> {
  await showExplorerTab(page, 'models');
  const ids = await page.$$eval('ul[aria-label="Hosted models"] li[data-model-id]', (rows) =>
    rows.map((row) => row.getAttribute('data-model-id') ?? ''),
  );
  return ids.sort();
}

/** The ids of the open tabs, in strip order. */
function openTabIds(page: Page): Promise<ModelId[]> {
  return page.$$eval('.me-doctabs__tab[data-model-id]', (tabs) =>
    tabs.map((tab) => tab.getAttribute('data-model-id') ?? ''),
  );
}

/** Create a model through the New model form; the id of the tab that opens, which the node minted. */
async function createInUi(editor: Editor, metamodel: MetamodelId): Promise<ModelId> {
  const { page } = editor;
  await showExplorerTab(page, 'models');
  const before = new Set(await openTabIds(page));
  await page.select('select[aria-label="Metamodel for the new model"]', metamodel.digest);
  await page.click('form[aria-label="New model"] button[type="submit"]');
  return waitFor(`${editor.name}: the created model's tab`, async () => {
    const fresh = (await openTabIds(page)).filter((id) => !before.has(id));
    return fresh.length === 1 ? fresh[0] : null;
  });
}

/** Join a model through the Join by id form, and wait for its tab. */
async function joinInUi(editor: Editor, id: ModelId, metamodel: MetamodelId): Promise<void> {
  const { page } = editor;
  await showExplorerTab(page, 'models');
  const input = await page.$('input[aria-label="Model id"]');
  if (input === null) throw new Error('no Model id field');
  await input.click({ count: 3 });
  await input.type(id);
  await page.select('select[aria-label="Metamodel of the model to join"]', metamodel.digest);
  await page.click('form[aria-label="Join model by id"] button[type="submit"]');
  await page.waitForSelector(`.me-doctabs__tab[data-model-id="${id}"]`, { timeout: 20_000 });
}

async function selectTab(page: Page, id: ModelId): Promise<void> {
  await page.click(`.me-doctabs__tab[data-model-id="${id}"] .me-doctabs__name`);
  await page.waitForSelector(`.me-doctabs__tab--active[data-model-id="${id}"]`);
}

/** The top bar's context for the selected tab: the package and the binding chip. */
async function context(page: Page): Promise<{ id: string; pkg: string | null; binding: string | null; bindingTitle: string | null }> {
  return page.evaluate(() => {
    const text = (selector: string) => document.querySelector(selector)?.textContent?.trim() ?? null;
    return {
      id: text('[data-testid="model-id"]') ?? '',
      pkg: text('[data-testid="package"]'),
      binding: text('[data-testid="binding"]'),
      bindingTitle: document.querySelector('[data-testid="binding"]')?.getAttribute('title') ?? null,
    };
  });
}

/** Wait until the selected tab reads `bound` with `digest` in its title. */
async function waitForBound(editor: Editor, id: ModelId, digest: string): Promise<void> {
  await selectTab(editor.page, id);
  await waitFor(`${editor.name}: tab ${id.slice(0, 8)} bound to ${digest.slice(0, 8)}`, async () => {
    const ctx = await context(editor.page);
    return ctx.binding === 'bound' && (ctx.bindingTitle ?? '').includes(digest) ? true : null;
  });
}

/** What the selected tab renders, read from the console's Document JSON view. */
async function renderedDoc(page: Page): Promise<PlainJson> {
  if ((await page.$('#console-body')) === null) await page.click('button.me-console__bar');
  await page.click('#tab-json');
  const pre = await page.waitForSelector('pre[data-testid="raw-inspector"]', { timeout: 10_000 });
  if (pre === null) throw new Error('no document view');
  const text = await pre.evaluate((el) => el.textContent ?? '');
  return JSON.parse(text) as PlainJson;
}

/** Wait until the tab `id` renders exactly `expected` (keys sorted), and return what it rendered. */
async function waitForRendered(editor: Editor, id: ModelId, expected: PlainJson): Promise<PlainJson> {
  await selectTab(editor.page, id);
  const want = canonicalJson(sortKeys(expected));
  return waitFor(`${editor.name}: tab ${id.slice(0, 8)} to render the converged document`, async () => {
    const doc = await renderedDoc(editor.page);
    return canonicalJson(sortKeys(doc)) === want ? doc : null;
  });
}

/** The tree row titled `title` (an element's label, or "label — eClass"), naming the rows present when it is not there. */
async function treeRow(page: Page, title: string): Promise<ElementHandle<Element>> {
  try {
    const row = await page.waitForSelector(`[role="treeitem"][title="${title}"]`, { timeout: 20_000 });
    if (row !== null) return row;
  } catch {
    // Reported below with the rows that were there.
  }
  const titles = await page.$$eval('[role="treeitem"]', (rows) => rows.map((row) => row.getAttribute('title')));
  throw new Error(`no tree row titled "${title}"; the rows present: ${JSON.stringify(titles)}`);
}

/** The properties form's text input for the attribute `name`, once it is writable. */
async function attributeInput(page: Page, name: string): Promise<ElementHandle<HTMLInputElement>> {
  const selector = `::-p-xpath(//div[contains(@class,"me-field")][.//span[normalize-space(text())="${name}"]]//input)`;
  const input = (await page.waitForSelector(selector, { timeout: 20_000 })) as ElementHandle<HTMLInputElement> | null;
  if (input === null) throw new Error(`no input for ${name}`);
  await page.waitForFunction((el: HTMLInputElement) => !el.readOnly, { timeout: 20_000 }, input);
  return input;
}

/** Every eClass in the document, for the cross-talk assertion. */
function eClassesOf(doc: PlainJson, out = new Set<string>()): Set<string> {
  if (Array.isArray(doc)) doc.forEach((entry) => eClassesOf(entry, out));
  else if (doc !== null && typeof doc === 'object') {
    if (typeof doc['eClass'] === 'string') out.add(doc['eClass']);
    for (const value of Object.values(doc)) eClassesOf(value, out);
  }
  return out;
}

/** The store of the page's origin: file name to text, from `models` under the OPFS root. */
function storeFiles(page: Page): Promise<Record<string, string>> {
  return page.evaluate(async () => {
    const root = await navigator.storage.getDirectory();
    const dir = (await root.getDirectoryHandle('models')) as unknown as {
      keys(): AsyncIterable<string>;
      getFileHandle(name: string): Promise<{ getFile(): Promise<{ text(): Promise<string> }> }>;
    };
    const files: Record<string, string> = {};
    for await (const name of dir.keys()) {
      files[name] = await (await (await dir.getFileHandle(name)).getFile()).text();
    }
    return files;
  });
}

/* ---------- the scenario ---------- */

describe('mp26 in headless Chrome', () => {
  it('mp26_two_tabs_two_metamodels_two_replicas_converge_with_no_cross_talk', async (ctx) => {
    const skip = skipReason('mp26');
    if (skip !== null) return ctx.skip(skip);
    if (!existsSync(CHROME)) {
      const reason = `E2E-SKIP mp26: no Chrome at ${CHROME}; set CHROME_BIN`;
      console.warn(reason);
      return ctx.skip(reason);
    }

    const run = scratchDir('mp26');
    const teardown: Teardown[] = [];
    const editors: Editor[] = [];
    const cleanUp = async () => {
      for (const step of teardown.reverse()) {
        try {
          await step();
        } catch (err) {
          console.warn(`teardown: ${err instanceof Error ? err.message : String(err)}`);
        }
      }
      teardown.length = 0;
    };
    process.once('exit', () => {
      for (const step of teardown) void step();
    });

    try {
      // The editor, built and served.
      console.log('mp26: building the editor');
      await vite(['build', '--logLevel', 'warn'], join(run, 'vite-build.log'));
      const preview = await servePreview(join(run, 'vite-preview.log'));
      teardown.push(() => killProcess(preview.child));
      console.log(`mp26: editor served at ${preview.url}`);

      // Two nodes, peered, both holding bt and uml.
      const metamodels = writeMetamodelDir(join(run, 'metamodels'), {
        'bt.metamodel.json': bt,
        'uml.metamodel.json': uml,
      });
      const [a, b] = await startNodes(
        [
          { name: 'editor-a', metamodelDir: metamodels },
          { name: 'editor-b', metamodelDir: metamodels },
        ],
        run,
      );
      teardown.push(() => Promise.all([a.stop(), b.stop()]).then(() => undefined));
      console.log(`mp26: editor-a at ${a.url} (pid ${a.pid}), editor-b at ${b.url} (pid ${b.pid})`);

      // Chrome, one page per editor in its own context.
      const browser = await launchChrome(join(run, 'chrome-profile'));
      teardown.push(() => browser.close());
      const chromePid = browser.process()?.pid;
      console.log(`mp26: chrome pid ${chromePid}`);
      const A = await openEditor('page-a', a, browser, preview.url);
      const B = await openEditor('page-b', b, browser, preview.url);
      editors.push(A, B);
      await connect(A);
      await connect(B);

      // mp29: the lists start with the default log alone; nothing bound is hosted.
      expect(await listedIds(A.page)).toEqual([HARNESS_LOG_ID]);
      expect(await listedIds(B.page)).toEqual([HARNESS_LOG_ID]);
      expect(await hostedIds(a)).toEqual([HARNESS_LOG_ID]);
      expect(await openTabIds(A.page)).toEqual([]);

      // Create the behaviour-tree model on editor-a: the node mints the id.
      const btModel = await createInUi(A, btId);
      expect(btModel).toMatch(/^[0-9a-f]{32}$/);
      expect(await hostedIds(a)).toEqual([btModel, HARNESS_LOG_ID].sort());
      expect(await listedIds(A.page)).toEqual([btModel, HARNESS_LOG_ID].sort());
      expect(await hostedIds(b)).toEqual([HARNESS_LOG_ID]);
      await waitForBound(A, btModel, btId.digest);
      expect(await context(A.page)).toMatchObject({ id: `${btModel.slice(0, 8)}…`, pkg: 'behaviortree', binding: 'bound' });
      expect(modelHeaderOf(await modelState(a, btModel))).toEqual({ modelId: btModel, metamodelId: btId });
      console.log(`mp26: bt model ${btModel} created on editor-a, bound to ${btId.digest.slice(0, 8)}`);

      // Join it by id on editor-b: it appears in editor-b's list and opens bound.
      await joinInUi(B, btModel, btId);
      expect(await hostedIds(b)).toEqual([btModel, HARNESS_LOG_ID].sort());
      expect(await listedIds(B.page)).toEqual([btModel, HARNESS_LOG_ID].sort());
      await waitForBound(B, btModel, btId.digest);
      console.log(`mp26: bt model joined on editor-b and bound there`);

      // Create the SimpleUML model on editor-b, join it on editor-a.
      const umlModel = await createInUi(B, umlId);
      expect(umlModel).toMatch(/^[0-9a-f]{32}$/);
      expect(umlModel).not.toBe(btModel);
      await waitForBound(B, umlModel, umlId.digest);
      expect(await context(B.page)).toMatchObject({ pkg: 'simpleuml', binding: 'bound' });
      expect(await hostedIds(a)).toEqual([btModel, HARNESS_LOG_ID].sort());
      await joinInUi(A, umlModel, umlId);
      await waitForBound(A, umlModel, umlId.digest);
      expect(await hostedIds(a)).toEqual([btModel, umlModel, HARNESS_LOG_ID].sort());
      console.log(`mp26: uml model ${umlModel} created on editor-b, joined on editor-a`);

      // Four tabs, one per model per editor.
      expect((await openTabIds(A.page)).sort()).toEqual([btModel, umlModel].sort());
      expect((await openTabIds(B.page)).sort()).toEqual([btModel, umlModel].sort());

      // The models get their first elements through the API, the setup of
      // the scenario: a Root with one BehaviorTree whose child is the
      // Sequence to rename, and a Model to add the Class to.
      await applyOps(a, btModel, [
        ...createRootOps('Root'),
        ...addChildOps(['behaviortrees'], 0, 'BehaviorTree'),
        ...setStringOps(['behaviortrees', 0, 'ID'], '', 'main'),
        ...createSingleContainmentOps(['behaviortrees', 0], 'child', 'Sequence'),
        ...setStringOps(['behaviortrees', 0, 'child', 'name'], '', 'root'),
      ]);
      await applyOps(b, umlModel, [...createRootOps('Model'), ...setStringOps(['name'], '', 'm')]);
      const btSeed: PlainJson = {
        __model: { modelId: btModel, metamodelId: btId } as unknown as PlainJson,
        eClass: 'Root',
        behaviortrees: [{ eClass: 'BehaviorTree', ID: 'main', child: { eClass: 'Sequence', name: 'root' } }],
      };
      const umlSeed: PlainJson = { __model: { modelId: umlModel, metamodelId: umlId } as unknown as PlainJson, eClass: 'Model', name: 'm' };
      await waitForState([a, b], btModel, btSeed);
      await waitForState([a, b], umlModel, umlSeed);
      await waitForRendered(A, btModel, btSeed);
      await waitForRendered(B, umlModel, umlSeed);
      console.log('mp26: both models seeded and converged on both nodes');

      // Edit 1: rename the Sequence in editor-a's behaviour-tree tab, through the form.
      await selectTab(A.page, btModel);
      await showExplorerTab(A.page, 'model');
      // The row is titled by its class: TreeNode's id attribute is `ID`, which
      // the seed leaves empty, and `name` is what the scenario renames.
      const sequence = await treeRow(A.page, 'Sequence');
      await sequence.click();
      const nameA = await attributeInput(A.page, 'name');
      await nameA.click({ count: 3 });
      await nameA.type('patrol');
      await nameA.press('Enter');
      const btExpected: PlainJson = {
        ...(btSeed as Record<string, PlainJson>),
        behaviortrees: [{ eClass: 'BehaviorTree', ID: 'main', child: { eClass: 'Sequence', name: 'patrol' } }],
      };
      await waitForState([a, b], btModel, btExpected);
      console.log('mp26: Sequence renamed in page-a; both nodes agree');

      // Edit 2: add a Class named Door in editor-b's UML tab, through the tree and the form.
      await selectTab(B.page, umlModel);
      await showExplorerTab(B.page, 'model');
      const owned = await B.page.waitForSelector('[role="treeitem"][title="ownedElements"]', { timeout: 20_000 });
      if (owned === null) throw new Error('no ownedElements row');
      await owned.hover();
      await B.page.click('button[aria-label="Add to ownedElements"]');
      await B.page.click('::-p-xpath(//button[@role="menuitem"][normalize-space(text())="Class"])');
      const classRow = await treeRow(B.page, 'Class');
      await classRow.click();
      const nameB = await attributeInput(B.page, 'name');
      await nameB.click({ count: 3 });
      await nameB.type('Door');
      await nameB.press('Enter');
      const umlExpected: PlainJson = {
        ...(umlSeed as Record<string, PlainJson>),
        ownedElements: [{ eClass: 'Class', name: 'Door' }],
      };
      await waitForState([a, b], umlModel, umlExpected);
      console.log('mp26: Class Door added in page-b; both nodes agree');

      // Per-model convergence, as each tab renders it: four tabs, two documents.
      const rendered = {
        aBt: await waitForRendered(A, btModel, btExpected),
        bBt: await waitForRendered(B, btModel, btExpected),
        aUml: await waitForRendered(A, umlModel, umlExpected),
        bUml: await waitForRendered(B, umlModel, umlExpected),
      };
      expect(sortKeys(rendered.aBt)).toEqual(sortKeys(rendered.bBt));
      expect(sortKeys(rendered.aUml)).toEqual(sortKeys(rendered.bUml));
      expect(sortKeys(rendered.aBt)).toEqual(sortKeys(await modelState(a, btModel)));
      expect(sortKeys(rendered.bUml)).toEqual(sortKeys(await modelState(b, umlModel)));
      console.log('mp26: each model\'s two tabs render the same document, the one both nodes serve');

      // No cross-talk: the behaviour-tree document holds no Class and no UML key,
      // the UML document no Sequence and no behaviour-tree key, on both nodes.
      for (const node of [a, b]) {
        const btDoc = await modelState(node, btModel);
        const umlDoc = await modelState(node, umlModel);
        expect([...eClassesOf(btDoc)].sort()).toEqual(['BehaviorTree', 'Root', 'Sequence']);
        expect([...eClassesOf(umlDoc)].sort()).toEqual(['Class', 'Model']);
        expect(JSON.stringify(btDoc)).not.toContain('ownedElements');
        expect(JSON.stringify(umlDoc)).not.toContain('behaviortrees');
        expect(modelHeaderOf(btDoc)).toEqual({ modelId: btModel, metamodelId: btId });
        expect(modelHeaderOf(umlDoc)).toEqual({ modelId: umlModel, metamodelId: umlId });
      }
      console.log('mp26: no cross-talk on either node');

      // Each tab was served the descriptor its model's header names.
      for (const editor of [A, B]) {
        await selectTab(editor.page, btModel);
        expect(await context(editor.page)).toMatchObject({ pkg: 'behaviortree', binding: 'bound' });
        expect((await context(editor.page)).bindingTitle).toContain(btId.digest);
        await selectTab(editor.page, umlModel);
        expect(await context(editor.page)).toMatchObject({ pkg: 'simpleuml', binding: 'bound' });
        expect((await context(editor.page)).bindingTitle).toContain(umlId.digest);
      }
      console.log('mp26: every tab bound to the descriptor its header names');

      // The store: exactly one file per model in each browser context, each
      // the canonical JSON of that model's converged state.
      for (const editor of [A, B]) {
        const files = await waitFor(`${editor.name}: the store to hold both models`, async () => {
          const found = await storeFiles(editor.page);
          return found[`${btModel}.json`] === canonicalJson(btExpected) && found[`${umlModel}.json`] === canonicalJson(umlExpected)
            ? found
            : null;
        });
        expect(Object.keys(files).sort()).toEqual([`${btModel}.json`, `${umlModel}.json`].sort());
        expect(JSON.parse(files[`${btModel}.json`])).toEqual(await modelState(editor.node, btModel));
        expect(JSON.parse(files[`${umlModel}.json`])).toEqual(await modelState(editor.node, umlModel));
      }
      console.log('mp26: each context\'s store holds one file per model, equal to the converged state');

      await cleanUp();
      expect(a.exited && b.exited).toBe(true);
      if (chromePid !== undefined) {
        expect(() => process.kill(chromePid, 0)).toThrow();
      }
      console.log('mp26: nodes, chrome and the preview server are down');
    } catch (err) {
      for (const editor of editors) {
        console.error(`--- ${editor.name} console ---\n${editor.console.slice(-40).join('\n')}`);
        console.error(`--- ${editor.node.name} log ---\n${editor.node.logTail(30)}`);
      }
      throw err;
    } finally {
      await cleanUp();
      rmSync(run, { recursive: true, force: true });
    }
  });
});

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
 * **Bob's join is made in bob's own window, through the Join by id form**,
 * and that is the point of the scenario rather than a detail of how it is
 * driven. The form's dropdown is fed by bob's own `GET /api/metamodels`, so
 * it cannot offer a language bob has never held; the digest field beside it
 * can name one, and this asserts both — that the dropdown does not offer the
 * digest, and that the join goes through anyway.
 *
 * Then the model is edited in bob's window and the edit shows up in alice's,
 * which is the other half of the claim: bob did not merely receive a
 * language, bob can write the model under it — in the tab the join opened,
 * with nothing closed and nothing reopened. The route that types that tab
 * answers 404 at the moment of the join and starts answering once the
 * language has travelled, and the session asks again while it has no
 * descriptor, so the scenario records how long after the join the tab fills
 * in rather than asserting a number.
 *
 * Then the route that is easy and wrong. bob names a *second* model in the
 * same language from bob's own dropdown — a language bob holds and the model
 * is not written in, which is the mistake the demo rehearsal made on
 * 2026-09-14. The form asks before it sends, and prints the digest it is
 * about to bind so it can be read against the one you were given; gone
 * through with anyway, the node hosts the model, converges on it, and then
 * answers 404 on its metamodel route rather than the descriptor of a
 * language the model is not written in.
 *
 * A last step is the refusal: a file the node will not serve comes back 422
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
import type { Descriptor, MetamodelListing, ModelId, PlainJson } from '../src/api/types';
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

/** Select the root element, which is what puts its features in the properties form. */
async function selectRootInUi(page: Page): Promise<void> {
  await showExplorerTab(page, 'model');
  const root = await page.waitForSelector('[role="treeitem"]', { timeout: 30_000 });
  if (root === null) throw new Error('no root element in the tree');
  await root.click();
}

/** Type `value` into the properties form's input for the attribute `name`, on whatever is selected. */
async function setAttributeInUi(page: Page, attribute: string, value: string): Promise<void> {
  const selector = `::-p-xpath(//div[contains(@class,"me-field")][.//span[normalize-space(text())="${attribute}"]]//input)`;
  const input = await page.waitForSelector(selector, { timeout: 30_000 });
  if (input === null) throw new Error(`no input for ${attribute}`);
  await page.waitForFunction((el: Element) => !(el as HTMLInputElement).readOnly, { timeout: 30_000 }, input);
  await input.click({ count: 3 });
  await input.type(value);
  await page.keyboard.press('Tab');
}

/** The containment card for `feature` on the selected element. */
function cardXPath(feature: string): string {
  return `//div[contains(@class,"me-block")][.//span[contains(@class,"me-block__name")][normalize-space(text())="${feature}"]]`;
}

/**
 * Click a containment card's Add control.
 *
 * The control is held while a structural batch is in flight (ui/editGate.ts),
 * so this waits for it to be enabled rather than clicking a dead button.
 */
async function addChildInUi(page: Page, feature: string, className: string): Promise<void> {
  const selector = `::-p-xpath(${cardXPath(feature)}//button[normalize-space(.)="Add ${className}"])`;
  const button = await page.waitForSelector(selector, { timeout: 30_000 });
  if (button === null) throw new Error(`no Add ${className} control on ${feature}`);
  await page.waitForFunction((el: Element) => !(el as HTMLButtonElement).disabled, { timeout: 30_000 }, button);
  await button.click();
}

/** Select the child at `index` of a containment card, which opens its own features. */
async function selectChildInUi(page: Page, feature: string, index: number): Promise<void> {
  const selector = `::-p-xpath(${cardXPath(feature)}//button[contains(@class,"me-block__link")])`;
  await page.waitForSelector(selector, { timeout: 30_000 });
  const links = await page.$$(selector);
  if (links.length <= index) throw new Error(`${feature} has ${links.length} children, wanted index ${index}`);
  await links[index].click();
}

/** The digests the Join by id dropdown offers right now: what this replica already serves. */
function offeredJoinDigests(page: Page): Promise<string[]> {
  return page.$$eval('select[aria-label="Metamodel of the model to join"] option', (options) =>
    options.map((option) => (option as HTMLOptionElement).value),
  );
}

/**
 * Join a model through the Join by id form, naming its metamodel by a digest
 * typed into the form rather than chosen from the dropdown.
 *
 * This is the control the scenario exists for. Typing into the digest field
 * puts it in force, which is asserted here: a join that quietly went out
 * under whatever the dropdown happened to show would pass every later
 * assertion on a replica that holds the language and prove nothing on one
 * that does not.
 */
async function joinByDigestInUi(page: Page, id: ModelId, digest: string): Promise<void> {
  await showExplorerTab(page, 'models');
  const form = 'form[aria-label="Join model by id"]';
  const idField = await page.waitForSelector(`${form} input[aria-label="Model id"]`);
  if (idField === null) throw new Error('no Model id field');
  await idField.click({ count: 3 });
  await idField.type(id);
  const digestField = await page.waitForSelector(`${form} input[aria-label="Metamodel digest"]`);
  if (digestField === null) throw new Error('no Metamodel digest field');
  await digestField.click({ count: 3 });
  await digestField.type(digest);
  const inForce = await page.$eval(
    `${form} input[name="join-bound-to"][value="digest"]`,
    (el) => (el as HTMLInputElement).checked,
  );
  if (!inForce) throw new Error('typing a digest did not put the digest field in force');
  await page.click(`${form} button[type="submit"]`);
}

/** The question the form asks about a join it cannot check. */
const UNCHECKED_PROMPT = '[aria-label="Confirm a join this replica cannot check"]';

/**
 * Join a model through the Join by id form's **dropdown**: the route that is
 * available to everyone and right for nobody when the model is written in a
 * language this replica has no way to recognise.
 *
 * Returns the question the form asked, or `null` if it sent the join without
 * asking — which is the regression this exists to catch. Nothing is confirmed
 * here; the caller decides whether to go through with it.
 */
async function dropdownJoinInUi(page: Page, id: ModelId, digest: string): Promise<string | null> {
  await showExplorerTab(page, 'models');
  const form = 'form[aria-label="Join model by id"]';
  const idField = await page.waitForSelector(`${form} input[aria-label="Model id"]`);
  if (idField === null) throw new Error('no Model id field');
  await idField.click({ count: 3 });
  await idField.type(id);
  await page.select(`${form} select[aria-label="Metamodel of the model to join"]`, digest);
  const inForce = await page.$eval(
    `${form} input[name="join-bound-to"][value="held"]`,
    (el) => (el as HTMLInputElement).checked,
  );
  if (!inForce) throw new Error('choosing from the dropdown did not put the dropdown in force');
  await page.click(`${form} button[type="submit"]`);
  const prompt = await page.waitForSelector(UNCHECKED_PROMPT, { timeout: 5_000 }).catch(() => null);
  if (prompt === null) return null;
  return (await prompt.evaluate((el) => el.textContent)) ?? '';
}

/** Go through with a join the form asked about. */
async function confirmUncheckedJoinInUi(page: Page): Promise<void> {
  await page.evaluate((selector: string) => {
    const buttons = Array.from(document.querySelectorAll(`${selector} button`));
    const confirm = buttons.find((button) => (button.textContent ?? '').startsWith('Join as'));
    if (confirm === undefined) throw new Error('no confirm button on the unchecked-join prompt');
    (confirm as HTMLButtonElement).click();
  }, UNCHECKED_PROMPT);
}

/**
 * How long after `since` the model tab becomes typed, in milliseconds.
 *
 * The tree draws a row per element and it is built from the descriptor, so a
 * `treeitem` is the first thing in the DOM that can only exist once the
 * language is in effect in this window. On a model joined under a digest this
 * replica did not hold, that is the moment the session's re-ask found the
 * descriptor the node adopted out of the model's own history.
 */
async function msUntilTyped(page: Page, since: number): Promise<number> {
  await showExplorerTab(page, 'model');
  return waitFor(
    'the joining tab to be typed by the language that arrived with the model',
    async () => ((await page.$('[role="treeitem"]')) === null ? null : Date.now() - since),
    { timeoutMs: 60_000, intervalMs: 50 },
  );
}

/* ---------- what to write, read out of the language itself ---------- */

/** A containment to fill, and the text to give each child. */
interface ChildPlan {
  feature: string;
  className: string;
  attribute: string;
  titles: string[];
}

/** The attributes of `className` that hold one string each. */
function textAttributes(descriptor: Descriptor, className: string): string[] {
  return (descriptor.classes[className]?.attributes ?? [])
    .filter((attribute) => attribute.kind === 'string' && !attribute.many)
    .map((attribute) => attribute.name);
}

/**
 * What this scenario writes, derived from the descriptor rather than named in
 * the file.
 *
 * The language is whatever `MOIRAI_DEMO_DESCRIPTOR` points at, and the
 * scenario has to fill it without knowing its vocabulary: the root's own
 * text, and a many-valued containment whose target is a concrete class with
 * text of its own. Where several qualify it takes the one whose target is
 * described by a single string, because a child that wants two and is given
 * one leaves a half-written element in the document this scenario prints —
 * `Library.books` of `Book.title` rather than `Library.writers` of a
 * `Writer` whose `lastName` would stay empty. A language offering no such
 * containment is edited at the root alone and the sequence half is skipped
 * rather than failed: it is the descriptor that has nothing to say, not the
 * editor.
 */
function editPlan(descriptor: Descriptor): {
  rootClass: string;
  rootAttribute: string;
  child: ChildPlan | null;
} {
  const rootClass = descriptor.rootClasses[0];
  const fillable = (descriptor.classes[rootClass]?.containments ?? []).filter((feature) => {
    const target = descriptor.classes[feature.target];
    return feature.many && target !== undefined && !target.abstract && textAttributes(descriptor, feature.target).length > 0;
  });
  const containment = fillable.find((feature) => textAttributes(descriptor, feature.target).length === 1) ?? fillable[0];
  return {
    rootClass,
    rootAttribute: textAttributes(descriptor, rootClass)[0] ?? 'name',
    child:
      containment === undefined
        ? null
        : {
            feature: containment.name,
            className: containment.target,
            attribute: textAttributes(descriptor, containment.target)[0],
            titles: ['The Little Prince', 'A Wizard of Earthsea'],
          },
  };
}

/* ---------- the nodes, read straight ---------- */

async function stateOf(url: string, id: ModelId): Promise<PlainJson> {
  const response = await fetch(`${url}/api/model/${id}/state`);
  if (!response.ok) throw new Error(`${url} /api/model/${id}/state: ${response.status}`);
  return (await response.json()) as PlainJson;
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
      const demoDescriptor = JSON.parse(text) as Descriptor;
      const nsURI = demoDescriptor.nsURI;
      const { rootClass, rootAttribute, child } = editPlan(demoDescriptor);
      console.log(
        `add-metamodel: the language is ${nsURI}, root ${rootClass}.${rootAttribute}` +
          (child === null ? ', with no sequence to fill' : `, sequence ${rootClass}.${child.feature} of ${child.className}.${child.attribute}`),
      );

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
      await selectRootInUi(alice);
      await setAttributeInUi(alice, rootAttribute, 'Door');
      await waitFor(`alice's ${modelId.slice(0, 8)}… to carry the edit`, async () =>
        mentions(await stateOf(aliceUrl, modelId), 'Door') ? true : null,
      );

      /*
       * The ordered containment, filled from the editor: two children, each
       * given its own text, so what bob receives is a sequence and not one
       * scalar. Each write is waited for on the node before the next control
       * is touched, because the add control computes an index from the
       * client's view of the document (ui/editGate.ts).
       */
      if (child !== null) {
        for (const [index, title] of child.titles.entries()) {
          await selectRootInUi(alice);
          await addChildInUi(alice, child.feature, child.className);
          await waitFor(`alice's ${child.feature} to carry ${index + 1} ${child.className}`, async () => {
            const state = await stateOf(aliceUrl, modelId);
            const list = (state as Record<string, PlainJson>)[child.feature];
            return Array.isArray(list) && list.length === index + 1 ? true : null;
          });
          await selectChildInUi(alice, child.feature, index);
          await setAttributeInUi(alice, child.attribute, title);
          await waitFor(`alice's ${modelId.slice(0, 8)}… to carry ${title}`, async () =>
            mentions(await stateOf(aliceUrl, modelId), title) ? true : null,
          );
        }
        console.log(`add-metamodel: alice's document is ${JSON.stringify(await stateOf(aliceUrl, modelId))}`);
      }

      /* 3. Bob, who was never given the descriptor, joins by id from bob's own window. */
      const bobBefore = await getMetamodels(bobUrl);
      expect(bobBefore.map((e) => e.nsURI)).not.toContain(nsURI);
      const bob = await openEditor(browser, preview.url, bobUrl);
      await showExplorerTab(bob, 'models');
      // The dropdown cannot express this join, which is why the field beside
      // it exists: assert that before using it, or the next line proves
      // nothing.
      expect(
        await offeredJoinDigests(bob),
        "bob's Join dropdown must not offer a language bob has never held",
      ).not.toContain(language.digest);
      const joinedAt = Date.now();
      await joinByDigestInUi(bob, modelId, language.digest);
      await waitFor(`bob's tab for ${modelId.slice(0, 8)}…`, async () =>
        (await openTabIds(bob)).includes(modelId) ? true : null,
      );
      // What the session that just opened could have been given: a 404 while
      // the binding is pending, which is what makes the re-ask the whole
      // difference between a tab that types itself and one that has to be
      // closed and opened again.
      const atJoin = await fetch(`${bobUrl}/api/model/${modelId}/metamodel`);
      console.log(`add-metamodel: bob's descriptor for the model, when the joining tab opened: HTTP ${atJoin.status}`);
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
      const adoptedAfterMs = Date.now() - joinedAt;
      console.log(
        `add-metamodel: bob now lists ${bobAfter.map((e) => e.package).join(', ')}, ${adoptedAfterMs} ms after the join`,
      );

      /* 4. The joining tab types itself where it stands, and bob writes the model. */
      const typedAfterMs = await msUntilTyped(bob, joinedAt);
      // Two numbers rather than one: what the node took to adopt the
      // language out of the model's history, and what the tab took on top of
      // that, which is at most one poll of the editor's own clock.
      console.log(
        `add-metamodel: bob's tab filled in ${typedAfterMs} ms after the join, ${
          typedAfterMs - adoptedAfterMs
        } ms after the node adopted the language, with nothing closed or reopened`,
      );
      // The tab is the one the join opened and no other, which is the claim:
      // a second session would have read the descriptor whatever the first
      // one did.
      expect(await openTabIds(bob)).toContain(modelId);
      await selectRootInUi(bob);
      await setAttributeInUi(bob, rootAttribute, 'Door, edited on bob');
      const backOnAlice = await waitFor("bob's edit to reach alice", async () =>
        mentions(await stateOf(aliceUrl, modelId), 'edited on bob') ? true : null,
      );
      expect(backOnAlice).toBe(true);
      console.log(`add-metamodel: alice's document after bob's edit is ${JSON.stringify(await stateOf(aliceUrl, modelId))}`);

      /*
       * 5. The route that is easy and wrong: the dropdown, on a model written
       *    in a language it cannot name.
       *
       * This is what the demo rehearsal did on 2026-09-14. bob's dropdown is
       * bob's own listing, so on a model it has never held the language for
       * there is nothing in it that is right, and picking one anyway used to
       * go straight through: the node holds the language that was named, so
       * the binding was taken without a word and bob served that language's
       * descriptor for a document not written in it. Both halves are checked
       * here — the form asks before it sends, and the node refuses to answer
       * for a model whose binding its own history has contradicted.
       */
      const secondId = await createInUi(alice, language.digest);
      await createRootInUi(alice, rootClass);
      await selectRootInUi(alice);
      await setAttributeInUi(alice, rootAttribute, 'Second');
      await waitFor(`alice's ${secondId.slice(0, 8)}… to carry its root`, async () =>
        mentions(await stateOf(aliceUrl, secondId), 'Second') ? true : null,
      );
      // A language bob holds and the model is not written in. bob holds the
      // real one too by now, having adopted it in step 3, and that changes
      // nothing about the defect: what is wrong is the binding, not what the
      // replica happens to serve.
      const wrong = (await getMetamodels(bobUrl)).find((candidate) => candidate.digest !== language.digest);
      expect(wrong, 'bob must hold some other language to bind this model wrongly to').toBeDefined();
      const wrongLanguage = wrong as MetamodelListing;
      const asked = await dropdownJoinInUi(bob, secondId, wrongLanguage.digest);
      expect(asked, 'a join named from this replica’s own listing must be asked about, not sent').not.toBeNull();
      expect(asked).toContain(wrongLanguage.digest);
      expect(await openTabIds(bob), 'the form joined before the question was answered').not.toContain(secondId);
      console.log(
        `add-metamodel: the form asked, and sent nothing: ${(asked ?? '').replace(/\s+/g, ' ').trim()}`,
      );

      // Gone through with anyway, which is the path the rehearsal took and
      // the one that had no way back.
      await confirmUncheckedJoinInUi(bob);
      await waitFor(`bob to host ${secondId.slice(0, 8)}…`, async () => {
        const listed = await fetch(`${bobUrl}/api/models`).then((r) => r.json() as Promise<PlainJson>);
        const models = (listed as { models?: { model_id: string }[] }).models ?? [];
        return models.some((model) => model.model_id === secondId) ? true : null;
      });
      const bound = await waitFor(
        'bob to stop answering for a model it cannot say the language of',
        async () => {
          const answer = await fetch(`${bobUrl}/api/model/${secondId}/metamodel`);
          return answer.status === 404 ? 404 : null;
        },
        { timeoutMs: 60_000, intervalMs: 100 },
      );
      expect(bound).toBe(404);
      const misbound = await waitFor(`bob to converge on ${secondId.slice(0, 8)}…`, async () => {
        const state = await stateOf(bobUrl, secondId).catch(() => null);
        return state !== null && mentions(state, 'Second') ? state : null;
      });
      console.log(
        `add-metamodel: bob was made to bind ${secondId} to ${wrongLanguage.package}; its document there is ` +
          `${JSON.stringify(misbound)} and GET /api/model/${secondId}/metamodel answers 404 rather than ` +
          `${wrongLanguage.package}`,
      );

      /* 6. A file this node will not serve: the node's own sentence, in the UI. */
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

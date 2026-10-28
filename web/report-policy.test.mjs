import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const source = await readFile(new URL('./report-policy.js', import.meta.url), 'utf8');
const { reportsEndpoint } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
for (const href of [
  'https://eqoaclassic.com/eqoa/',
  'https://eqoaclassic.com/eqoa/?reports=/reports',
  'http://127.0.0.1:8080/',
  'http://127.0.0.1:8080/?reports=https://eqoaclassic.com/eqoa/reports',
  'http://127.0.0.1:8080/?reports=//example.com/reports',
  'http://127.0.0.1:8080/?reports=http://127.0.0.1:8081/reports',
  'http://127.0.0.1:8080/?reports=/reports%3Fnext=evil',
  'http://127.0.0.1.evil:8080/?reports=/reports',
]) assert.equal(reportsEndpoint(href), null, href);
assert.equal(reportsEndpoint('http://127.0.0.1:8080/?reports=/reports'), 'http://127.0.0.1:8080/reports');
assert.equal(reportsEndpoint('http://[::1]:8080/?reports=/reports/'), 'http://[::1]:8080/reports');
console.log('ok: remote/default destinations denied; explicit loopback fixture is the control');
import vm from 'node:vm';
const reportSource = (await readFile(new URL('./report.js', import.meta.url), 'utf8'))
  .replace("import { reportsEndpoint } from './report-policy.js';", '')
  .replaceAll('import.meta.url', JSON.stringify('http://127.0.0.1:8080/report.js'));

function page(href, previous = false) {
  const nodes = new Map();
  const listeners = new Map();
  const storage = new Map();
  const requests = [];
  class Element {
    constructor(tag, id) { this.tagName = tag.toUpperCase(); this.children = []; this.dataset = {}; this.hidden = false; this.value = ''; this.textContent = ''; if (id) { this.id = id; nodes.set(id, this); } }
    append(...kids) { this.children.push(...kids); for (const k of kids) if (k?.id) nodes.set(k.id, k); }
    replaceChildren(...kids) { this.children = []; this.append(...kids); }
    addEventListener(name, callback) { this[name] = callback; }
    focus() {}
    remove() {}
    set innerHTML(html) { for (const [, id] of html.matchAll(/id="([^"]+)"/g)) new Element('div', id); }
  }
  new Element('button', 'report-gate');
  new Element('div', 'notices');
  if (previous) storage.set('eqoa.session', JSON.stringify({ when: Date.now(), hidden: false, log: ['kept local'], state: 'kept state' }));
  const location = new URL(href);
  const document = {
    readyState: 'complete', visibilityState: 'visible', body: new Element('body'),
    getElementById: id => nodes.get(id) || null,
    createElement: tag => new Element(tag),
    addEventListener: () => {},
  };
  const context = vm.createContext({
    URL, URLSearchParams, reportsEndpoint, document, location,
    navigator: { userAgent: 'Test browser' }, innerWidth: 800, innerHeight: 600, devicePixelRatio: 1,
    console: { log() {}, info() {}, warn() {}, error() {} },
    localStorage: { getItem: k => storage.get(k), setItem: (k, v) => storage.set(k, v), removeItem: k => storage.delete(k) },
    addEventListener: (event, callback) => { const list = listeners.get(event) || []; list.push(callback); listeners.set(event, list); },
    setInterval() {}, setTimeout() {}, clearTimeout() {},
    fetch: async (url, options) => {
      if (options?.method === 'POST') requests.push({ url, options });
      return { ok: true, json: async () => options ? { ok: true, id: 'fixture' } : { build: 'test' } };
    },
  });
  context.window = context;
  vm.runInContext(reportSource, context);
  return { context, nodes, listeners, storage, requests };
}

const disabled = page('https://fork.example/eqoa/');
assert.equal(disabled.context.eqoaReport.enabled, false);
assert.equal(disabled.nodes.get('report-gate').hidden, true);
assert.equal(disabled.nodes.get('report-gate').disabled, true);
await disabled.context.eqoaReportOpen();
disabled.context.eqoaProblem(['fixture problem', 'diagnostic notice']);
assert.equal(disabled.nodes.get('notices').children.flatMap(n => n.children).some(e => /Send/.test(e?.textContent)), false);
let swallowed = false;
for (const callback of disabled.listeners.get('keydown')) callback({ key: 'F3', preventDefault() { swallowed = true; }, stopImmediatePropagation() {} });
assert.equal(swallowed, false);
assert.equal(disabled.nodes.has('report'), false);
await assert.rejects(vm.runInContext("post('report', {})", disabled.context), /disabled/);
disabled.context.eqoaSecret('account', 'password');
disabled.context.eqoaCrash(['The client stopped', 'panic account password']);
assert.equal(disabled.context.eqoaReport.dead, true);
assert.match(disabled.context.eqoaReport.diagnostics(), /panic <account> <password>/);
assert.match(disabled.storage.get('eqoa.enhanced.crash'), /panic <account> <password>/);
assert.equal(disabled.nodes.has('report'), false);
assert.equal(disabled.requests.length, 0);

const previous = page('https://fork.example/eqoa/', true);
assert.equal(previous.nodes.get('notices').children.flatMap(n => n.children).some(e => /Send/.test(e?.textContent)), false);
assert.equal(previous.storage.has('eqoa.session'), true);
assert.equal(previous.requests.length, 0);

const fixture = page('http://127.0.0.1:8080/?reports=/reports');
assert.equal(fixture.context.eqoaReport.enabled, true);
assert.equal(fixture.nodes.get('report-gate').disabled, undefined);
await fixture.context.eqoaReportOpen();
assert.equal(fixture.nodes.get('report').hidden, false);
fixture.nodes.get('rp-note').value = 'fixture control';
await vm.runInContext('send()', fixture.context);
assert.equal(fixture.requests.length, 1);
assert.equal(fixture.requests[0].url, 'http://127.0.0.1:8080/reports/report');
assert.equal(fixture.requests[0].options.redirect, 'error');
assert.equal(fixture.requests[0].options.credentials, 'omit');
console.log('ok: default F3/API/crash/previous-session cannot upload; diagnostics stay local; fixture Send is the control');

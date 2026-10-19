// scripts/dashboard-js-check.mjs
// Headless node vm checks for dashboard JS (vocabulary, formatters, components)

import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');

class CustomEvent {
  constructor(type, init = {}) {
    this.type = type;
    this.detail = init.detail;
  }
}

class EventSource {
  static instances = [];
  constructor(url) {
    this.url = url;
    this.handlers = {};
    EventSource.instances.push(this);
  }
  addEventListener(event, handler) {
    if (!this.handlers[event]) this.handlers[event] = [];
    this.handlers[event].push(handler);
  }
  close() {}
}

function makeContext() {
  const ctx = {
    console,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    Promise,
    Intl,
    document: {
      addEventListener() {},
      getElementById() {
        return null;
      },
      body: {
        classList: {
          add() {},
          remove() {},
        },
      },
      head: {
        appendChild() {},
      },
      createElement() {
        return {};
      },
      activeElement: null,
    },
    localStorage: {
      getItem() {
        return null;
      },
      setItem() {},
    },
    CustomEvent,
    EventSource,
    location: { hash: '' },
    addEventListener() {},
    dispatchEvent() {
      return true;
    },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  const appJsPath = path.join(repoRoot, 'agent-brain/assets/dashboard/js/app.js');
  const appJsSrc = fs.readFileSync(appJsPath, 'utf8');
  vm.runInContext(appJsSrc, ctx, { filename: 'app.js' });
  return ctx;
}

let checkCount = 0;
const failures = [];

function check(name, ok, detail) {
  checkCount++;
  if (!ok) {
    failures.push({ name, detail: detail || 'assertion failed' });
  }
}

const sections = [
  {
    name: 'vocab',
    async run(ctx) {
      const AB = ctx.AB;
      const stepClass = AB.stepClass('dang_chay');
      check(
        'stepClass dang_chay has bg-blue-100',
        stepClass.includes('bg-blue-100'),
        `got "${stepClass}"`
      );
      check(
        'stepClass dang_chay has dark:bg-blue-900/50',
        stepClass.includes('dark:bg-blue-900/50'),
        `got "${stepClass}"`
      );
      check(
        'stepClass dang_chay has no indigo',
        !stepClass.includes('indigo'),
        `got "${stepClass}"`
      );

      const expected = {
        xong: {
          codePoint: 0x2713,
          text: 'text-emerald-700 dark:text-emerald-300',
          card: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
          word: 'xong',
        },
        dat: {
          codePoint: 0x2713,
          text: 'text-emerald-700 dark:text-emerald-300',
          card: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
          word: 'đạt',
        },
        dang_chay: {
          codePoint: 0x25D0,
          text: 'text-blue-700 dark:text-blue-300',
          card: 'border-blue-300 bg-blue-50 dark:border-blue-700 dark:bg-blue-900/20',
          word: 'đang chạy',
        },
        cho_ban: {
          codePoint: 0x25CF,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'chờ bạn',
        },
        vuong_mac: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'vướng mắc',
        },
        can_sua: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'cần sửa',
        },
        killed: {
          codePoint: 0x25A0,
          text: 'text-rose-700 dark:text-rose-300',
          card: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
          word: 'đã dừng',
        },
        loi: {
          codePoint: 0x25A0,
          text: 'text-rose-700 dark:text-rose-300',
          card: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
          word: 'lỗi',
        },
        vi_pham: {
          codePoint: 0x25A0,
          text: 'text-rose-700 dark:text-rose-300',
          card: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
          word: 'vi phạm giao thức',
        },
        cho: {
          codePoint: 0x25CC,
          text: 'text-slate-600 dark:text-slate-400',
          card: 'border-dashed border-slate-500 bg-white dark:border-slate-400 dark:bg-slate-800',
          word: 'chưa bắt đầu',
        },
        chua_tu_dong: {
          codePoint: 0x25CC,
          text: 'text-slate-600 dark:text-slate-400',
          card: 'border-dashed border-slate-400 bg-transparent dark:border-slate-500 dark:bg-transparent',
          word: 'chưa tự động',
        },
      };

      const states = Object.keys(expected);
      for (const st of states) {
        const glyph = AB.stepGlyph(st);
        const cp = glyph ? glyph.codePointAt(0) : 0;
        check(
          `stepGlyph(${st}) code point`,
          cp === expected[st].codePoint,
          `expected 0x${expected[st].codePoint.toString(16)}, got 0x${cp.toString(16)}`
        );
        check(
          `stepGlyph(${st}) ends with U+FE0E`,
          glyph.endsWith('\uFE0E'),
          `glyph is "${glyph}"`
        );
        check(
          `stepGlyph(${st}) does not start with U+2715 or U+25CB`,
          cp !== 0x2715 && cp !== 0x25CB,
          `got 0x${cp.toString(16)}`
        );

        const text = AB.stateText(st);
        check(
          `stateText(${st})`,
          text === expected[st].text,
          `expected "${expected[st].text}", got "${text}"`
        );

        const card = AB.stateCard(st);
        check(
          `stateCard(${st})`,
          card === expected[st].card,
          `expected "${expected[st].card}", got "${card}"`
        );

        const word = AB.stateWord(st);
        check(
          `stateWord(${st})`,
          word === expected[st].word,
          `expected "${expected[st].word}", got "${word}"`
        );
      }

      check(
        'stateCard cho !== chua_tu_dong',
        AB.stateCard('cho') !== AB.stateCard('chua_tu_dong'),
        'both card styles are equal'
      );
      check(
        'stateCard cho has border-dashed',
        AB.stateCard('cho').includes('border-dashed'),
        AB.stateCard('cho')
      );
      check(
        'stateCard chua_tu_dong has border-dashed',
        AB.stateCard('chua_tu_dong').includes('border-dashed'),
        AB.stateCard('chua_tu_dong')
      );

      const unknown = 'khong-co';
      check(
        'stateText unknown falls back to cho',
        AB.stateText(unknown) === expected.cho.text,
        `got "${AB.stateText(unknown)}"`
      );
      check(
        'stateCard unknown falls back to cho',
        AB.stateCard(unknown) === expected.cho.card,
        `got "${AB.stateCard(unknown)}"`
      );
      check(
        'stateWord unknown falls back to cho',
        AB.stateWord(unknown) === expected.cho.word,
        `got "${AB.stateWord(unknown)}"`
      );
    },
  },
  {
    name: 'formatters',
    async run(ctx) {
      const AB = ctx.AB;

      // fmtMinutes: null, undefined, NaN → `—`; 0 → `0 phút`; 29999 → `< 1 phút`; 30000 → `1 phút`; 89999 → `1 phút`; 420000 → `7 phút`; a negative value → `0 phút`
      check('fmtMinutes(null)', AB.fmtMinutes(null) === '—', `got "${AB.fmtMinutes(null)}"`);
      check('fmtMinutes(undefined)', AB.fmtMinutes(undefined) === '—', `got "${AB.fmtMinutes(undefined)}"`);
      check('fmtMinutes(NaN)', AB.fmtMinutes(NaN) === '—', `got "${AB.fmtMinutes(NaN)}"`);
      check('fmtMinutes(0)', AB.fmtMinutes(0) === '0 phút', `got "${AB.fmtMinutes(0)}"`);
      check('fmtMinutes(29999)', AB.fmtMinutes(29999) === '< 1 phút', `got "${AB.fmtMinutes(29999)}"`);
      check('fmtMinutes(30000)', AB.fmtMinutes(30000) === '1 phút', `got "${AB.fmtMinutes(30000)}"`);
      check('fmtMinutes(89999)', AB.fmtMinutes(89999) === '1 phút', `got "${AB.fmtMinutes(89999)}"`);
      check('fmtMinutes(420000)', AB.fmtMinutes(420000) === '7 phút', `got "${AB.fmtMinutes(420000)}"`);
      check('fmtMinutes(-5000)', AB.fmtMinutes(-5000) === '0 phút', `got "${AB.fmtMinutes(-5000)}"`);

      // fmtUsd: null, undefined, NaN → `—`; 0 → `$0.00`; 0.004 → `< $0.01`; 2.734 → `$2.73`; 17.62 → `$17.62`
      check('fmtUsd(null)', AB.fmtUsd(null) === '—', `got "${AB.fmtUsd(null)}"`);
      check('fmtUsd(undefined)', AB.fmtUsd(undefined) === '—', `got "${AB.fmtUsd(undefined)}"`);
      check('fmtUsd(NaN)', AB.fmtUsd(NaN) === '—', `got "${AB.fmtUsd(NaN)}"`);
      check('fmtUsd(0)', AB.fmtUsd(0) === '$0.00', `got "${AB.fmtUsd(0)}"`);
      check('fmtUsd(0.004)', AB.fmtUsd(0.004) === '< $0.01', `got "${AB.fmtUsd(0.004)}"`);
      check('fmtUsd(2.734)', AB.fmtUsd(2.734) === '$2.73', `got "${AB.fmtUsd(2.734)}"`);
      check('fmtUsd(17.62)', AB.fmtUsd(17.62) === '$17.62', `got "${AB.fmtUsd(17.62)}"`);

      // fmtClock: null → `—`; now → matches `^\d\d:\d\d$`; local 2020-01-02 03:04 → `02/01 03:04`
      check('fmtClock(null)', AB.fmtClock(null) === '—', `got "${AB.fmtClock(null)}"`);
      check('fmtClock(now)', /^\d\d:\d\d$/.test(AB.fmtClock(Date.now())), `got "${AB.fmtClock(Date.now())}"`);
      const t2020 = new Date(2020, 0, 2, 3, 4).getTime();
      check('fmtClock(2020-01-02 03:04)', AB.fmtClock(t2020) === '02/01 03:04', `got "${AB.fmtClock(t2020)}"`);

      // stepWord: null → ''; {state: 'killed', label: 'killed'} → `đã dừng`; {state: 'xong', label: 'xong'} → `xong`; {state: 'vuong_mac', label: 'Vướng mắc'} → `Vướng mắc`
      check('stepWord(null)', AB.stepWord(null) === '', `got "${AB.stepWord(null)}"`);
      check('stepWord(killed)', AB.stepWord({ state: 'killed', label: 'killed' }) === 'đã dừng', `got "${AB.stepWord({ state: 'killed', label: 'killed' })}"`);
      check('stepWord(xong)', AB.stepWord({ state: 'xong', label: 'xong' }) === 'xong', `got "${AB.stepWord({ state: 'xong', label: 'xong' })}"`);
      check('stepWord(vuong_mac)', AB.stepWord({ state: 'vuong_mac', label: 'Vướng mắc' }) === 'Vướng mắc', `got "${AB.stepWord({ state: 'vuong_mac', label: 'Vướng mắc' })}"`);
    },
  },
  {
    name: 'focus',
    async run(ctx) {
      const AB = ctx.AB;
      const screenEl = {
        offsetParent: {},
        focus() {
          ctx.document.activeElement = this;
        },
      };
      ctx.document.getElementById = function (id) {
        return id === 'screen' ? screenEl : null;
      };

      ctx.document.activeElement = null;
      AB.focusLater('missing', 'screen');
      await new Promise((r) => setTimeout(r, 1500));
      check(
        'focusLater missing fallback to screen',
        ctx.document.activeElement === screenEl,
        `activeElement is ${ctx.document.activeElement === screenEl ? 'screen' : 'null'}`
      );

      ctx.document.activeElement = null;
      AB.focusLater('screen');
      await new Promise((r) => setTimeout(r, 100));
      check(
        'focusLater screen direct',
        ctx.document.activeElement === screenEl,
        `activeElement is ${ctx.document.activeElement === screenEl ? 'screen' : 'null'}`
      );
    },
  },
  {
    name: 'overview',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const overviewJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/overview.js'
      );
      const overviewJsSrc = fs.readFileSync(overviewJsPath, 'utf8');
      vm.runInContext(overviewJsSrc, ctx, { filename: 'overview.js' });

      check(
        'overview factory registered',
        typeof factories.overview === 'function',
        'factories.overview not found'
      );
      if (typeof factories.overview !== 'function') return;

      const comp = factories.overview();
      const killedTask = {
        steps: [{ role: 'lead', state: 'killed', label: 'killed' }],
      };
      const killedLabel = comp.pillLabel(killedTask, 'lead');
      check(
        'overview pillLabel lead killed',
        killedLabel === 'Lead: đã dừng',
        `got "${killedLabel}"`
      );

      const runningTask = {
        steps: [{ role: 'dev', state: 'dang_chay', label: 'đang chạy' }],
      };
      const runningLabel = comp.pillLabel(runningTask, 'dev');
      check(
        'overview pillLabel dev running',
        runningLabel === 'Dev: đang chạy',
        `got "${runningLabel}"`
      );
    },
  },
  {
    name: 'task-cards',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };

      const taskJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/task.js'
      );
      const taskJsSrc = fs.readFileSync(taskJsPath, 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      check(
        'task factory registered',
        typeof factories.task === 'function',
        'factories.task not found'
      );
      if (typeof factories.task !== 'function') return;

      const T_LEAD = {
        id: 1,
        slug: 'sua-app',
        status: 'planned',
        current_step: 'dev',
        group: 'waiting',
        actions: [
          { id: 'run_dev', label: 'Chạy Dev', enabled: true, reason: null },
          { id: 'relead', label: 'Chạy lại Lead', enabled: true, reason: null },
        ],
        steps: [],
        runs: [{ id: 1, role: 'lead', status: 'succeeded', outcome: null }],
      };

      const fixedMs = Date.now() - 600000;
      const P_LEAD = {
        task_id: 1,
        cycle: 1,
        current: 'ban',
        cards: {
          start: {
            state: 'xong',
            state_word: 'xong',
            result: 'bạn đã giao',
            at_ms: fixedMs - 420000,
          },
          plan: {
            role: 'lead',
            state: 'xong',
            state_word: 'xong',
            result: 'plan đã commit',
            rounds: 1,
            run_id: 1,
            duration_ms: 420000,
            cost_usd: 2.73,
          },
          ban: {
            state: 'cho_ban',
            state_word: 'chờ bạn',
            result: 'chờ duyệt plan',
            at_ms: fixedMs,
          },
          code: {
            role: 'dev',
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa bắt đầu',
            rounds: 0,
            run_id: null,
            duration_ms: null,
            cost_usd: null,
          },
          qa: {
            role: 'qa',
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa bắt đầu',
            rounds: 0,
            run_id: null,
            duration_ms: null,
            cost_usd: null,
          },
        },
        ledger: { groups: [], total: { runs: 1, duration_ms: 420000, cost_usd: 2.73, cost_unknown: 0 } },
      };

      const P_EMPTY = {
        task_id: 1,
        cycle: null,
        current: null,
        cards: {
          start: {
            state: 'xong',
            state_word: 'xong',
            result: 'bạn đã giao',
            at_ms: fixedMs,
          },
          plan: {
            role: 'lead',
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa bắt đầu',
            rounds: 0,
            run_id: null,
            duration_ms: null,
            cost_usd: null,
          },
          ban: {
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa có plan',
            at_ms: null,
          },
          code: {
            role: 'dev',
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa bắt đầu',
            rounds: 0,
            run_id: null,
            duration_ms: null,
            cost_usd: null,
          },
          qa: {
            role: 'qa',
            state: 'cho',
            state_word: 'chưa bắt đầu',
            result: 'chưa bắt đầu',
            rounds: 0,
            run_id: null,
            duration_ms: null,
            cost_usd: null,
          },
        },
        ledger: { groups: [], total: { runs: 0, duration_ms: null, cost_usd: null, cost_unknown: 0 } },
      };

      const T_KILLED = {
        ...T_LEAD,
        runs: [
          { id: 1, role: 'lead', status: 'succeeded', outcome: null },
          { id: 2, role: 'dev', status: 'killed', outcome: null },
        ],
      };

      const P_KILLED = {
        ...P_LEAD,
        current: 'code',
        cards: {
          ...P_LEAD.cards,
          ban: {
            state: 'xong',
            state_word: 'xong',
            result: 'đã duyệt plan',
            at_ms: fixedMs,
          },
          code: {
            role: 'dev',
            state: 'killed',
            state_word: 'đã dừng',
            result: 'đã dừng',
            rounds: 1,
            run_id: 2,
            duration_ms: 120000,
            cost_usd: null,
          },
        },
      };

      const c = factories.task();
      c.task = T_LEAD;
      c.pipe = P_LEAD;

      // Checks: with P_LEAD
      check('cardState(ban)', c.cardState('ban') === 'cho_ban', `got "${c.cardState('ban')}"`);
      check('cardCurrentAttr(ban)', c.cardCurrentAttr('ban') === 'step', `got "${c.cardCurrentAttr('ban')}"`);
      check('cardCurrentAttr(plan)', c.cardCurrentAttr('plan') === false, `got "${c.cardCurrentAttr('plan')}"`);
      const banCls = c.cardClass('ban');
      check('cardClass(ban) has ring-2 ring-indigo-600', banCls.includes('ring-2 ring-indigo-600'), `got "${banCls}"`);
      check('cardClass(ban) has border-amber-300', banCls.includes('border-amber-300'), `got "${banCls}"`);
      const codeCls = c.cardClass('code');
      check('cardClass(code) has border-dashed border-slate-500', codeCls.includes('border-dashed border-slate-500'), `got "${codeCls}"`);
      check('cardClass(code) has no ring-2', !codeCls.includes('ring-2'), `got "${codeCls}"`);
      check('cardResult(plan)', c.cardResult('plan') === 'plan đã commit', `got "${c.cardResult('plan')}"`);
      check('cardSr(plan)', c.cardSr('plan') === 'xong, ', `got "${c.cardSr('plan')}"`);
      check('cardMinutes(plan)', c.cardMinutes('plan') === '7 phút', `got "${c.cardMinutes('plan')}"`);
      check('cardCost(plan)', c.cardCost('plan') === '$2.73', `got "${c.cardCost('plan')}"`);
      check('loopFooter(code)', c.loopFooter('code') === 'chưa chạy', `got "${c.loopFooter('code')}"`);
      const planFooter = c.loopFooter('plan');
      check('loopFooter(plan) ends with vòng 1', planFooter.endsWith(' vòng 1'), `got "${planFooter}"`);
      check('loopFooter(plan) contains no /', !planFooter.includes('/'), `got "${planFooter}"`);
      const banTime = c.cardTime('ban');
      check('cardTime(ban) starts with từ ', banTime.startsWith('từ '), `got "${banTime}"`);

      // with P_EMPTY
      c.pipe = P_EMPTY;
      for (const k of ['start', 'plan', 'ban', 'code', 'qa']) {
        check(`P_EMPTY cardCurrentAttr(${k}) is false`, c.cardCurrentAttr(k) === false, `got "${c.cardCurrentAttr(k)}"`);
      }

      // strip with T_LEAD
      c.pipe = P_LEAD;
      check('stripCaption with T_LEAD', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);
      check('actionList length 2', c.actionList.length === 2, `got ${c.actionList.length}`);
      check('actionId second is ab-act-relead', c.actionId(c.actionList[1]) === 'ab-act-relead', `got "${c.actionId(c.actionList[1])}"`);

      // strip independent of /pipeline
      c.pipe = null;
      c.pipeError = 'x';
      check('stripCaption with pipe=null', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);
      check('actionList length 2 with pipe=null', c.actionList.length === 2, `got ${c.actionList.length}`);
      c.pipeError = '';

      // killed Dev (caption follows the ring)
      c.task = T_KILLED;
      c.pipe = P_KILLED;
      check('cardCurrentAttr(code) is step with P_KILLED', c.cardCurrentAttr('code') === 'step', `got "${c.cardCurrentAttr('code')}"`);
      check('cardCurrentAttr(ban) is false with P_KILLED', c.cardCurrentAttr('ban') === false, `got "${c.cardCurrentAttr('ban')}"`);
      check('stripCaption follows ring to Code', c.stripCaption === 'Bước hiện tại: Code', `got "${c.stripCaption}"`);

      // with T_KILLED and pipe = null
      c.pipe = null;
      check('stripCaption fallback with T_KILLED is Bạn', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);

      // agreement check
      for (const pair of [
        { t: T_LEAD, p: P_LEAD },
        { t: T_KILLED, p: P_KILLED },
      ]) {
        c.task = pair.t;
        c.pipe = pair.p;
        const allKeys = ['start', 'plan', 'ban', 'code', 'qa'];
        const ringedKeys = allKeys.filter((k) => c.cardCurrentAttr(k) === 'step');
        check(
          'agreement: exactly one card is current',
          ringedKeys.length === 1,
          `ringed cards: ${ringedKeys.join(', ')}`
        );
        const ringedKey = ringedKeys[0];
        const expectedTitle = c.stageTitles[ringedKey];
        check(
          `agreement: stripCaption matches card title "${expectedTitle}"`,
          c.stripCaption === 'Bước hiện tại: ' + expectedTitle,
          `got "${c.stripCaption}"`
        );
      }

      // pipeState
      c.pipe = null;
      c.pipeError = '';
      check('pipeState loading fresh', c.pipeState === 'loading', `got "${c.pipeState}"`);
      c.pipe = P_LEAD;
      check('pipeState ready with data', c.pipeState === 'ready', `got "${c.pipeState}"`);
      c.pipe = null;
      c.pipeError = 'x';
      check('pipeState error with only pipeError', c.pipeState === 'error', `got "${c.pipeState}"`);
      c.pipeError = '';

      // loadPipeline stubbing
      const apiCalls = [];
      const deferredQueue = [];
      ctx.AB.api = function (method, url) {
        apiCalls.push({ method, url });
        let resolve, reject;
        const promise = new Promise((res, rej) => {
          resolve = res;
          reject = rej;
        });
        deferredQueue.push({ resolve, reject });
        return promise;
      };

      const c2 = factories.task();
      c2.id = '1';
      c2.loadPipeline();
      c2.loadPipeline();
      check('two calls -> one request', apiCalls.length === 1, `got ${apiCalls.length}`);

      const firstReq = deferredQueue.shift();
      firstReq.resolve({ ok: true, status: 200, data: P_LEAD });
      await Promise.resolve();
      await Promise.resolve();

      check('resolving triggers exactly one trailing request', apiCalls.length === 2, `got ${apiCalls.length}`);
      check('pipe set to P_LEAD', c2.pipe === P_LEAD, 'pipe not P_LEAD');

      const secondReq = deferredQueue.shift();
      secondReq.resolve({ ok: true, status: 200, data: P_EMPTY });
      await Promise.resolve();
      await Promise.resolve();

      check('resolving trailing leaves pipe === P_EMPTY', c2.pipe === P_EMPTY, 'pipe not P_EMPTY');

      c2.loadPipeline();
      check('third request made', apiCalls.length === 3, `got ${apiCalls.length}`);
      const thirdReq = deferredQueue.shift();
      thirdReq.resolve({ ok: false, status: 500, data: null });
      await Promise.resolve();
      await Promise.resolve();

      check('500 keeps pipe', c2.pipe === P_EMPTY, 'pipe wiped');
      check('500 sets pipeStale', c2.pipeStale === true, `pipeStale is ${c2.pipeStale}`);

      c2.pipeStale = false;
      c2.loadPipeline();
      check('fourth request made', apiCalls.length === 4, `got ${apiCalls.length}`);
      const fourthReq = deferredQueue.shift();
      c2._pipeSeq += 1;
      fourthReq.resolve({ ok: true, status: 200, data: P_LEAD });
      await Promise.resolve();
      await Promise.resolve();

      check('stale response discarded: pipe unchanged', c2.pipe === P_EMPTY, 'pipe was overwritten by stale response');
    },
  },
  {
    name: 'task-states',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };

      const taskJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/task.js'
      );
      const taskJsSrc = fs.readFileSync(taskJsPath, 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const c = factories.task();
      c.id = '1';

      // Behavior 1: pipeLoading, pipeFailed, pipeErrorBody, showStale
      c.pipe = null;
      c.pipeError = '';
      check('pipeLoading true fresh', c.pipeLoading === true, `got ${c.pipeLoading}`);
      check('pipeFailed false fresh', c.pipeFailed === false, `got ${c.pipeFailed}`);

      c.pipeError = 'Lỗi 500';
      check('pipeLoading false on error', c.pipeLoading === false, `got ${c.pipeLoading}`);
      check('pipeFailed true on error', c.pipeFailed === true, `got ${c.pipeFailed}`);
      check(
        'pipeErrorBody format',
        c.pipeErrorBody === 'Lỗi 500. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.',
        `got "${c.pipeErrorBody}"`
      );

      c.pipe = { cards: {} };
      c.pipeError = '';
      check('pipeLoading false with pipe', c.pipeLoading === false, `got ${c.pipeLoading}`);
      check('pipeFailed false with pipe', c.pipeFailed === false, `got ${c.pipeFailed}`);

      c.stale = false;
      c.pipeStale = false;
      check('showStale false when both fresh', c.showStale === false, `got ${c.showStale}`);
      c.stale = true;
      c.pipeStale = false;
      check('showStale true when task stale', c.showStale === true, `got ${c.showStale}`);
      c.stale = false;
      c.pipeStale = true;
      check('showStale true when pipe stale', c.showStale === true, `got ${c.showStale}`);
      c.stale = false;
      c.pipeStale = false;

      // Behavior 2: bannerState, bannerClass, bannerGlyph
      c.task = null;
      check('bannerState null task -> cho', c.bannerState === 'cho', `got "${c.bannerState}"`);

      c.task = { group: 'running', status: 'running' };
      check('bannerState running -> dang_chay', c.bannerState === 'dang_chay', `got "${c.bannerState}"`);
      check('bannerClass running blue', c.bannerClass === ctx.AB.stepClass('dang_chay'), `got "${c.bannerClass}"`);
      check('bannerGlyph running', c.bannerGlyph === ctx.AB.stepGlyph('dang_chay'), `got "${c.bannerGlyph}"`);

      c.task = { group: 'waiting', status: 'planned' };
      check('bannerState waiting -> cho_ban', c.bannerState === 'cho_ban', `got "${c.bannerState}"`);
      check('bannerClass waiting amber', c.bannerClass === ctx.AB.stepClass('cho_ban'), `got "${c.bannerClass}"`);
      check('bannerGlyph waiting ●', c.bannerGlyph === ctx.AB.stepGlyph('cho_ban'), `got "${c.bannerGlyph}"`);

      c.task = { group: 'attention', status: 'blocked' };
      check('bannerState blocked -> vuong_mac', c.bannerState === 'vuong_mac', `got "${c.bannerState}"`);
      check('bannerGlyph blocked !', c.bannerGlyph === ctx.AB.stepGlyph('vuong_mac'), `got "${c.bannerGlyph}"`);

      c.task = { group: 'attention', status: 'qa_fix' };
      check('bannerState qa_fix -> vuong_mac', c.bannerState === 'vuong_mac', `got "${c.bannerState}"`);

      c.task = { group: 'attention', status: 'failed' };
      check('bannerState failed -> loi', c.bannerState === 'loi', `got "${c.bannerState}"`);
      check('bannerGlyph failed ■', c.bannerGlyph === ctx.AB.stepGlyph('loi'), `got "${c.bannerGlyph}"`);

      c.task = { group: 'done', status: 'succeeded' };
      check('bannerState done -> xong', c.bannerState === 'xong', `got "${c.bannerState}"`);
      check('bannerGlyph done ✓', c.bannerGlyph === ctx.AB.stepGlyph('xong'), `got "${c.bannerGlyph}"`);

      c.task = { group: 'unknown', status: 'unknown' };
      check('bannerState unknown -> cho', c.bannerState === 'cho', `got "${c.bannerState}"`);

      // Behavior 3: legend & placeholder
      for (const st of ['xong', 'dang_chay', 'cho_ban', 'killed', 'cho', 'chua_tu_dong', 'vuong_mac']) {
        check(`legendGlyph(${st})`, c.legendGlyph(st) === ctx.AB.stepGlyph(st), `got "${c.legendGlyph(st)}"`);
        check(`legendGlyphClass(${st})`, c.legendGlyphClass(st) === 'w-6 text-center ' + ctx.AB.stateText(st), `got "${c.legendGlyphClass(st)}"`);
      }
      check('placeholderGlyph', c.placeholderGlyph === ctx.AB.stepGlyph('chua_tu_dong'), `got "${c.placeholderGlyph}"`);

      // Behavior 4: SSE status refetch
      c.task = {
        id: 1,
        runs: [{ id: 1, role: 'lead', status: 'running', outcome: null }],
      };
      let pipeLoads = 0;
      c.loadPipeline = function () {
        pipeLoads++;
      };

      const esInstancesBefore = ctx.EventSource.instances.length;
      c.openStream(1);
      const es = ctx.EventSource.instances[ctx.EventSource.instances.length - 1];
      check('EventSource instantiated', Boolean(es && ctx.EventSource.instances.length > esInstancesBefore));

      const statusHandlers = es.handlers['status'] || [];
      check('status event handler registered', statusHandlers.length > 0);

      if (statusHandlers.length > 0) {
        // Same status & outcome -> no refetch
        statusHandlers[0]({ data: JSON.stringify({ role: 'lead', status: 'running', outcome: null }) });
        check('SSE status same status/outcome does not refetch', pipeLoads === 0, `pipeLoads=${pipeLoads}`);

        // Status changed to succeeded -> refetch
        statusHandlers[0]({ data: JSON.stringify({ role: 'lead', status: 'succeeded', outcome: null }) });
        check('SSE status change refetches pipeline once', pipeLoads === 1, `pipeLoads=${pipeLoads}`);
      }

      // Behavior 5: Dialog focus return
      const focusLaterCalls = [];
      const lockScrollCalls = [];
      ctx.AB.focusLater = function (id, fallback) {
        focusLaterCalls.push({ id, fallback });
      };
      ctx.AB.lockScroll = function (lock) {
        lockScrollCalls.push(lock);
      };

      let ranAction = null;
      c.runAction = function (act) {
        ranAction = act;
      };

      const releadAct = { id: 'relead', label: 'Chạy lại Lead' };
      c.onAction(releadAct);
      check('onAction(relead) stores lastFocusId', c.lastFocusId === 'ab-act-relead', `got "${c.lastFocusId}"`);
      check('onAction(relead) sets dialog', c.dialog === releadAct);
      check('onAction(relead) locks scroll', lockScrollCalls[lockScrollCalls.length - 1] === true);
      check('onAction(relead) focuses ab-dlg-cancel', focusLaterCalls[focusLaterCalls.length - 1].id === 'ab-dlg-cancel');

      // dialogCancel
      c.dialogCancel();
      check('dialogCancel clears lastFocusId', c.lastFocusId === '');
      check('dialogCancel clears dialog', c.dialog === null);
      check('dialogCancel unlocks scroll', lockScrollCalls[lockScrollCalls.length - 1] === false);
      const cancelFocus = focusLaterCalls[focusLaterCalls.length - 1];
      check('dialogCancel focuses ab-act-relead', cancelFocus.id === 'ab-act-relead' && cancelFocus.fallback === 'screen');

      // onAction + dialogOk
      c.onAction(releadAct);
      c.dialogOk();
      check('dialogOk clears lastFocusId', c.lastFocusId === '');
      check('dialogOk clears dialog', c.dialog === null);
      check('dialogOk unlocks scroll', lockScrollCalls[lockScrollCalls.length - 1] === false);
      const okFocus = focusLaterCalls[focusLaterCalls.length - 1];
      check('dialogOk focuses ab-act-relead', okFocus.id === 'ab-act-relead' && okFocus.fallback === 'screen');
      check('dialogOk ran action', ranAction === releadAct);
    },
  },
  {
    name: 'ledger',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };

      const taskJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/task.js'
      );
      const taskJsSrc = fs.readFileSync(taskJsPath, 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const c = factories.task();
      c.id = '1';

      const r1 = {
        run_id: 1,
        role: 'lead',
        round: 1,
        label: 'Vòng 1 · plan',
        state: 'xong',
        verdict: 'xong',
        turns: 69,
        duration_ms: 420000,
        cost_usd: 2.73,
        running: false,
        started_at: 1000,
        ended_at: 421000,
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 20,
        cache_write_tokens: 10,
      };

      const r2 = {
        run_id: 2,
        role: 'dev',
        round: 1,
        label: 'Vòng 1 · code',
        state: 'killed',
        verdict: 'đã dừng',
        turns: null,
        duration_ms: 30000,
        cost_usd: null,
        running: false,
        started_at: 422000,
        ended_at: 452000,
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
      };

      const r3 = {
        run_id: 3,
        role: 'dev',
        round: 2,
        label: 'Vòng 2 · code',
        state: 'xong',
        verdict: 'xong',
        turns: 15,
        duration_ms: 60000,
        cost_usd: 0.50,
        running: false,
        started_at: 453000,
        ended_at: 513000,
        input_tokens: 20,
        output_tokens: 20,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
      };

      const r4 = {
        run_id: 4,
        role: 'qa',
        round: 1,
        label: 'Vòng 1 · QA',
        state: 'can_sua',
        verdict: 'cần sửa',
        turns: 5,
        duration_ms: 210000,
        cost_usd: 0.52,
        running: false,
        started_at: 514000,
        ended_at: 724000,
        input_tokens: 30,
        output_tokens: 30,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
      };

      const P_LEDGER = {
        task_id: 1,
        cycle: 1,
        current: 'qa',
        cards: {},
        ledger: {
          groups: [
            {
              key: 'c1-plan-1',
              kind: 'plan',
              cycle: 1,
              header: 'Chu kỳ 1 · plan đầu tiên',
              right: 'bạn đã duyệt',
              rows: [r1],
            },
            {
              key: 'c1-build-1',
              kind: 'build',
              cycle: 1,
              header: 'Chu kỳ 1 · build',
              right: null,
              rows: [r2, r3],
            },
            {
              key: 'c1-qa-1',
              kind: 'qa',
              cycle: 1,
              header: 'Chu kỳ 1 · QA',
              right: null,
              rows: [r4],
            },
          ],
          total: {
            runs: 4,
            duration_ms: 720000,
            cost_usd: 3.75,
            cost_unknown: 1,
          },
        },
      };

      c.pipe = P_LEDGER;

      check('ledgerGroups.length === 3', c.ledgerGroups.length === 3, `got ${c.ledgerGroups.length}`);
      check('ledgerGroups[0] has 1 row', c.ledgerGroups[0] && c.ledgerGroups[0].rows.length === 1);
      check('ledgerGroups[1] has 2 rows', c.ledgerGroups[1] && c.ledgerGroups[1].rows.length === 2);
      check('ledgerGroups[2] has 1 row', c.ledgerGroups[2] && c.ledgerGroups[2].rows.length === 1);

      check(
        'totalText matches',
        c.totalText === '4 lượt chạy claude · 12 phút · $3.75',
        `got "${c.totalText}"`
      );

      check(
        'costNote with cost_unknown: 1',
        c.costNote === 'Chưa tính chi phí của 1 run bị dừng hoặc chưa có kết quả.',
        `got "${c.costNote}"`
      );

      c.pipe.ledger.total.cost_unknown = 0;
      check(
        'costNote with cost_unknown: 0 is empty',
        c.costNote === '',
        `got "${c.costNote}"`
      );
      c.pipe.ledger.total.cost_unknown = 1;

      check(
        'rowMetrics(r1)',
        c.rowMetrics(r1) === '69 lượt · 7 phút · $2.73',
        `got "${c.rowMetrics(r1)}"`
      );

      const rRunning = { turns: null, duration_ms: 60000, cost_usd: null, state: 'dang_chay' };
      check(
        'rowMetrics(running)',
        c.rowMetrics(rRunning) === '— · 1 phút · —',
        `got "${c.rowMetrics(rRunning)}"`
      );

      check(
        'rowTitle(r1) contains vào 100',
        c.rowTitle(r1).includes('vào 100'),
        `got "${c.rowTitle(r1)}"`
      );

      check(
        'rowTitle zero-token contains vào — · ra — · cache — / —',
        c.rowTitle(r2).includes('vào — · ra — · cache — / —'),
        `got "${c.rowTitle(r2)}"`
      );

      check(
        'rowVerdictClass(r2) contains text-rose-700',
        c.rowVerdictClass(r2).includes('text-rose-700'),
        `got "${c.rowVerdictClass(r2)}"`
      );

      c.runId = 2;
      check('ledgerRowCurrent(r2) === "true"', c.ledgerRowCurrent(r2) === 'true', `got "${c.ledgerRowCurrent(r2)}"`);
      check('ledgerRowCurrent(r1) === false', c.ledgerRowCurrent(r1) === false, `got "${c.ledgerRowCurrent(r1)}"`);
      check('ledgerRowClass(r2) contains bg-indigo-50', c.ledgerRowClass(r2).includes('bg-indigo-50'), `got "${c.ledgerRowClass(r2)}"`);

      let recorded = null;
      ctx.document.getElementById = function (id) {
        if (id === 'ab-log-card') {
          return {
            scrollIntoView(o) {
              recorded = o;
            },
          };
        }
        return null;
      };

      c.pickRun(r3);
      check('pickRun(r3) sets runId === 3', c.runId === 3, `got ${c.runId}`);
      check('pickRun(r3) sets pinned === true', c.pinned === true, `got ${c.pinned}`);
      check('pickRun(r3) records { block: "nearest" }', recorded && recorded.block === 'nearest', `got ${JSON.stringify(recorded)}`);

      c.pipe = {
        ledger: {
          groups: [],
          total: { runs: 0, duration_ms: null, cost_usd: null, cost_unknown: 0 },
        },
      };
      check('no-runs ledgerEmpty === true', c.ledgerEmpty === true, `got ${c.ledgerEmpty}`);
      check('no-runs showTotal === false', c.showTotal === false, `got ${c.showTotal}`);
    },
  },
  {
    name: 'ledger-paging',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };

      const taskJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/task.js'
      );
      const taskJsSrc = fs.readFileSync(taskJsPath, 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const c = factories.task();
      c.id = '1';

      // 26 rows over 9 groups (roles cycling lead, dev, qa)
      const groupRowCounts = [3, 4, 3, 3, 3, 3, 3, 2, 2]; // sum = 26
      const roles = ['lead', 'dev', 'qa'];
      let rowId = 1;
      const groups26 = [];
      for (let gIdx = 0; gIdx < groupRowCounts.length; gIdx++) {
        const count = groupRowCounts[gIdx];
        const role = roles[gIdx % 3];
        const rows = [];
        for (let r = 0; r < count; r++) {
          rows.push({
            run_id: rowId,
            role,
            round: r + 1,
            label: `Vòng ${r + 1} · ${role}`,
            state: 'xong',
            verdict: 'xong',
            turns: 3,
            duration_ms: 30000,
            cost_usd: 0.10,
            running: false,
            started_at: rowId * 60000,
            ended_at: rowId * 60000 + 30000,
            input_tokens: 10,
            output_tokens: 10,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
          });
          rowId++;
        }
        groups26.push({
          key: `g-${gIdx}`,
          kind: role === 'lead' ? 'plan' : (role === 'dev' ? 'build' : 'qa'),
          cycle: Math.floor(gIdx / 3) + 1,
          header: `Nhóm ${gIdx}`,
          right: null,
          rows,
        });
      }

      c.pipe = {
        ledger: {
          groups: groups26,
          total: {
            runs: 26,
            duration_ms: 780000,
            cost_usd: 2.60,
            cost_unknown: 0,
          },
        },
      };

      c.ledgerVisible = 20;

      // Check: ledgerGroups holds exactly newest 20 rows
      const visibleRows26 = [];
      c.ledgerGroups.forEach(g => visibleRows26.push(...g.rows));
      check('ledgerGroups holds exactly 20 rows', visibleRows26.length === 20, `got ${visibleRows26.length}`);
      check('first visible row is run_id 7', visibleRows26[0] && visibleRows26[0].run_id === 7, `got ${visibleRows26[0]?.run_id}`);
      check('last visible row is run_id 26', visibleRows26[19] && visibleRows26[19].run_id === 26, `got ${visibleRows26[19]?.run_id}`);

      // Drops groups with no visible row (group 0 dropped)
      check('first visible group is g-1', c.ledgerGroups[0] && c.ledgerGroups[0].key === 'g-1', `got ${c.ledgerGroups[0]?.key}`);
      // Keeps partially visible group with only its visible rows (g-1 had 4 rows, only 1 kept)
      check('partially visible group g-1 has 1 row', c.ledgerGroups[0] && c.ledgerGroups[0].rows.length === 1, `got ${c.ledgerGroups[0]?.rows?.length}`);
      check('ledgerHasMore is true', c.ledgerHasMore === true);

      // totalText never depends on window
      check('totalText uses total (26 runs)', c.totalText === '26 lượt chạy claude · 13 phút · $2.60', `got "${c.totalText}"`);

      // showMoreRuns() reveals all 26 rows
      c.showMoreRuns();
      check('after showMoreRuns ledgerVisible is 40', c.ledgerVisible === 40);
      const allRevealed = [];
      c.ledgerGroups.forEach(g => allRevealed.push(...g.rows));
      check('all 26 rows visible', allRevealed.length === 26, `got ${allRevealed.length}`);
      check('all 9 groups present', c.ledgerGroups.length === 9, `got ${c.ledgerGroups.length}`);
      check('ledgerHasMore is false', c.ledgerHasMore === false);

      // exactly 20 rows -> ledgerHasMore false
      c.ledgerVisible = 20;
      const groups20 = [
        {
          key: 'g20',
          kind: 'plan',
          cycle: 1,
          header: 'Nhóm 20',
          right: null,
          rows: groups26[0].rows.concat(groups26[1].rows, groups26[2].rows, groups26[3].rows, groups26[4].rows, groups26[5].rows, groups26[6].rows).slice(0, 20),
        },
      ];
      c.pipe = {
        ledger: {
          groups: groups20,
          total: { runs: 20, duration_ms: 600000, cost_usd: 2.00, cost_unknown: 0 },
        },
      };
      check('20 rows ledgerHasMore is false', c.ledgerHasMore === false);

      // 21 rows -> ledgerHasMore true; one showMoreRuns() shows 21 rows
      const groups21 = [
        {
          key: 'g21',
          kind: 'plan',
          cycle: 1,
          header: 'Nhóm 21',
          right: null,
          rows: groups26[0].rows.concat(groups26[1].rows, groups26[2].rows, groups26[3].rows, groups26[4].rows, groups26[5].rows, groups26[6].rows, groups26[7].rows).slice(0, 21),
        },
      ];
      c.pipe = {
        ledger: {
          groups: groups21,
          total: { runs: 21, duration_ms: 630000, cost_usd: 2.10, cost_unknown: 0 },
        },
      };
      check('21 rows ledgerHasMore is true', c.ledgerHasMore === true);
      const rows21Before = [];
      c.ledgerGroups.forEach(g => rows21Before.push(...g.rows));
      check('21 rows initially shows 20', rows21Before.length === 20, `got ${rows21Before.length}`);

      c.showMoreRuns();
      check('21 rows after showMoreRuns ledgerHasMore is false', c.ledgerHasMore === false);
      const rows21After = [];
      c.ledgerGroups.forEach(g => rows21After.push(...g.rows));
      check('21 rows after showMoreRuns shows 21', rows21After.length === 21, `got ${rows21After.length}`);
    },
  },
];

async function main() {
  const ctx = makeContext();
  for (const section of sections) {
    await section.run(ctx);
  }

  if (failures.length > 0) {
    for (const f of failures) {
      console.error(`LỖI ${f.name}: ${f.detail}`);
    }
    process.exit(1);
  }

  console.log(`JS check OK: ${checkCount} checks`);
}

main().catch((err) => {
  console.error(`LỖI uncaught: ${err.message || err}`);
  process.exit(1);
});

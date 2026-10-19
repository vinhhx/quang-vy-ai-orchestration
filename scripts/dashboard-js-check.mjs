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

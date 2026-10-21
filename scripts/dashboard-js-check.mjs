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
        duyet: {
          codePoint: 0x2713,
          text: 'text-emerald-700 dark:text-emerald-300',
          card: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
          word: 'duyệt',
        },
        sua_lai: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'sửa lại',
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

      // 05.2 vocabulary: duyet reuses xong, sua_lai reuses can_sua
      check('stateCard(duyet) == stateCard(xong)', AB.stateCard('duyet') === AB.stateCard('xong'), `got "${AB.stateCard('duyet')}"`);
      check('stepClass(duyet) == stepClass(xong)', AB.stepClass('duyet') === AB.stepClass('xong'), `got "${AB.stepClass('duyet')}"`);
      check('stateText(sua_lai) == stateText(can_sua)', AB.stateText('sua_lai') === AB.stateText('can_sua'), `got "${AB.stateText('sua_lai')}"`);
      check('stepClass(sua_lai) == stepClass(can_sua)', AB.stepClass('sua_lai') === AB.stepClass('can_sua'), `got "${AB.stepClass('sua_lai')}"`);
      check('stepGlyph(sua_lai) == stepGlyph(can_sua)', AB.stepGlyph('sua_lai') === AB.stepGlyph('can_sua'), `got "${AB.stepGlyph('sua_lai')}"`);
      check('roleName(plan-review)', AB.roleName('plan-review') === 'Review plan', `got "${AB.roleName('plan-review')}"`);
      check('roleName(code-review)', AB.roleName('code-review') === 'Review code', `got "${AB.roleName('code-review')}"`);
      check('roleName(lead) unchanged', AB.roleName('lead') === 'Lead', `got "${AB.roleName('lead')}"`);

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

      // countParts: order blocking, advisory, specs, not_automatable, failing; unknown omitted, known 0 shown
      const cp1 = AB.countParts({ blocking: 0, advisory: 4, specs: null, failing: null, not_automatable: null });
      check('countParts review', JSON.stringify(cp1) === JSON.stringify(['0 chặn', '4 góp ý']), `got ${JSON.stringify(cp1)}`);
      const cp2 = AB.countParts({ specs: 3, failing: 1, not_automatable: 2 });
      check('countParts qa', JSON.stringify(cp2) === JSON.stringify(['3 kiểm thử', '2 không tự động được', '1 lỗi']), `got ${JSON.stringify(cp2)}`);
      const cp3 = AB.countParts(null);
      check('countParts(null)', Array.isArray(cp3) && cp3.length === 0, `got ${JSON.stringify(cp3)}`);
      const cp4 = AB.countParts({ blocking: 2 });
      check('countParts undefined omitted', JSON.stringify(cp4) === JSON.stringify(['2 chặn']), `got ${JSON.stringify(cp4)}`);
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

      // Phase 5.2: Chờ bạn hint, autopilot stop reason line, Autopilot badge
      const DETAIL = 'Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.';
      const stoppedTask = { id: 2, group: 'waiting', auto_state: 'stopped', stop_reason: 'het_vong', stop_detail: DETAIL, banner: 'Chờ bạn · hết vòng', updated_at: 2000 };
      const gateTask = { id: 1, group: 'waiting', auto_state: 'gate', stop_reason: null, stop_detail: null, banner: 'Chờ duyệt plan', updated_at: 1000 };
      const runTask = { id: 3, group: 'running', auto_state: 'running', stop_detail: null };
      const stoppingTask = { id: 4, group: 'running', auto_state: 'stopping', stop_detail: null };
      comp.tasks = [stoppedTask, gateTask, runTask, stoppingTask];
      const waiting = comp.groups.find((g) => g.key === 'waiting');
      check('overview: waiting group exists', Boolean(waiting));
      check(
        'overview: waiting hint',
        waiting && waiting.hint === 'Duyệt plan hoặc xử lý chỗ autopilot dừng',
        `got "${waiting && waiting.hint}"`
      );
      check(
        'overview: waiting keeps server order (newest first)',
        waiting && waiting.items.length === 2 && waiting.items[0].id === 2 && waiting.items[1].id === 1,
        `got ${waiting && waiting.items.map((t) => t.id).join(',')}`
      );
      check('overview: hasReason stopped', comp.hasReason(stoppedTask) === true);
      check('overview: reasonText stopped', comp.reasonText(stoppedTask) === DETAIL, `got "${comp.reasonText(stoppedTask)}"`);
      check('overview: hasReason gate is false', comp.hasReason(gateTask) === false);
      check('overview: reasonText gate is empty', comp.reasonText(gateTask) === '', `got "${comp.reasonText(gateTask)}"`);
      check('overview: hasReason stopped without detail is false', comp.hasReason({ auto_state: 'stopped', stop_detail: null }) === false);
      check('overview: showAutoBadge running', comp.showAutoBadge(runTask) === true);
      check('overview: showAutoBadge stopping', comp.showAutoBadge(stoppingTask) === true);
      for (const st of ['off', 'gate', 'stopped', 'lost', 'done', undefined]) {
        check(`overview: showAutoBadge ${st} is false`, comp.showAutoBadge({ auto_state: st }) === false);
      }
      check(
        'overview: autoBadgeClass is the dang_chay step class',
        comp.autoBadgeClass === ctx.AB.stepClass('dang_chay'),
        `got "${comp.autoBadgeClass}"`
      );
      check(
        'overview: autoBadgeGlyph is the dang_chay glyph',
        comp.autoBadgeGlyph === ctx.AB.stepGlyph('dang_chay'),
        `got "${comp.autoBadgeGlyph}"`
      );

    },
  },
  {
    name: 'assign-workflow',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const assignJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/assign.js'
      );
      const assignJsSrc = fs.readFileSync(assignJsPath, 'utf8');
      vm.runInContext(assignJsSrc, ctx, { filename: 'assign.js' });

      check(
        'assign factory registered',
        typeof factories.assign === 'function',
        'factories.assign not found'
      );
      if (typeof factories.assign !== 'function') return;

      const SUMMARY_MAC = 'plan ⇄︎ review → Bạn → code ⇄︎ review → QA ⇄︎ sửa';
      const WF_LIST = [
        { name: 'mac-dinh', source: 'builtin', summary: SUMMARY_MAC, auto: true, loops: true, limits: '3 vòng · 60 phút · $20', default_for: [] },
        { name: 'nhanh', source: 'builtin', summary: 'plan → Bạn → code → QA', auto: false, loops: false, limits: '60 phút · $20', default_for: ['Web'] },
        { name: 'chi-phi-thap', source: 'file', summary: 'plan → Bạn → code → QA', auto: true, loops: false, limits: '30 phút · $5', default_for: [] },
      ];
      const NOTE = "Quy trình 'x' của dự án không có trong workflows.json — dùng mac-dinh.";
      let wfResponse = {
        ok: true,
        status: 200,
        data: {
          workflows: WF_LIST,
          errors: [],
          project_defaults: {
            web: { workflow: 'nhanh', effective: 'nhanh', note: null },
            app: { workflow: null, effective: 'mac-dinh', note: null },
            cu: { workflow: 'x', effective: 'mac-dinh', note: NOTE },
          },
        },
      };
      const posts = [];
      const savedApi = ctx.AB.api;
      ctx.AB.api = function (method, url, body) {
        if (method === 'GET' && url === '/api/workflows') {
          return Promise.resolve(wfResponse);
        }
        if (method === 'POST' && url === '/api/tasks') {
          posts.push(body);
          return Promise.resolve({ ok: false, status: 422, data: { error: 'Quy trình không hợp lệ' } });
        }
        return Promise.resolve({ ok: false, status: 404, data: null });
      };

      try {
        const c = factories.assign();
        check('assign: workflowsLoading starts true', c.workflowsLoading === true, `got ${c.workflowsLoading}`);
        check('assign: workflowName starts mac-dinh', c.workflowName === 'mac-dinh', `got "${c.workflowName}"`);
        await c.loadWorkflows();
        check('assign: workflows loaded in server order', c.workflows.length === 3 && c.workflows[0].name === 'mac-dinh' && c.workflows[2].name === 'chi-phi-thap', `got ${c.workflows.map((w) => w.name).join(',')}`);
        check('assign: workflowsLoading false after load', c.workflowsLoading === false, `got ${c.workflowsLoading}`);
        check('assign: no project → mac-dinh', c.workflowName === 'mac-dinh', `got "${c.workflowName}"`);
        check('assign: no project → no default chip', JSON.stringify(c.optionChips(WF_LIST[0])) === JSON.stringify(['có sẵn', 'autopilot']), `got ${JSON.stringify(c.optionChips(WF_LIST[0]))}`);
        check('assign: file chips', JSON.stringify(c.optionChips(WF_LIST[2])) === JSON.stringify(['từ workflows.json', 'autopilot']), `got ${JSON.stringify(c.optionChips(WF_LIST[2]))}`);
        check('assign: helper for auto workflow', c.selectedHelp === 'Quy trình này tự chạy autopilot và luôn dừng ở bước bạn duyệt plan.', `got "${c.selectedHelp}"`);
        check('assign: mac-dinh option selected class', c.workflowOptionClass(WF_LIST[0]) === 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400', `got "${c.workflowOptionClass(WF_LIST[0])}"`);
        check('assign: nhanh option unselected class', c.workflowOptionClass(WF_LIST[1]).includes('ring-1 ring-slate-300') && !c.workflowOptionClass(WF_LIST[1]).includes('indigo'), `got "${c.workflowOptionClass(WF_LIST[1])}"`);
        check('assign: workflowChecked', c.workflowChecked(WF_LIST[0]) === true && c.workflowChecked(WF_LIST[1]) === false);

        c.choose({ id: 'web', name_vi: 'Web', profile: 'work', workflow: 'nhanh' });
        check('assign: choose web → nhanh (project default)', c.workflowName === 'nhanh', `got "${c.workflowName}"`);
        check('assign: workflowTouched stays false after choose', c.workflowTouched === false, `got ${c.workflowTouched}`);
        const nhanhChips = c.optionChips(WF_LIST[1]);
        check('assign: nhanh chips with project default', JSON.stringify(nhanhChips) === JSON.stringify(['mặc định của dự án', 'có sẵn', 'chạy tay']), `got ${JSON.stringify(nhanhChips)}`);
        const macChips = c.optionChips(WF_LIST[0]);
        check('assign: mac-dinh chips without default chip', JSON.stringify(macChips) === JSON.stringify(['có sẵn', 'autopilot']), `got ${JSON.stringify(macChips)}`);
        check('assign: helper for manual workflow', c.selectedHelp === 'Quy trình này chạy tay: bạn bấm từng bước.', `got "${c.selectedHelp}"`);

        c.choose({ id: 'app', name_vi: 'App', profile: 'work', workflow: null });
        check('assign: project without default → mac-dinh', c.workflowName === 'mac-dinh', `got "${c.workflowName}"`);
        const macChipsApp = c.optionChips(WF_LIST[0]);
        check('assign: mac-dinh default chip for app', macChipsApp.includes('mặc định của dự án') && macChipsApp.includes('có sẵn') && macChipsApp.includes('autopilot'), `got ${JSON.stringify(macChipsApp)}`);

        c.choose({ id: 'cu', name_vi: 'Cũ', profile: 'work', workflow: 'x' });
        check('assign: unknown project default → mac-dinh', c.workflowName === 'mac-dinh', `got "${c.workflowName}"`);
        check('assign: project default note shown verbatim', c.selectedHelp === NOTE, `got "${c.selectedHelp}"`);

        c.choose({ id: 'web', name_vi: 'Web', profile: 'work', workflow: 'nhanh' });
        c.onWorkflowChange({ target: { value: 'chi-phi-thap' } });
        check('assign: onWorkflowChange picks the radio value', c.workflowName === 'chi-phi-thap', `got "${c.workflowName}"`);
        check('assign: onWorkflowChange sets workflowTouched', c.workflowTouched === true, `got ${c.workflowTouched}`);
        c.choose({ id: 'app', name_vi: 'App', profile: 'work', workflow: null });
        check('assign: touched selection survives a project change', c.workflowName === 'chi-phi-thap', `got "${c.workflowName}"`);

        const c2 = factories.assign();
        await c2.loadWorkflows();
        c2.pickWorkflow('chi-phi-thap');
        check('assign: pickWorkflow sets workflowTouched', c2.workflowTouched === true, `got ${c2.workflowTouched}`);
        c2.choose({ id: 'web', name_vi: 'Web', profile: 'work', workflow: 'nhanh' });
        check('assign: pickWorkflow then choose keeps chi-phi-thap', c2.workflowName === 'chi-phi-thap', `got "${c2.workflowName}"`);

        // postBody carries the selected workflow once the list loaded
        const body = c2.postBody;
        check('assign: postBody.workflow == workflowName', body.workflow === c2.workflowName && body.project === 'web', `got ${JSON.stringify(body)}`);
        check('assign: the picker adds no disabled reason', c2.disabledReason === 'Nhập yêu cầu', `got "${c2.disabledReason}"`);

        // submit posts postBody
        c2.request = 'sửa app';
        c2.slug = 'sua-app';
        c2.preflight = { can_start: true };
        c2.preflightPending = false;
        await c2.submit();
        check('assign: submit posts once', posts.length === 1, `got ${posts.length}`);
        check('assign: submit POST body has workflow', posts[0] && posts[0].workflow === 'chi-phi-thap' && posts[0].name === 'sua-app' && posts[0].request === 'sửa app', `got ${JSON.stringify(posts[0])}`);

        // errors: 12 server lines → 10 + '… và 2 lỗi khác'
        const errs = [];
        for (let i = 1; i <= 12; i++) errs.push(`Dòng ${i}: lỗi ${i}`);
        wfResponse = { ok: true, status: 200, data: { workflows: WF_LIST.slice(0, 2), errors: errs, project_defaults: {} } };
        const c3 = factories.assign();
        await c3.loadWorkflows();
        check('assign: hasWorkflowErrors', c3.hasWorkflowErrors === true);
        check('assign: errorLines has 11 lines', c3.errorLines.length === 11, `got ${c3.errorLines.length}`);
        check('assign: errorLines first is server line 1', c3.errorLines[0] === 'Dòng 1: lỗi 1', `got "${c3.errorLines[0]}"`);
        check('assign: errorLines tenth is server line 10', c3.errorLines[9] === 'Dòng 10: lỗi 10', `got "${c3.errorLines[9]}"`);
        check('assign: errorLines last is the remainder', c3.errorLines[10] === '… và 2 lỗi khác', `got "${c3.errorLines[10]}"`);
        check('assign: built-ins still selectable with errors', c3.workflows.length === 2 && c3.workflowName === 'mac-dinh', `got ${c3.workflows.length} / "${c3.workflowName}"`);

        // fetch failure → workflowsFailed, no workflow key in the POST body
        wfResponse = { ok: false, status: 0, data: null };
        const c4 = factories.assign();
        await c4.loadWorkflows();
        check('assign: workflowsFailed on fetch failure', c4.workflowsFailed === true && c4.workflowsLoading === false, `got ${c4.workflowsFailed} / ${c4.workflowsLoading}`);
        c4.choose({ id: 'web', name_vi: 'Web', profile: 'work', workflow: 'nhanh' });
        const body4 = c4.postBody;
        check('assign: failed list → postBody has no workflow key', !Object.prototype.hasOwnProperty.call(body4, 'workflow'), `got ${JSON.stringify(body4)}`);
        check('assign: failed list → no selected helper', c4.selectedHelp === '', `got "${c4.selectedHelp}"`);

        // still loading → no workflow key either (server default)
        const c5 = factories.assign();
        c5.choose({ id: 'web', name_vi: 'Web', profile: 'work', workflow: 'nhanh' });
        check('assign: loading → postBody has no workflow key', !Object.prototype.hasOwnProperty.call(c5.postBody, 'workflow'), `got ${JSON.stringify(c5.postBody)}`);
      } finally {
        ctx.AB.api = savedApi;
      }
    },
  },
  {
    name: 'projects-workflows',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const projectsJsPath = path.join(
        repoRoot,
        'agent-brain/assets/dashboard/js/projects.js'
      );
      const projectsJsSrc = fs.readFileSync(projectsJsPath, 'utf8');
      vm.runInContext(projectsJsSrc, ctx, { filename: 'projects.js' });

      check(
        'projects factory registered',
        typeof factories.projects === 'function',
        'factories.projects not found'
      );
      if (typeof factories.projects !== 'function') return;

      const WF_LIST = [
        { name: 'mac-dinh', source: 'builtin', summary: 'plan ⇄︎ review → Bạn → code ⇄︎ review → QA ⇄︎ sửa', auto: true, limits: '3 vòng · 60 phút · $20', default_for: [] },
        { name: 'nhanh', source: 'override', summary: 'plan → Bạn → code → QA', auto: true, limits: '60 phút · $20', default_for: ['Web bán hàng'] },
        { name: 'chi-phi-thap', source: 'file', summary: 'plan → Bạn → code → QA', auto: false, limits: '30 phút · $5', default_for: [] },
      ];
      let resolveNext = null;
      let wfResponse = {
        ok: true,
        status: 200,
        data: {
          workflows: WF_LIST,
          errors: [],
          project_defaults: {
            'web-ban-hang': { workflow: 'nhanh', effective: 'nhanh', note: null },
            'web-ban-hang-cu': { workflow: null, effective: 'mac-dinh', note: null },
          },
        },
      };
      const calls = [];
      const savedApi = ctx.AB.api;
      ctx.AB.api = function (method, url) {
        calls.push(method + ' ' + url);
        if (method === 'GET' && url === '/api/workflows') {
          if (resolveNext) {
            const gate = resolveNext;
            resolveNext = null;
            return gate.then(() => wfResponse);
          }
          return Promise.resolve(wfResponse);
        }
        return Promise.resolve({ ok: false, status: 404, data: null });
      };

      try {
        const c = factories.projects();
        check('projects: workflowsLoading starts true', c.workflowsLoading === true, `got ${c.workflowsLoading}`);
        check('projects: line before the list loads uses p.workflow', c.projectWorkflowLine({ id: 'x', workflow: 'nhanh' }) === 'Quy trình mặc định: nhanh', `got "${c.projectWorkflowLine({ id: 'x', workflow: 'nhanh' })}"`);
        check('projects: line before the list loads defaults to mac-dinh', c.projectWorkflowLine({ id: 'x', workflow: null }) === 'Quy trình mặc định: mac-dinh', `got "${c.projectWorkflowLine({ id: 'x', workflow: null })}"`);

        await c.loadWorkflows();
        check('projects: loadWorkflows fills workflowList', c.workflowList.length === 3 && c.workflowList[1].name === 'nhanh', `got ${c.workflowList.length}`);
        check('projects: hasWorkflowList', c.hasWorkflowList === true);
        check('projects: workflowsLoading false after load', c.workflowsLoading === false);
        check('projects: GET /api/workflows only (read-only)', calls.every((x) => x === 'GET /api/workflows'), `got ${JSON.stringify(calls)}`);

        check('projects: builtin chips', JSON.stringify(c.itemChips(WF_LIST[0])) === JSON.stringify(['có sẵn', 'autopilot']), `got ${JSON.stringify(c.itemChips(WF_LIST[0]))}`);
        check('projects: override chips', JSON.stringify(c.itemChips(WF_LIST[1])) === JSON.stringify(['ghi đè quy trình có sẵn', 'autopilot']), `got ${JSON.stringify(c.itemChips(WF_LIST[1]))}`);
        check('projects: file chips', JSON.stringify(c.itemChips(WF_LIST[2])) === JSON.stringify(['từ workflows.json', 'chạy tay']), `got ${JSON.stringify(c.itemChips(WF_LIST[2]))}`);
        check('projects: defaultForLine with projects', c.defaultForLine(WF_LIST[1]) === 'Mặc định của: Web bán hàng', `got "${c.defaultForLine(WF_LIST[1])}"`);
        check('projects: defaultForLine empty', c.defaultForLine(WF_LIST[0]) === '', `got "${c.defaultForLine(WF_LIST[0])}"`);
        check('projects: defaultForLine two projects', c.defaultForLine({ default_for: ['A', 'B'] }) === 'Mặc định của: A, B', `got "${c.defaultForLine({ default_for: ['A', 'B'] })}"`);

        const lineWeb = c.projectWorkflowLine({ id: 'web-ban-hang', workflow: 'nhanh' });
        check('projects: projectWorkflowLine uses project_defaults effective', lineWeb === 'Quy trình mặc định: nhanh', `got "${lineWeb}"`);
        const lineCu = c.projectWorkflowLine({ id: 'web-ban-hang-cu', workflow: null });
        check('projects: projectWorkflowLine without a default', lineCu === 'Quy trình mặc định: mac-dinh', `got "${lineCu}"`);
        const lineNew = c.projectWorkflowLine({ id: 'moi', workflow: null });
        check('projects: projectWorkflowLine for a project missing from project_defaults', lineNew === 'Quy trình mặc định: mac-dinh', `got "${lineNew}"`);

        check('projects: reloadLabel idle', c.reloadLabel === 'Đọc lại', `got "${c.reloadLabel}"`);
        let release;
        resolveNext = new Promise((r) => { release = r; });
        const pending = c.reloadWorkflows();
        check('projects: reloadWorkflows sets the in-flight label', c.reloadLabel === 'Đang đọc...', `got "${c.reloadLabel}"`);
        check('projects: reloadingWorkflows true while in flight', c.reloadingWorkflows === true);
        release();
        await pending;
        check('projects: reloadLabel back to idle', c.reloadLabel === 'Đọc lại' && c.reloadingWorkflows === false, `got "${c.reloadLabel}"`);

        // workflows.json errors: 12 lines → 10 + remainder, built-ins still listed
        const errs = [];
        for (let i = 1; i <= 12; i++) errs.push(`Dòng ${i}: lỗi ${i}`);
        wfResponse = { ok: true, status: 200, data: { workflows: WF_LIST.slice(0, 2), errors: errs, project_defaults: {} } };
        await c.reloadWorkflows();
        check('projects: hasWorkflowErrors', c.hasWorkflowErrors === true);
        check('projects: workflowErrorLines 11 lines', c.workflowErrorLines.length === 11, `got ${c.workflowErrorLines.length}`);
        check('projects: workflowErrorLines remainder', c.workflowErrorLines[10] === '… và 2 lỗi khác', `got "${c.workflowErrorLines[10]}"`);
        check('projects: built-ins listed with errors', c.workflowList.length === 2, `got ${c.workflowList.length}`);

        // fetch failure → rose callout body
        wfResponse = { ok: false, status: 500, data: { error: 'Lỗi nội bộ khi đọc danh sách quy trình' } };
        const c2 = factories.projects();
        await c2.loadWorkflows();
        check('projects: failed fetch sets workflowLoadError', c2.workflowLoadError === 'Lỗi nội bộ khi đọc danh sách quy trình', `got "${c2.workflowLoadError}"`);
        check('projects: hasWorkflowLoadError', c2.hasWorkflowLoadError === true);
        check('projects: rose callout body', c2.workflowErrorBody === 'Lỗi nội bộ khi đọc danh sách quy trình. Bấm Đọc lại để thử lại.', `got "${c2.workflowErrorBody}"`);
        check('projects: not pending after a failure', c2.workflowsPending === false);
        wfResponse = { ok: false, status: 0, data: null };
        await c2.reloadWorkflows();
        check('projects: offline body', c2.workflowErrorBody === 'Mất kết nối tới agent-brain serve. Bấm Đọc lại để thử lại.', `got "${c2.workflowErrorBody}"`);
        wfResponse = { ok: true, status: 200, data: { workflows: WF_LIST, errors: [], project_defaults: {} } };
        await c2.reloadWorkflows();
        check('projects: Đọc lại clears the error', c2.workflowLoadError === '' && c2.hasWorkflowLoadError === false && c2.workflowList.length === 3, `got "${c2.workflowLoadError}"`);
      } finally {
        ctx.AB.api = savedApi;
      }
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

      function flowCard(key, title, tag, state, extra) {
        return {
          key,
          title,
          tag,
          state,
          state_word: ctx.AB.stateWord(state),
          result: state === 'cho' ? 'chưa bắt đầu' : 'xong',
          counts: null,
          duration_ms: null,
          cost_usd: null,
          has_run: state !== 'cho',
          run_id: null,
          ...(extra || {}),
        };
      }

      function basePipe(groups, gate, currentCard) {
        return {
          task_id: 1,
          cycle: 1,
          current: null,
          cards: {
            start: { state: 'xong', state_word: 'xong', result: 'bạn đã giao', at_ms: fixedMs - 420000 },
          },
          ledger: { groups: [], total: { runs: 1, duration_ms: 420000, cost_usd: 2.73, cost_unknown: 0 } },
          workflow: { name: 'mac-dinh', summary: 'plan ⇄ review → Bạn → code ⇄ review → QA ⇄ sửa', auto: true, max_rounds: 3, budget_minutes: 60, budget_usd: 20, limits: '3 vòng · 60 phút · $20', loops: ['plan', 'build', 'qa'], stages: [] },
          groups,
          gate,
          current_card: currentCard,
          budget: null,
        };
      }

      const GATE_WAIT = {
        present: true,
        state: 'cho_ban',
        state_word: 'chờ bạn',
        result: 'chờ duyệt plan',
        at_ms: fixedMs,
        note_kind: 'auto_waiting',
        note: 'Autopilot dừng ở đây để bạn duyệt plan. Duyệt plan rồi autopilot tự chạy Code, Review code, QA và Sửa, tối đa 3 vòng mỗi bước, trong 60 phút hoặc $20.',
        derived: false,
      };

      // mac-dinh pipeline: plan [plan xong, plan_review sua_lai {1, 4}] round 2 cap 3; build and qa not run
      const P_LEAD = basePipe(
        [
          {
            key: 'plan',
            cards: [
              flowCard('plan', 'Lên plan', 'lead', 'xong', { result: 'plan đã commit', duration_ms: 420000, cost_usd: 2.73, run_id: 1 }),
              flowCard('plan_review', 'Review plan', 'plan-review', 'sua_lai', {
                result: 'sửa lại',
                counts: { blocking: 1, advisory: 4, specs: null, failing: null, not_automatable: null },
                duration_ms: 120000,
                cost_usd: 1.32,
                run_id: 2,
              }),
            ],
            round: 2,
            cap: 3,
            stopped: false,
            stop_word: null,
          },
          {
            key: 'build',
            cards: [flowCard('code', 'Code', 'dev', 'cho'), flowCard('code_review', 'Review code', 'code-review', 'cho')],
            round: null,
            cap: 3,
            stopped: false,
            stop_word: null,
          },
          {
            key: 'qa',
            cards: [flowCard('qa', 'QA', 'qa', 'cho'), flowCard('fix', 'Sửa', 'dev · fix', 'cho')],
            round: null,
            cap: 3,
            stopped: false,
            stop_word: null,
          },
        ],
        GATE_WAIT,
        'ban'
      );
      P_LEAD.current = 'ban';

      // Task with no runs: every card chưa bắt đầu, footers chưa chạy, no ring
      const P_EMPTY = basePipe(
        [
          { key: 'plan', cards: [flowCard('plan', 'Lên plan', 'lead', 'cho'), flowCard('plan_review', 'Review plan', 'plan-review', 'cho')], round: null, cap: 3, stopped: false, stop_word: null },
          { key: 'build', cards: [flowCard('code', 'Code', 'dev', 'cho'), flowCard('code_review', 'Review code', 'code-review', 'cho')], round: null, cap: 3, stopped: false, stop_word: null },
          { key: 'qa', cards: [flowCard('qa', 'QA', 'qa', 'cho'), flowCard('fix', 'Sửa', 'dev · fix', 'cho')], round: null, cap: 3, stopped: false, stop_word: null },
        ],
        { present: true, state: 'cho', state_word: 'chưa bắt đầu', result: 'chưa có plan', at_ms: null, note_kind: 'not_open', note: 'Cổng này mở khi plan đã commit và review plan xong.', derived: false },
        null
      );
      P_EMPTY.cycle = null;
      P_EMPTY.ledger = { groups: [], total: { runs: 0, duration_ms: null, cost_usd: null, cost_unknown: 0 } };

      // nhanh: single-card groups, no cap
      const P_NHANH = basePipe(
        [
          { key: 'plan', cards: [flowCard('plan', 'Lên plan', 'lead', 'xong', { duration_ms: 60000, cost_usd: 0.5 })], round: 1, cap: null, stopped: false, stop_word: null },
          { key: 'build', cards: [flowCard('code', 'Code', 'dev', 'cho')], round: null, cap: null, stopped: false, stop_word: null },
          { key: 'qa', cards: [flowCard('qa', 'QA', 'qa', 'cho')], round: null, cap: null, stopped: false, stop_word: null },
        ],
        GATE_WAIT,
        'ban'
      );

      // build loop stopped after a grant: round 4 / cap 3, stopped hết vòng, current card code_review
      const P_STOPPED = basePipe(
        [
          { key: 'plan', cards: [flowCard('plan', 'Lên plan', 'lead', 'xong'), flowCard('plan_review', 'Review plan', 'plan-review', 'duyet', { result: 'duyệt', counts: { blocking: 0, advisory: 2, specs: null, failing: null, not_automatable: null } })], round: 1, cap: 3, stopped: false, stop_word: null },
          { key: 'build', cards: [flowCard('code', 'Code', 'dev', 'xong'), flowCard('code_review', 'Review code', 'code-review', 'sua_lai', { result: 'sửa lại', counts: { blocking: 2, advisory: 1, specs: null, failing: null, not_automatable: null } })], round: 4, cap: 3, stopped: true, stop_word: 'hết vòng' },
          { key: 'qa', cards: [flowCard('qa', 'QA', 'qa', 'cho'), flowCard('fix', 'Sửa', 'dev · fix', 'cho')], round: null, cap: 3, stopped: false, stop_word: null },
        ],
        { present: true, state: 'xong', state_word: 'xong', result: 'đã duyệt plan', at_ms: fixedMs, note_kind: 'approved_explicit', note: null, derived: false },
        'code_review'
      );

      const c = factories.task();
      c.task = T_LEAD;
      c.pipe = P_LEAD;

      // groups and cards from the payload
      check('hasGroup(plan)', c.hasGroup('plan') === true, `got ${c.hasGroup('plan')}`);
      check('hasGroup(nope) false', c.hasGroup('nope') === false, `got ${c.hasGroup('nope')}`);
      const planKeys = c.groupCards('plan').map((x) => x.key);
      check('groupCards(plan) keys', JSON.stringify(planKeys) === JSON.stringify(['plan', 'plan_review']), `got ${JSON.stringify(planKeys)}`);
      check('groupCards(nope) empty', c.groupCards('nope').length === 0, `got ${c.groupCards('nope').length}`);
      check('groupFooter(plan)', c.groupFooter('plan') === '\u27F2\uFE0E vòng 2 / 3', `got "${c.groupFooter('plan')}"`);
      check('groupFooter(build) chưa chạy', c.groupFooter('build') === 'chưa chạy', `got "${c.groupFooter('build')}"`);
      check('groupFooter(nope) chưa chạy', c.groupFooter('nope') === 'chưa chạy', `got "${c.groupFooter('nope')}"`);

      const planCard = c.groupCards('plan')[0];
      const reviewCard = c.groupCards('plan')[1];
      const codeCard = c.groupCards('build')[0];
      check('flowCardCurrent(plan) false', c.flowCardCurrent(planCard) === false, `got "${c.flowCardCurrent(planCard)}"`);
      check('gateCurrent() step', c.gateCurrent() === 'step', `got "${c.gateCurrent()}"`);
      check(
        'flowAriaLabel mac-dinh',
        c.flowAriaLabel === 'Pipeline: Bắt đầu, Lên plan, Review plan, Bạn, Code, Review code, QA, Sửa',
        `got "${c.flowAriaLabel}"`
      );
      check('flowCardClass(review) has border-amber-300', c.flowCardClass(reviewCard).includes('border-amber-300'), `got "${c.flowCardClass(reviewCard)}"`);
      check('flowCardClass(review) has no ring-2', !c.flowCardClass(reviewCard).includes('ring-2'), `got "${c.flowCardClass(reviewCard)}"`);
      check('flowCardClass(code) dashed', c.flowCardClass(codeCard).includes('border-dashed border-slate-500'), `got "${c.flowCardClass(codeCard)}"`);
      check('gateClass has ring-2 ring-indigo-600', c.gateClass.includes('ring-2 ring-indigo-600'), `got "${c.gateClass}"`);
      check('gateClass has border-amber-300', c.gateClass.includes('border-amber-300'), `got "${c.gateClass}"`);
      check('flowCardSr(review)', c.flowCardSr(reviewCard) === 'sửa lại, ', `got "${c.flowCardSr(reviewCard)}"`);
      check('flowCardResult(review)', c.flowCardResult(reviewCard) === 'sửa lại', `got "${c.flowCardResult(reviewCard)}"`);
      check('flowCardGlyph(review) !', c.flowCardGlyph(reviewCard) === ctx.AB.stepGlyph('sua_lai'), `got "${c.flowCardGlyph(reviewCard)}"`);
      const rc = c.flowCardCounts(reviewCard);
      check('flowCardCounts(review)', JSON.stringify(rc) === JSON.stringify(['1 chặn', '4 góp ý']), `got ${JSON.stringify(rc)}`);
      check('flowCardHasCounts(review)', c.flowCardHasCounts(reviewCard) === true);
      check('flowCardHasCounts(plan) false', c.flowCardHasCounts(planCard) === false);
      check('countSep(0) false', c.countSep(0) === false);
      check('countSep(1) true', c.countSep(1) === true);
      check('flowCardMetrics(plan)', c.flowCardMetrics(planCard) === '7 phút · $2.73', `got "${c.flowCardMetrics(planCard)}"`);
      check('flowCardMetrics(code) —', c.flowCardMetrics(codeCard) === '—', `got "${c.flowCardMetrics(codeCard)}"`);
      check('groupFrameClass(plan) normal', c.groupFrameClass('plan') === 'ring-1 ring-slate-200 dark:ring-slate-700', `got "${c.groupFrameClass('plan')}"`);
      check('groupStopped(plan) false', c.groupStopped('plan') === false);
      check('groupStopWord(plan) empty', c.groupStopWord('plan') === '', `got "${c.groupStopWord('plan')}"`);

      // Bạn card
      check('gatePresent', c.gatePresent === true);
      check('gateState cho_ban', c.gateState === 'cho_ban', `got "${c.gateState}"`);
      check('gateResult', c.gateResult === 'chờ duyệt plan', `got "${c.gateResult}"`);
      check('gateSr', c.gateSr === 'chờ bạn, ', `got "${c.gateSr}"`);
      check('gateTime starts with từ ', c.gateTime.startsWith('từ '), `got "${c.gateTime}"`);
      check('gateNote visible text', c.gateNote.startsWith('Autopilot dừng ở đây để bạn duyệt plan.'), `got "${c.gateNote}"`);
      check('gateDerivedTitle false when explicit', c.gateDerivedTitle === false, `got "${c.gateDerivedTitle}"`);
      check('start card not current', c.cardCurrentAttr('start') === false);
      check('cardState(start) xong', c.cardState('start') === 'xong', `got "${c.cardState('start')}"`);
      check('cardResult(start)', c.cardResult('start') === 'bạn đã giao', `got "${c.cardResult('start')}"`);

      // exactly one current card, from current_card
      function ringed(comp) {
        const keys = [];
        for (const g of comp.flowGroups) {
          for (const card of g.cards) {
            if (comp.flowCardCurrent(card) === 'step') keys.push(card.key);
          }
        }
        if (comp.gateCurrent() === 'step') keys.push('ban');
        if (comp.cardCurrentAttr('start') === 'step') keys.push('start');
        return keys;
      }
      check('P_LEAD exactly one ring on ban', JSON.stringify(ringed(c)) === JSON.stringify(['ban']), `got ${JSON.stringify(ringed(c))}`);

      // empty task: no ring, every footer chưa chạy, Bạn chưa có plan
      c.pipe = P_EMPTY;
      check('P_EMPTY no ring', ringed(c).length === 0, `got ${JSON.stringify(ringed(c))}`);
      for (const k of ['plan', 'build', 'qa']) {
        check(`P_EMPTY groupFooter(${k})`, c.groupFooter(k) === 'chưa chạy', `got "${c.groupFooter(k)}"`);
      }
      check('P_EMPTY gateResult', c.gateResult === 'chưa có plan', `got "${c.gateResult}"`);
      check('P_EMPTY gateTime —', c.gateTime === '—', `got "${c.gateTime}"`);

      // nhanh: footer without cap; single-card groups
      c.pipe = P_NHANH;
      check('P_NHANH groupFooter(plan)', c.groupFooter('plan') === '\u27F2\uFE0E vòng 1', `got "${c.groupFooter('plan')}"`);
      check('P_NHANH flowAriaLabel', c.flowAriaLabel === 'Pipeline: Bắt đầu, Lên plan, Bạn, Code, QA', `got "${c.flowAriaLabel}"`);

      // stopped build group after a grant
      c.pipe = P_STOPPED;
      check('P_STOPPED groupFooter(build) 4 / 3', c.groupFooter('build') === '\u27F2\uFE0E vòng 4 / 3', `got "${c.groupFooter('build')}"`);
      check('P_STOPPED groupFrameClass(build) amber', c.groupFrameClass('build').includes('ring-2 ring-amber-500'), `got "${c.groupFrameClass('build')}"`);
      check('P_STOPPED groupFrameClass(plan) normal', c.groupFrameClass('plan').includes('ring-1 ring-slate-200'), `got "${c.groupFrameClass('plan')}"`);
      check('P_STOPPED groupStopped(build)', c.groupStopped('build') === true);
      check('P_STOPPED groupStopWord(build)', c.groupStopWord('build') === 'hết vòng', `got "${c.groupStopWord('build')}"`);
      check('P_STOPPED ring on code_review', JSON.stringify(ringed(c)) === JSON.stringify(['code_review']), `got ${JSON.stringify(ringed(c))}`);
      const pr = c.groupCards('plan')[1];
      check('P_STOPPED duyet card class emerald', c.flowCardClass(pr).includes('border-emerald-300'), `got "${c.flowCardClass(pr)}"`);
      check('P_STOPPED duyet counts known zero', JSON.stringify(c.flowCardCounts(pr)) === JSON.stringify(['0 chặn', '2 góp ý']), `got ${JSON.stringify(c.flowCardCounts(pr))}`);
      check('P_STOPPED gateTime is a clock', /^\d\d:\d\d$/.test(c.gateTime), `got "${c.gateTime}"`);
      check('P_STOPPED gateNote hidden', c.gateNote === '', `got "${c.gateNote}"`);

      // legacy derived gate keeps the 05.1 title
      c.pipe = basePipe(
        P_NHANH.groups,
        { present: true, state: 'xong', state_word: 'xong', result: 'đã duyệt plan', at_ms: fixedMs, note_kind: 'derived', note: 'Cổng duyệt suy ra từ dữ liệu: Dev đã chạy trên bản plan hiện tại thì coi là đã duyệt.', derived: true },
        'code'
      );
      check('derived gate title', c.gateDerivedTitle === 'Cổng duyệt suy ra từ dữ liệu: Dev đã chạy trên bản plan hiện tại thì coi là đã duyệt.', `got "${c.gateDerivedTitle}"`);

      // no pipeline: nothing drawn, nothing fabricated
      c.pipe = null;
      check('pipe=null flowGroups empty', c.flowGroups.length === 0);
      check('pipe=null gatePresent false', c.gatePresent === false);
      check('pipe=null gateNote empty', c.gateNote === '');
      check('pipe=null no ring', ringed(c).length === 0);

      // strip independent of /pipeline
      c.pipe = P_LEAD;
      check('stripCaption with T_LEAD', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);
      check('actionList length 2', c.actionList.length === 2, `got ${c.actionList.length}`);
      check('actionId second is ab-act-relead', c.actionId(c.actionList[1]) === 'ab-act-relead', `got "${c.actionId(c.actionList[1])}"`);
      c.pipe = null;
      c.pipeError = 'x';
      check('stripCaption with pipe=null', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);
      check('actionList length 2 with pipe=null', c.actionList.length === 2, `got ${c.actionList.length}`);
      c.pipeError = '';
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
    name: 'task-header',
    async run(ctx) {
      const AB = ctx.AB;

      // autoBadge: seven states, exact text and key
      const badgeCases = [
        [{ state: 'off' }, 'Autopilot: tắt', 'cho'],
        [{ state: 'running' }, 'Autopilot: đang chạy', 'dang_chay'],
        [{ state: 'stopping' }, 'Autopilot: đang dừng', 'dang_chay'],
        [{ state: 'gate' }, 'Autopilot: chờ bạn duyệt plan', 'cho_ban'],
        [{ state: 'stopped', stop_word: 'hết vòng' }, 'Autopilot: chờ bạn · hết vòng', 'cho_ban'],
        [{ state: 'lost' }, 'Autopilot: đã dừng', 'killed'],
        [{ state: 'done' }, 'Autopilot: xong', 'xong'],
      ];
      for (const [auto, text, key] of badgeCases) {
        const b = AB.autoBadge(auto);
        check(`autoBadge(${auto.state}) text`, b.text === text, `got "${b.text}"`);
        check(`autoBadge(${auto.state}) key`, b.key === key, `got "${b.key}"`);
      }
      check('autoBadge(null) off', AB.autoBadge(null).text === 'Autopilot: tắt', `got "${AB.autoBadge(null).text}"`);

      // budgetParts
      const bp1 = AB.budgetParts({ used_ms: 2880000, used_minutes: 48, limit_minutes: 60, used_usd: 16, limit_usd: 20, estimated: false, estimated_runs: 0, level: 'near' });
      check('budgetParts near text', bp1.text === 'Ngân sách: 48 / 60 phút · $16.00 / $20.00', `got "${bp1.text}"`);
      check('budgetParts near level', bp1.level === 'sắp hết', `got "${bp1.level}"`);
      check('budgetParts near note empty', bp1.note === '', `got "${bp1.note}"`);
      const bp2 = AB.budgetParts({ used_ms: 3700000, used_minutes: 62, limit_minutes: 60, used_usd: 21.5, limit_usd: 20, estimated: true, estimated_runs: 2, level: 'over' });
      check('budgetParts estimated has ≈ $', bp2.text.includes('≈ $'), `got "${bp2.text}"`);
      check('budgetParts over level', bp2.level === 'đã hết', `got "${bp2.level}"`);
      check('budgetParts estimated note', bp2.note.startsWith('Gồm chi phí ước tính từ token cho 2 run'), `got "${bp2.note}"`);
      const bp3 = AB.budgetParts({ used_ms: 0, used_minutes: 0, limit_minutes: 60, used_usd: 0, limit_usd: 20, estimated: false, estimated_runs: 0, level: 'ok' });
      check('budgetParts ok level empty', bp3.level === '', `got "${bp3.level}"`);
      check('budgetParts(null) empty', AB.budgetParts(null).text === '', `got "${AB.budgetParts(null).text}"`);

      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };
      const taskJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/task.js'), 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const SUMMARY = 'plan ⇄ review → Bạn → code ⇄ review → QA ⇄ sửa';
      const AUTO_START = { id: 'auto_start', label: 'Bật autopilot', enabled: true, reason: null, run_id: null };
      const AUTO_STOP = { id: 'auto_stop', label: 'Dừng autopilot', enabled: true, reason: null, run_id: null };
      const T = {
        id: 1,
        slug: 'sua-app',
        status: 'planned',
        current_step: 'dev',
        actions: [{ id: 'run_dev', label: 'Chạy Dev', enabled: true, reason: null }],
        controls: [
          { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null },
          { id: 'return_lead', label: 'Trả về Lead', enabled: true, reason: null, run_id: null },
          AUTO_START,
        ],
        auto: { state: 'off', legacy: false, workflow_auto: true, stop_reason: null, stop_word: null, stop_detail: null, stopped_in: null, stop_requested: false },
        workflow: { name: 'mac-dinh', summary: SUMMARY, auto: true },
        rev: 'rev-1',
        steps: [],
        runs: [],
      };
      const WF = { name: 'mac-dinh', summary: SUMMARY, auto: true, max_rounds: 3, budget_minutes: 60, budget_usd: 20, limits: '3 vòng · 60 phút · $20', loops: ['plan', 'build', 'qa'], stages: [] };

      const c = factories.task();
      c.id = '1';
      c.task = T;
      c.pipe = { workflow: WF, groups: [], gate: null, current_card: 'ban', budget: null, cards: {}, ledger: { groups: [], total: null } };

      check('workflowLine mac-dinh', c.workflowLine === 'Quy trình: mac-dinh · ' + SUMMARY, `got "${c.workflowLine}"`);
      c.task = { ...T, workflow: null };
      check(
        'workflowLine legacy',
        c.workflowLine === 'Quy trình: chưa có (task tạo trước Phase 5.2) — chạy tay từng bước.',
        `got "${c.workflowLine}"`
      );
      c.task = T;

      const hc = c.headerControls.map((x) => x.id);
      check('headerControls only auto_start / auto_stop', JSON.stringify(hc) === JSON.stringify(['auto_start']), `got ${JSON.stringify(hc)}`);
      c.task = { ...T, controls: [AUTO_STOP], auto: { ...T.auto, state: 'running' } };
      check('headerControls auto_stop while running', JSON.stringify(c.headerControls.map((x) => x.id)) === JSON.stringify(['auto_stop']));
      c.task = T;

      check(
        'autoHelper enabled auto_start with mac-dinh limits',
        c.autoHelper(AUTO_START) === 'Giới hạn: 3 vòng mỗi bước lặp, 60 phút hoặc $20 cho cả task. Autopilot luôn dừng ở bước bạn duyệt plan.',
        `got "${c.autoHelper(AUTO_START)}"`
      );
      c.pipe = { ...c.pipe, workflow: { ...WF, loops: [], budget_usd: 0.05 } };
      check(
        'autoHelper without loops, cents',
        c.autoHelper(AUTO_START) === 'Giới hạn: 60 phút hoặc $0.05 cho cả task. Autopilot luôn dừng ở bước bạn duyệt plan.',
        `got "${c.autoHelper(AUTO_START)}"`
      );
      c.pipe = { ...c.pipe, workflow: WF };
      check('autoHelper disabled auto_start empty', c.autoHelper({ ...AUTO_START, enabled: false, reason: 'x' }) === '', `got "${c.autoHelper({ ...AUTO_START, enabled: false })}"`);
      c.pipe = null;
      check('autoHelper without pipeline empty (no fabricated limits)', c.autoHelper(AUTO_START) === '', `got "${c.autoHelper(AUTO_START)}"`);
      c.task = { ...T, controls: [AUTO_STOP], auto: { ...T.auto, state: 'running' } };
      check('autoHelper running', c.autoHelper(AUTO_STOP) === 'Run hiện tại chạy nốt rồi autopilot dừng. Muốn dừng ngay: Kill.', `got "${c.autoHelper(AUTO_STOP)}"`);
      check('autoBadgeText running', c.autoBadgeText === 'Autopilot: đang chạy', `got "${c.autoBadgeText}"`);
      check('autoBadgeClass running blue', c.autoBadgeClass === AB.stepClass('dang_chay'), `got "${c.autoBadgeClass}"`);
      c.task = { ...T, auto: { ...T.auto, state: 'lost' } };
      check(
        'autoHelper lost',
        c.autoHelper(AUTO_START) === 'Autopilot đã dừng vì agent-brain serve tắt giữa chừng. Bật autopilot để chạy tiếp từ bước dở.',
        `got "${c.autoHelper(AUTO_START)}"`
      );
      check('driverLost', c.driverLost === true);
      check('headerHelper lost', c.headerHelper.startsWith('Autopilot đã dừng vì agent-brain serve'), `got "${c.headerHelper}"`);
      c.task = T;

      // budget getters read the pipeline payload
      c.pipe = { workflow: WF, groups: [], gate: null, current_card: null, budget: { used_ms: 2880000, used_minutes: 48, limit_minutes: 60, used_usd: 16, limit_usd: 20, estimated: false, estimated_runs: 0, level: 'near' }, cards: {}, ledger: { groups: [], total: null } };
      check('hasBudget', c.hasBudget === true);
      check('budgetText', c.budgetText === 'Ngân sách: 48 / 60 phút · $16.00 / $20.00', `got "${c.budgetText}"`);
      check('budgetLevel', c.budgetLevel === 'sắp hết', `got "${c.budgetLevel}"`);
      c.pipe = null;
      check('hasBudget false without pipeline', c.hasBudget === false);

      // strip caption follows current_card
      for (const [key, title] of [['plan', 'Lên plan'], ['plan_review', 'Review plan'], ['ban', 'Bạn'], ['code', 'Code'], ['code_review', 'Review code'], ['qa', 'QA'], ['fix', 'Sửa']]) {
        c.pipe = { current_card: key, groups: [], gate: null };
        check(`stripCaption current_card ${key}`, c.stripCaption === 'Bước hiện tại: ' + title, `got "${c.stripCaption}"`);
      }
      c.pipe = { current_card: null, groups: [], gate: null };
      check('stripCaption fallback current_step', c.stripCaption === 'Bước hiện tại: Bạn', `got "${c.stripCaption}"`);

      // in-flight labels
      c.busyAction = 'auto_start';
      check('actionText auto_start in flight', c.actionText(AUTO_START) === 'Đang bật...', `got "${c.actionText(AUTO_START)}"`);
      c.busyAction = 'auto_stop';
      check('actionText auto_stop in flight', c.actionText(AUTO_STOP) === 'Đang dừng...', `got "${c.actionText(AUTO_STOP)}"`);
      c.busyAction = '';

      // postControl
      const posts = [];
      let nextRes = null;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };
      let loads = 0;
      c.load = function () {
        loads++;
      };
      c.task = T;

      nextRes = { ok: true, status: 202, data: { job_id: 7 } };
      c.onAction(AUTO_START);
      check('postControl busy while in flight', c.busyAction === 'auto_start', `got "${c.busyAction}"`);
      await Promise.resolve();
      await Promise.resolve();
      check('postControl auto_start POST path', posts.length === 1 && posts[0].method === 'POST' && posts[0].url === '/api/tasks/1/auto', JSON.stringify(posts));
      check('postControl body carries rev', posts[0] && posts[0].body && posts[0].body.rev === 'rev-1', JSON.stringify(posts[0]));
      check('postControl 202 message', c.message === 'Đã bật autopilot.', `got "${c.message}"`);
      check('postControl 202 reloads', loads === 1, `loads=${loads}`);
      check('postControl clears busy', c.busyAction === '', `got "${c.busyAction}"`);

      nextRes = { ok: true, status: 200, data: { ok: true } };
      c.onAction(AUTO_STOP);
      await Promise.resolve();
      await Promise.resolve();
      check('postControl auto_stop path', posts[1] && posts[1].url === '/api/tasks/1/auto/stop', JSON.stringify(posts[1]));
      check('postControl auto_stop message', c.message === 'Đã yêu cầu dừng autopilot — run hiện tại chạy nốt rồi dừng.', `got "${c.message}"`);
      check('postControl auto_stop reloads', loads === 2, `loads=${loads}`);

      nextRes = { ok: false, status: 409, data: { error: 'Dữ liệu đã cũ — tải lại rồi thử lại', kind: 'stale' } };
      c.onAction(AUTO_START);
      await Promise.resolve();
      await Promise.resolve();
      check('postControl 409 stale message', c.message === 'Dữ liệu đã cũ — tải lại rồi thử lại', `got "${c.message}"`);
      check('postControl 409 stale reloads at once', loads === 3, `loads=${loads}`);

      nextRes = { ok: false, status: 409, data: { error: 'Dự án đang bận', kind: 'busy' } };
      c.onAction(AUTO_START);
      await Promise.resolve();
      await Promise.resolve();
      check('postControl 409 busy message', c.message === 'Dự án đang bận', `got "${c.message}"`);
      check('postControl 409 busy no reload', loads === 3, `loads=${loads}`);

      // 05.1 WR-04: a request busy for 10 s or more is superseded
      const pipeCalls = [];
      ctx.AB.api = function (method, url) {
        pipeCalls.push(url);
        return new Promise(() => {});
      };
      const w = factories.task();
      w.id = '1';
      w._pipeBusy = true;
      w._pipeStarted = Date.now() - 1000;
      w._pipeAgain = false;
      w.loadPipeline();
      check('WR-04 busy 1 s: no new request', pipeCalls.length === 0, `got ${pipeCalls.length}`);
      check('WR-04 busy 1 s: sets _pipeAgain', w._pipeAgain === true);
      check('WR-04 busy 1 s: not stale', w.pipeStale === false);

      w._pipeAgain = false;
      w._pipeStarted = Date.now() - 11000;
      const seqBefore = w._pipeSeq;
      w.loadPipeline();
      check('WR-04 busy 11 s: new request started', pipeCalls.length === 1, `got ${pipeCalls.length}`);
      check('WR-04 busy 11 s: pipeStale true', w.pipeStale === true);
      check('WR-04 busy 11 s: seq advanced', w._pipeSeq === seqBefore + 1, `got ${w._pipeSeq}`);
      check('WR-04 busy 11 s: _pipeStarted renewed', Date.now() - w._pipeStarted < 1000, `got ${w._pipeStarted}`);

      // a superseded response is discarded and does not clear the newer request's busy flag
      let resolvers = [];
      ctx.AB.api = function () {
        return new Promise((res) => resolvers.push(res));
      };
      const z = factories.task();
      z.id = '1';
      z.loadPipeline();
      z._pipeStarted = Date.now() - 11000;
      z.loadPipeline();
      check('WR-04 two requests in flight', resolvers.length === 2, `got ${resolvers.length}`);
      resolvers[0]({ ok: true, status: 200, data: { old: true } });
      await Promise.resolve();
      await Promise.resolve();
      check('WR-04 old response discarded', z.pipe === null, 'pipe set from superseded response');
      check('WR-04 newer request still busy', z._pipeBusy === true);
      const fresh = { fresh: true };
      resolvers[1]({ ok: true, status: 200, data: fresh });
      await Promise.resolve();
      await Promise.resolve();
      check('WR-04 latest response applied', z.pipe === fresh, 'pipe not from the latest response');
      check('WR-04 latest clears stale', z.pipeStale === false);
      check('WR-04 latest clears busy', z._pipeBusy === false);
    },
  },
  {
    name: 'task-gate-actions',
    async run(ctx) {
      const AB = ctx.AB;
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };
      const taskJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/task.js'), 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const PRIMARY = 'bg-indigo-600';
      const APPROVE = { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null };
      const RETURN = { id: 'return_lead', label: 'Trả về Lead', enabled: true, reason: null, run_id: null };
      const AUTO_START_OFF = { id: 'auto_start', label: 'Bật autopilot', enabled: false, reason: 'x', run_id: null };
      const RUN_DEV = { id: 'run_dev', label: 'Chạy Dev', enabled: true, reason: null };
      const RELEAD = { id: 'relead', label: 'Chạy lại Lead', enabled: true, reason: null };
      const T = {
        id: 1,
        slug: 'sua-app',
        status: 'planned',
        current_step: 'dev',
        actions: [RUN_DEV, RELEAD],
        controls: [APPROVE, RETURN, AUTO_START_OFF],
        auto: { state: 'gate', workflow_auto: true, stop_reason: null, stop_word: null, stop_detail: null, stopped_in: null, stop_requested: false },
        workflow: { name: 'mac-dinh', summary: 'plan', auto: true },
        rev: 'R',
        steps: [],
        runs: [],
      };
      const GATE = { present: true, state: 'cho_ban', state_word: 'chờ bạn', result: 'chờ duyệt plan', at_ms: Date.now(), note: '', derived: false };
      const PIPE = { workflow: null, groups: [], gate: GATE, current_card: 'ban', budget: null, cards: {}, ledger: { groups: [], total: null } };

      const c = factories.task();
      c.id = '1';
      c.task = T;
      c.pipe = PIPE;

      // gateControls / strip
      check('gateControls ids', JSON.stringify(c.gateControls.map((x) => x.id)) === JSON.stringify(['approve', 'return_lead']), `got ${JSON.stringify(c.gateControls.map((x) => x.id))}`);
      check('hasGateControls', c.hasGateControls === true);
      check('stripActions without gate controls', JSON.stringify(c.stripActions.map((x) => x.id)) === JSON.stringify(['run_dev', 'relead']), `got ${JSON.stringify(c.stripActions.map((x) => x.id))}`);
      check('actionClass(approve) primary', c.actionClass(APPROVE).includes(PRIMARY), `got "${c.actionClass(APPROVE)}"`);
      check('actionClass(return_lead) secondary', !c.actionClass(RETURN).includes(PRIMARY) && c.actionClass(RETURN).includes('ring-slate-300'), `got "${c.actionClass(RETURN)}"`);
      check('approve outranks the strip first enabled', !c.actionClass(RUN_DEV).includes(PRIMARY), `got "${c.actionClass(RUN_DEV)}"`);

      c.pipe = null;
      c.pipeError = 'Lỗi 500';
      const failedIds = c.stripActions.map((x) => x.id);
      check('pipeFailed: gateControls empty', c.gateControls.length === 0);
      check('pipeFailed: strip has approve and return_lead', failedIds.includes('approve') && failedIds.includes('return_lead'), `got ${JSON.stringify(failedIds)}`);
      check('pipeFailed: actionClass(approve) still primary', c.actionClass(APPROVE).includes(PRIMARY));
      check('pipeFailed: run_dev secondary', !c.actionClass(RUN_DEV).includes(PRIMARY));
      c.pipeError = '';
      check('pipe loading: gate controls drawn nowhere', c.gateControls.length === 0 && !c.stripActions.some((x) => x.id === 'approve'));
      c.task = { ...T, controls: [AUTO_START_OFF] };
      c.pipe = PIPE;
      check('no gate: first enabled strip action primary', c.actionClass(RUN_DEV).includes(PRIMARY), `got "${c.actionClass(RUN_DEV)}"`);
      c.task = T;

      // stubs
      const posts = [];
      let nextRes = null;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };
      const focusCalls = [];
      ctx.AB.focusLater = function (id, fallback) {
        focusCalls.push({ id, fallback });
      };
      ctx.AB.lockScroll = function () {};
      let loads = 0;
      c.load = function () {
        loads++;
      };
      const settle = async () => {
        await Promise.resolve();
        await Promise.resolve();
      };

      // approve: no dialog, POST with rev
      const approvedAt = Date.now();
      nextRes = { ok: true, status: 200, data: { approved_at: approvedAt, plan_commit: 'abc', job_id: 3 } };
      c.onAction(APPROVE);
      check('approve: no dialog', c.dialog === null);
      check('approve: busy while in flight', c.busyAction === 'approve');
      check('approve: in-flight label', c.actionText(APPROVE) === 'Đang duyệt...', `got "${c.actionText(APPROVE)}"`);
      check('approve: every action disabled while in flight', c.actionDisabled(RUN_DEV) && c.actionDisabled(RETURN));
      await settle();
      check('approve: POST path', posts.length === 1 && posts[0].method === 'POST' && posts[0].url === '/api/tasks/1/approve', JSON.stringify(posts));
      check('approve: body is {rev}', posts[0] && JSON.stringify(posts[0].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(posts[0]));
      check('approve: 200 message', c.message === 'Đã duyệt plan lúc ' + AB.fmtClock(approvedAt) + '.', `got "${c.message}"`);
      check('approve: 200 reloads', loads === 1, `loads=${loads}`);
      check('approve: busy cleared', c.busyAction === '');

      nextRes = { ok: false, status: 409, data: { error: 'Plan hiện tại đã được duyệt lúc 10:00 — không duyệt lại', kind: 'duplicate' } };
      c.onAction(APPROVE);
      await settle();
      check('approve: 409 duplicate message', c.message === 'Plan hiện tại đã được duyệt lúc 10:00 — không duyệt lại', `got "${c.message}"`);
      check('approve: 409 not retried', posts.length === 2, `posts=${posts.length}`);
      await settle();
      check('approve: 409 still not retried later', posts.length === 2, `posts=${posts.length}`);
      check('approve: 409 duplicate no reload', loads === 1, `loads=${loads}`);

      // a second click while one is in flight sends nothing
      let hold;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return new Promise((r) => {
          hold = r;
        });
      };
      c.onAction(APPROVE);
      c.postControl(APPROVE);
      check('in flight: second click sends nothing', posts.length === 3, `posts=${posts.length}`);
      hold({ ok: false, status: 500, data: null });
      await settle();
      check('500: errorText message', c.message === 'HTTP 500', `got "${c.message}"`);
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };

      // Trả về Lead: dialog with note
      c.returnNote = 'cũ';
      c.onAction(RETURN);
      check('return: opens dialog', c.dialog && c.dialog.id === 'return_lead');
      check('return: dialogIsReturn', c.dialogIsReturn === true);
      check('return: note reset', c.returnNote === '', `got "${c.returnNote}"`);
      check('return: focuses ab-return-note', focusCalls.length > 0 && focusCalls[focusCalls.length - 1].id === 'ab-return-note', JSON.stringify(focusCalls));
      check('return: lastFocusId is the trigger', c.lastFocusId === 'ab-act-return_lead', `got "${c.lastFocusId}"`);
      check('return: no POST on open', posts.length === 3, `posts=${posts.length}`);
      check('return: dialogTitle', c.dialogTitle === 'Trả plan về Lead cho task sua-app?', `got "${c.dialogTitle}"`);
      const AUTO_TAIL = ' Autopilot sẽ chạy lại vòng plan ⇄︎ review plan.';
      check('return: dialogBody auto tail', c.dialogBody.endsWith(AUTO_TAIL), `got "${c.dialogBody}"`);
      check('return: dialogBody starts with the relead text', c.dialogBody.startsWith('Lead sẽ lập lại .plan.md trên nhánh task/sua-app và commit \'sua-app: plan\'.'), `got "${c.dialogBody}"`);
      check('return: ok label', c.dialogOkLabel === 'Trả về Lead', `got "${c.dialogOkLabel}"`);
      check('return: cancel label', c.dialogCancelLabel === 'Giữ plan hiện tại', `got "${c.dialogCancelLabel}"`);
      check('return: ok class primary', c.dialogOkClass.includes(PRIMARY), `got "${c.dialogOkClass}"`);
      c.task = { ...T, workflow: { name: 'nhanh', summary: 'plan → Bạn → code → QA', auto: false } };
      check('return: nhanh has no auto tail', !c.dialogBody.includes('Autopilot'), `got "${c.dialogBody}"`);
      c.task = T;

      // whitespace-only note -> no note key
      c.returnNote = '   ';
      nextRes = { ok: true, status: 202, data: { job_id: 7 } };
      c.dialogOk();
      check('return: dialog closed on ok', c.dialog === null);
      await settle();
      check('return: POST path', posts[3] && posts[3].url === '/api/tasks/1/return', JSON.stringify(posts[3]));
      check('return: blank note sent as no note', posts[3] && JSON.stringify(posts[3].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(posts[3]));
      check('return: 202 message', c.message === 'Đã trả plan về Lead (job 7).', `got "${c.message}"`);

      c.onAction(RETURN);
      c.returnNote = '  tách bước 2 ';
      c.dialogOk();
      await settle();
      check('return: trimmed note', posts[4] && posts[4].body && posts[4].body.note === 'tách bước 2' && posts[4].body.rev === 'R', JSON.stringify(posts[4]));

      // cancel sends nothing
      c.onAction(RETURN);
      c.dialogCancel();
      check('return: cancel sends nothing', posts.length === 5, `posts=${posts.length}`);
      check('return: cancel focuses the trigger', focusCalls[focusCalls.length - 1].id === 'ab-act-return_lead');

      // stale -> reload at once
      const loadsBefore = loads;
      nextRes = { ok: false, status: 409, data: { error: 'Dữ liệu đã cũ — tải lại rồi thử lại', kind: 'stale' } };
      c.onAction(RETURN);
      c.dialogOk();
      await settle();
      check('return: 409 stale message', c.message === 'Dữ liệu đã cũ — tải lại rồi thử lại', `got "${c.message}"`);
      check('return: 409 stale reloads at once', loads === loadsBefore + 1, `loads=${loads}`);

      // long note refused by the server (422)
      nextRes = { ok: false, status: 422, data: { error: 'Ghi chú quá dài (tối đa 500 ký tự)' } };
      c.onAction(RETURN);
      c.returnNote = 'x';
      c.dialogOk();
      await settle();
      check('return: 422 shows server error', c.message === 'Ghi chú quá dài (tối đa 500 ký tự)', `got "${c.message}"`);
      check('return: in-flight label', (() => { c.busyAction = 'return_lead'; const t = c.actionText(RETURN); c.busyAction = ''; return t; })() === 'Đang giao...');
    },
  },
  {
    name: 'task-banner',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };
      const taskJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/task.js'), 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const PRIMARY = 'bg-indigo-600';
      const D = 'Review code vẫn yêu cầu sửa lại sau 3 / 3 vòng.';
      const MORE = { id: 'auto_more_round', label: 'Chạy thêm 1 vòng', enabled: true, reason: null, run_id: null };
      const END = { id: 'auto_end', label: 'Dừng hẳn', enabled: true, reason: null, run_id: null };
      const START = { id: 'auto_start', label: 'Bật autopilot', enabled: true, reason: null, run_id: null };
      const STOP = { id: 'auto_stop', label: 'Dừng autopilot', enabled: true, reason: null, run_id: null };
      const APPROVE = { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null };
      const RETURN = { id: 'return_lead', label: 'Trả về Lead', enabled: true, reason: null, run_id: null };
      const RUN_DEV = { id: 'run_dev', label: 'Chạy Dev', enabled: true, reason: null };
      const RELEAD = { id: 'relead', label: 'Chạy lại Lead', enabled: true, reason: null };
      const KILL = { id: 'kill', label: 'Kill', enabled: true, reason: null, run_id: 9 };
      const STOPPED = { state: 'stopped', workflow_auto: true, stop_reason: 'het_vong', stop_word: 'hết vòng', stop_detail: D, stopped_in: 'build', stop_requested: false };
      const T = {
        id: 1,
        slug: 'dung-lai',
        status: 'planned',
        current_step: 'dev',
        actions: [RUN_DEV, RELEAD],
        controls: [START, MORE, END],
        auto: STOPPED,
        workflow: { name: 'mac-dinh', summary: 'plan', auto: true },
        rev: 'R',
        steps: [],
        runs: [],
      };
      const BUDGET = { used_ms: 3700000, used_minutes: 62, limit_minutes: 60, used_usd: 21.5, limit_usd: 20, estimated: false, estimated_runs: 0, level: 'over' };
      const PIPE = { workflow: null, groups: [], gate: { present: true, state: 'xong', result: 'đã duyệt plan', at_ms: Date.now() }, current_card: 'code_review', budget: BUDGET, cards: {}, ledger: { groups: [], total: null } };

      const c = factories.task();
      c.id = '1';
      c.task = T;
      c.pipe = PIPE;

      check('banner: showBanner when stopped', c.showBanner === true);
      check('banner: heading', c.bannerHeading === 'Autopilot dừng · hết vòng', `got "${c.bannerHeading}"`);
      check('banner: body is stop_detail verbatim', c.bannerBody === D, `got "${c.bannerBody}"`);
      check('banner: controls order', JSON.stringify(c.bannerControls.map((x) => x.id)) === JSON.stringify(['auto_more_round', 'relead', 'auto_end']), `got ${JSON.stringify(c.bannerControls.map((x) => x.id))}`);
      check('banner: strip excludes relead', JSON.stringify(c.stripActions.map((x) => x.id)) === JSON.stringify(['run_dev']), `got ${JSON.stringify(c.stripActions.map((x) => x.id))}`);
      check('banner: no budget repeat for hết vòng', c.bannerBudget === '' && c.moreRoundHelp(MORE) === '', `got "${c.bannerBudget}" / "${c.moreRoundHelp(MORE)}"`);
      check('banner: more-round primary (no gate)', c.actionClass(MORE).includes(PRIMARY), `got "${c.actionClass(MORE)}"`);
      check('banner: relead secondary', !c.actionClass(RELEAD).includes(PRIMARY), `got "${c.actionClass(RELEAD)}"`);
      check('banner: end secondary', !c.actionClass(END).includes(PRIMARY), `got "${c.actionClass(END)}"`);
      check('banner: strip first enabled not primary while banner primary shows', !c.actionClass(RUN_DEV).includes(PRIMARY), `got "${c.actionClass(RUN_DEV)}"`);

      // hết ngân sách: helper under Chạy thêm 1 vòng, budget line repeated
      c.task = { ...T, auto: { ...STOPPED, stop_reason: 'het_ngan_sach', stop_word: 'hết ngân sách', stopped_in: null } };
      check('budget: moreRoundHelp', c.moreRoundHelp(MORE) === 'Chạy vượt ngân sách một lần, chỉ cho vòng này.', `got "${c.moreRoundHelp(MORE)}"`);
      check('budget: moreRoundHelp only for more-round', c.moreRoundHelp(END) === '');
      check('budget: bannerBudget repeats budgetText', c.bannerBudget === c.budgetText && c.bannerBudget.startsWith('Ngân sách: 62 / 60 phút'), `got "${c.bannerBudget}"`);

      // gate shown: Duyệt plan is the only primary
      c.task = { ...T, controls: [APPROVE, RETURN, START, MORE, END] };
      check('gate + banner: approve primary', c.actionClass(APPROVE).includes(PRIMARY));
      check('gate + banner: more-round secondary', !c.actionClass(MORE).includes(PRIMARY), `got "${c.actionClass(MORE)}"`);

      // running: no banner, relead back in the strip
      c.task = { ...T, controls: [STOP], auto: { ...STOPPED, state: 'running', stop_reason: null, stop_word: null, stop_detail: null, stopped_in: null } };
      check('running: showBanner false', c.showBanner === false);
      check('running: bannerControls empty', c.bannerControls.length === 0);
      check('running: strip has relead', c.stripActions.some((x) => x.id === 'relead'));
      check('running: strip first enabled primary', c.actionClass(RUN_DEV).includes(PRIMARY), `got "${c.actionClass(RUN_DEV)}"`);

      // colours: red stays Kill only
      for (const a of [APPROVE, MORE, END, STOP]) {
        check(`colour: actionClass(${a.id}) has no rose`, !c.actionClass(a).includes('rose'), `got "${c.actionClass(a)}"`);
      }
      check('colour: actionClass(kill) bg-rose-600', c.actionClass(KILL).includes('bg-rose-600'), `got "${c.actionClass(KILL)}"`);

      // D-03 callouts hidden for the same cause
      c.task = { ...T, status: 'blocked', auto: { ...STOPPED, stop_reason: 'vuong_mac', stop_word: 'vướng mắc', stopped_in: 'build' } };
      check('D-03: blocked hidden under vướng mắc banner', c.isBlocked === false);
      check('D-03: plan section keeps its amber ring', c.sectionRing('Vướng mắc').includes('ring-amber-500'), `got "${c.sectionRing('Vướng mắc')}"`);
      c.task = { ...T, status: 'blocked', auto: { state: 'off', stop_reason: null } };
      check('D-03: blocked shown without banner', c.isBlocked === true);
      c.task = { ...T, status: 'qa_fix', auto: { ...STOPPED, stop_reason: 'het_vong', stopped_in: 'qa', stop_detail: 'QA vẫn kết luận Cần sửa sau 3 / 3 vòng. Xem báo cáo QA bên dưới.' } };
      check('D-03: qa_fix hidden under QA hết vòng banner', c.isQaFix === false);
      check('D-03: QA card keeps its amber ring', c.qaCardRing.includes('ring-amber-500'), `got "${c.qaCardRing}"`);
      c.task = { ...T, status: 'qa_fix', auto: { ...STOPPED, stop_reason: 'het_vong', stopped_in: 'build' } };
      check('D-03: qa_fix shown for a build-loop stop', c.isQaFix === true);
      c.task = { ...T, status: 'qa_fix', auto: { state: 'off' } };
      check('D-03: qa_fix shown without banner', c.isQaFix === true);

      // more-round / end
      const posts = [];
      let nextRes = null;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };
      ctx.AB.focusLater = function () {};
      ctx.AB.lockScroll = function () {};
      let loads = 0;
      c.load = function () {
        loads++;
      };
      const settle = async () => {
        await Promise.resolve();
        await Promise.resolve();
      };
      c.task = T;

      let hold;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return new Promise((r) => {
          hold = r;
        });
      };
      c.onAction(MORE);
      check('more-round: no dialog', c.dialog === null);
      check('more-round: in-flight label', c.actionText(MORE) === 'Đang giao...', `got "${c.actionText(MORE)}"`);
      for (const a of [MORE, END, RELEAD, RUN_DEV, START]) {
        check(`in flight: ${a.id} disabled`, c.actionDisabled(a) === true);
      }
      hold({ ok: true, status: 202, data: { job_id: 11 } });
      await settle();
      check('more-round: POST path', posts[0] && posts[0].url === '/api/tasks/1/auto/more-round', JSON.stringify(posts[0]));
      check('more-round: body {rev}', posts[0] && JSON.stringify(posts[0].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(posts[0]));
      check('more-round: 202 message', c.message === 'Đã giao thêm 1 vòng.', `got "${c.message}"`);
      check('more-round: reloads', loads === 1, `loads=${loads}`);

      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };
      nextRes = { ok: true, status: 200, data: { ok: true } };
      c.busyAction = 'auto_end';
      check('end: in-flight label', c.actionText(END) === 'Đang dừng...', `got "${c.actionText(END)}"`);
      c.busyAction = '';
      c.onAction(END);
      check('end: no dialog', c.dialog === null);
      await settle();
      check('end: POST path', posts[1] && posts[1].url === '/api/tasks/1/auto/end', JSON.stringify(posts[1]));
      check('end: body {rev}', posts[1] && JSON.stringify(posts[1].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(posts[1]));
      check('end: 200 message', c.message === 'Đã dừng hẳn autopilot. Bật autopilot để chạy tiếp.', `got "${c.message}"`);

      nextRes = { ok: false, status: 409, data: { error: 'Autopilot không ở trạng thái dừng', kind: 'duplicate' } };
      c.onAction(MORE);
      await settle();
      check('more-round: 409 message', c.message === 'Autopilot không ở trạng thái dừng', `got "${c.message}"`);
      check('more-round: 409 not retried', posts.length === 3, `posts=${posts.length}`);

      // banner relead keeps the confirmation dialog
      c.onAction(RELEAD);
      check('banner relead opens the dialog', c.dialog === RELEAD);
      check('banner relead sends nothing yet', posts.length === 3);
      c.dialogCancel();

      // Kill dialog: role of the killed run, autopilot sentence
      c.task = { ...T, actions: [KILL], auto: { state: 'running' }, runs: [{ id: 8, role: 'dev' }, { id: 9, role: 'code-review' }], steps: [{ role: 'dev', state: 'dang_chay' }] };
      c.dialog = KILL;
      check('kill: title names the killed run role', c.dialogTitle === 'Kill run 9 (Review code)?', `got "${c.dialogTitle}"`);
      check('kill: autopilot sentence while running', c.dialogBody.endsWith(' Autopilot cũng dừng với lý do "dừng bởi bạn".'), `got "${c.dialogBody}"`);
      c.task = { ...c.task, auto: { state: 'stopping' } };
      check('kill: autopilot sentence while stopping', c.dialogBody.endsWith('"dừng bởi bạn".'));
      c.task = { ...c.task, auto: { state: 'off' } };
      check('kill: no autopilot sentence when off', !c.dialogBody.includes('Autopilot'), `got "${c.dialogBody}"`);
      c.task = { ...c.task, runs: [{ id: 9, role: 'plan-review' }] };
      check('kill: Review plan title', c.dialogTitle === 'Kill run 9 (Review plan)?', `got "${c.dialogTitle}"`);
      c.task = { ...c.task, runs: [] };
      check('kill: falls back to the running step', c.dialogTitle === 'Kill run 9 (Dev)?', `got "${c.dialogTitle}"`);
      c.dialog = null;
    },
  },
  {
    name: 'task-gate-autopilot',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      ctx.AB.route = { name: 'task', params: { id: '1' } };
      const taskJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/task.js'), 'utf8');
      vm.runInContext(taskJsSrc, ctx, { filename: 'task.js' });

      const GATE_HELP = 'Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.';
      const ENDED_MSG = 'Đã tắt autopilot cho task này — Duyệt plan giờ chỉ ghi nhận, không tự chạy tiếp. Bấm Bật autopilot để bật lại.';
      const SOFT_MSG = 'Đã yêu cầu dừng autopilot — run hiện tại chạy nốt rồi dừng.';
      const LIMITS = 'Giới hạn: 3 vòng mỗi bước lặp, 60 phút hoặc $20 cho cả task. Autopilot luôn dừng ở bước bạn duyệt plan.';
      const APPROVE = { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null };
      const RETURN = { id: 'return_lead', label: 'Trả về Lead', enabled: true, reason: null, run_id: null };
      const STOP = { id: 'auto_stop', label: 'Dừng autopilot', enabled: true, reason: null, run_id: null };
      const START = { id: 'auto_start', label: 'Bật autopilot', enabled: true, reason: null, run_id: null };
      const GATE_AUTO = { state: 'gate', workflow_auto: true, stop_reason: null, stop_word: null, stop_detail: null, stopped_in: null, stop_requested: false };
      const T = {
        id: 1,
        slug: 'sua-app',
        status: 'planned',
        current_step: 'dev',
        actions: [],
        controls: [APPROVE, RETURN, STOP],
        auto: GATE_AUTO,
        workflow: { name: 'mac-dinh', summary: 'plan', auto: true },
        rev: 'R',
        steps: [],
        runs: [],
      };
      const WF = { name: 'mac-dinh', summary: 'plan', auto: true, max_rounds: 3, budget_minutes: 60, budget_usd: 20, limits: '3 vòng · 60 phút · $20', loops: ['plan', 'build', 'qa'], stages: [] };
      const GATE = { present: true, state: 'cho_ban', state_word: 'chờ bạn', result: 'chờ duyệt plan', at_ms: Date.now(), note: '', derived: false };
      const PIPE = { workflow: WF, groups: [], gate: GATE, current_card: 'ban', budget: null, cards: {}, ledger: { groups: [], total: null } };

      const c = factories.task();
      c.id = '1';
      c.task = T;
      c.pipe = PIPE;

      // Armed gate: Dừng autopilot in the header with the gate helper.
      check('gate: headerControls ids', JSON.stringify(c.headerControls.map((x) => x.id)) === JSON.stringify(['auto_stop']), `got ${JSON.stringify(c.headerControls.map((x) => x.id))}`);
      check('gate: headerHelper', c.headerHelper === GATE_HELP, `got "${c.headerHelper}"`);
      const stopClass = c.actionClass(STOP);
      check('gate: auto_stop secondary', stopClass.includes('ring-slate-300') && !stopClass.includes('indigo') && !stopClass.includes('rose'), `got "${stopClass}"`);
      check('gate: approve stays the primary', c.actionClass(APPROVE).includes('bg-indigo-600'), `got "${c.actionClass(APPROVE)}"`);

      const posts = [];
      let nextRes = null;
      ctx.AB.api = function (method, url, body) {
        posts.push({ method, url, body });
        return Promise.resolve(nextRes);
      };
      ctx.AB.focusLater = function () {};
      ctx.AB.lockScroll = function () {};
      let loads = 0;
      c.load = function () {
        loads++;
      };
      const settle = async () => {
        await Promise.resolve();
        await Promise.resolve();
      };

      // Dừng autopilot at the gate: the server turns autopilot off ({ended: true}).
      nextRes = { ok: true, status: 200, data: { ok: true, ended: true } };
      c.onAction(STOP);
      check('gate stop: no dialog', c.dialog === null);
      check('gate stop: in-flight label', c.actionText(STOP) === 'Đang dừng...', `got "${c.actionText(STOP)}"`);
      await settle();
      check('gate stop: POST path', posts.length === 1 && posts[0].method === 'POST' && posts[0].url === '/api/tasks/1/auto/stop', JSON.stringify(posts));
      check('gate stop: body {rev}', posts[0] && JSON.stringify(posts[0].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(posts[0]));
      check('gate stop: ended message', c.message === ENDED_MSG, `got "${c.message}"`);
      check('gate stop: reloads once', loads === 1, `loads=${loads}`);
      check('gate stop: busy cleared', c.busyAction === '');

      // Soft stop of a running driver keeps the existing copy.
      c.task = { ...T, controls: [STOP], auto: { ...GATE_AUTO, state: 'running' } };
      nextRes = { ok: true, status: 200, data: { ok: true } };
      c.onAction(STOP);
      await settle();
      check('running stop: soft-stop message', c.message === SOFT_MSG, `got "${c.message}"`);
      check('running stop: reloads', loads === 2, `loads=${loads}`);
      check('running stop: helper unchanged', c.headerHelper === 'Run hiện tại chạy nốt rồi autopilot dừng. Muốn dừng ngay: Kill.', `got "${c.headerHelper}"`);

      // Turned off at the gate: Bật autopilot with the limits helper.
      c.task = { ...T, controls: [APPROVE, RETURN, START], auto: { ...GATE_AUTO, state: 'off' } };
      check('off: headerControls ids', JSON.stringify(c.headerControls.map((x) => x.id)) === JSON.stringify(['auto_start']), `got ${JSON.stringify(c.headerControls.map((x) => x.id))}`);
      check('off: limits helper', c.headerHelper === LIMITS, `got "${c.headerHelper}"`);
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

      // 05.2 counts first, then the 05.1 figures
      const rReview = { run_id: 21, role: 'plan-review', label: 'Vòng 1 · review plan', state: 'sua_lai', verdict: 'sửa lại', counts: { blocking: 1, advisory: 4 }, turns: 41, duration_ms: 120000, cost_usd: 1.32, input_tokens: 1200, output_tokens: 300, cache_read_tokens: 50000, cache_write_tokens: 4000 };
      check('rowMetrics review counts first', c.rowMetrics(rReview) === '1 chặn · 4 góp ý · 41 lượt · 2 phút · $1.32', `got "${c.rowMetrics(rReview)}"`);
      const rQa = { run_id: 22, role: 'qa', counts: { specs: 3, failing: 1, not_automatable: null }, turns: 5, duration_ms: 60000, cost_usd: 0.5 };
      check('rowMetrics QA counts', c.rowMetrics(rQa).startsWith('3 kiểm thử · 1 lỗi · '), `got "${c.rowMetrics(rQa)}"`);
      check('rowMetrics QA unknown count omitted', !c.rowMetrics(rQa).includes('không tự động được'), `got "${c.rowMetrics(rQa)}"`);
      check('rowMetrics known zero shown', c.rowMetrics({ ...rReview, counts: { blocking: 0, advisory: null } }).startsWith('0 chặn · 41 lượt'), `got "${c.rowMetrics({ ...rReview, counts: { blocking: 0, advisory: null } })}"`);
      check('rowMetrics without counts keeps the 05.1 string', c.rowMetrics({ ...r1, counts: null }) === '69 lượt · 7 phút · $2.73', `got "${c.rowMetrics({ ...r1, counts: null })}"`);

      // token line under the selected row only
      c.runId = 21;
      check('tokenLineFor selected row', c.tokenLineFor(rReview) === 'Token: vào 1.200 · ra 300 · cache 50.000 / 4.000', `got "${c.tokenLineFor(rReview)}"`);
      check('tokenLineFor uses AB.fmtNum', c.tokenLineFor(rReview) === 'Token: vào ' + ctx.AB.fmtNum(1200) + ' · ra ' + ctx.AB.fmtNum(300) + ' · cache ' + ctx.AB.fmtNum(50000) + ' / ' + ctx.AB.fmtNum(4000));
      check('tokenLineFor non-selected row empty', c.tokenLineFor(r1) === '', `got "${c.tokenLineFor(r1)}"`);
      c.runId = 2;
      check('tokenLineFor unknown tokens', c.tokenLineFor(r2) === 'Token: vào — · ra — · cache — / —', `got "${c.tokenLineFor(r2)}"`);
      check('tokenLineFor(null) empty', c.tokenLineFor(null) === '');
      check('rowTitle tooltip kept', c.rowTitle(rReview).includes('vào 1.200'), `got "${c.rowTitle(rReview)}"`);

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

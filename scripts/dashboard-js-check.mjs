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
        // Phase 6 words (06-UI-SPEC § Status vocabulary — additions)
        cho_nguoi: {
          codePoint: 0x25D0,
          text: 'text-blue-700 dark:text-blue-300',
          card: 'border-blue-300 bg-blue-50 dark:border-blue-700 dark:bg-blue-900/20',
          word: 'chờ người',
        },
        het_quota: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'hết quota',
        },
        thieu_key: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'thiếu key',
        },
        thieu_lenh: {
          codePoint: 0x0021,
          text: 'text-amber-800 dark:text-amber-300',
          card: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
          word: 'thiếu lệnh',
        },
        chua_kiem: {
          codePoint: 0x25CC,
          text: 'text-slate-600 dark:text-slate-400',
          card: 'border-dashed border-slate-400 bg-transparent dark:border-slate-500 dark:bg-transparent',
          word: 'chưa kiểm',
        },
        san_sang: {
          codePoint: 0x25B6,
          text: 'text-emerald-700 dark:text-emerald-300',
          card: 'border-slate-300 bg-white dark:border-slate-600 dark:bg-slate-800',
          word: 'sẵn sàng',
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

      // Phase 6 vocabulary: every new key copies the class string of an existing family
      check('stepClass(cho_nguoi) == stepClass(dang_chay)', AB.stepClass('cho_nguoi') === AB.stepClass('dang_chay'), `got "${AB.stepClass('cho_nguoi')}"`);
      check('stepClass(het_quota) == stepClass(vuong_mac)', AB.stepClass('het_quota') === AB.stepClass('vuong_mac'), `got "${AB.stepClass('het_quota')}"`);
      check('stepClass(thieu_key) == stepClass(vuong_mac)', AB.stepClass('thieu_key') === AB.stepClass('vuong_mac'), `got "${AB.stepClass('thieu_key')}"`);
      check('stepClass(thieu_lenh) == stepClass(vuong_mac)', AB.stepClass('thieu_lenh') === AB.stepClass('vuong_mac'), `got "${AB.stepClass('thieu_lenh')}"`);
      check('stepClass(chua_kiem) == stepClass(chua_tu_dong)', AB.stepClass('chua_kiem') === AB.stepClass('chua_tu_dong'), `got "${AB.stepClass('chua_kiem')}"`);
      check('stateCard(cho_nguoi) == stateCard(dang_chay)', AB.stateCard('cho_nguoi') === AB.stateCard('dang_chay'));
      check('stateCard(het_quota) == stateCard(vuong_mac)', AB.stateCard('het_quota') === AB.stateCard('vuong_mac'));
      check('stateCard(chua_kiem) == stateCard(chua_tu_dong)', AB.stateCard('chua_kiem') === AB.stateCard('chua_tu_dong'));
      check('stateText(cho_nguoi) == stateText(dang_chay)', AB.stateText('cho_nguoi') === AB.stateText('dang_chay'));
      check('stepGlyphClass(cho_nguoi) does not pulse', AB.stepGlyphClass('cho_nguoi') === 'w-6 text-center' && AB.stepGlyphClass('cho_nguoi').indexOf('animate-pulse') === -1, `got "${AB.stepGlyphClass('cho_nguoi')}"`);
      check('stepGlyphClass(dang_chay) still pulses', AB.stepGlyphClass('dang_chay').indexOf('animate-pulse') >= 0, `got "${AB.stepGlyphClass('dang_chay')}"`);
      for (const k of ['het_quota', 'thieu_key', 'thieu_lenh', 'chua_kiem']) {
        check(`stepGlyphClass(${k}) does not pulse`, AB.stepGlyphClass(k).indexOf('animate-pulse') === -1);
      }

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

      // fmtWait: null → `—`; under 60 min `{M} phút`; under 48 h `{H} giờ {MM} phút`; else `{D} ngày {H} giờ`
      check('fmtWait(null)', AB.fmtWait(null) === '—', `got "${AB.fmtWait(null)}"`);
      check('fmtWait(undefined)', AB.fmtWait(undefined) === '—', `got "${AB.fmtWait(undefined)}"`);
      check('fmtWait(NaN)', AB.fmtWait(NaN) === '—', `got "${AB.fmtWait(NaN)}"`);
      check('fmtWait(0)', AB.fmtWait(0) === '0 phút', `got "${AB.fmtWait(0)}"`);
      check('fmtWait(59 min)', AB.fmtWait(59 * 60000) === '59 phút', `got "${AB.fmtWait(59 * 60000)}"`);
      check('fmtWait(60 min)', AB.fmtWait(60 * 60000) === '1 giờ 00 phút', `got "${AB.fmtWait(60 * 60000)}"`);
      check('fmtWait(3 h 5 min)', AB.fmtWait(3 * 3600000 + 5 * 60000) === '3 giờ 05 phút', `got "${AB.fmtWait(3 * 3600000 + 5 * 60000)}"`);
      check('fmtWait(47 h 59 min)', AB.fmtWait(47 * 3600000 + 59 * 60000) === '47 giờ 59 phút', `got "${AB.fmtWait(47 * 3600000 + 59 * 60000)}"`);
      check('fmtWait(48 h)', AB.fmtWait(48 * 3600000) === '2 ngày 0 giờ', `got "${AB.fmtWait(48 * 3600000)}"`);
      check('fmtWait(3 d 2 h)', AB.fmtWait(3 * 86400000 + 2 * 3600000) === '3 ngày 2 giờ', `got "${AB.fmtWait(3 * 86400000 + 2 * 3600000)}"`);
      check('fmtWait(-5000)', AB.fmtWait(-5000) === '0 phút', `got "${AB.fmtWait(-5000)}"`);

      // engineText: claude / deepseek / agy verbatim; người:{id} → `người {tên}` (names arg, stored map, else id)
      check('engineText(claude)', AB.engineText('claude') === 'claude', `got "${AB.engineText('claude')}"`);
      check('engineText(deepseek)', AB.engineText('deepseek') === 'deepseek', `got "${AB.engineText('deepseek')}"`);
      check('engineText(agy)', AB.engineText('agy') === 'agy', `got "${AB.engineText('agy')}"`);
      check('engineText(người:ha, names)', AB.engineText('người:ha', { ha: 'Hà' }) === 'người Hà', `got "${AB.engineText('người:ha', { ha: 'Hà' })}"`);
      check('engineText(người:x) without a name', AB.engineText('người:x') === 'người x', `got "${AB.engineText('người:x')}"`);
      check('engineText(người:ha, names without ha)', AB.engineText('người:ha', { khac: 'K' }) === 'người ha', `got "${AB.engineText('người:ha', { khac: 'K' })}"`);
      check('engineText(unknown) verbatim', AB.engineText('mot-engine-la') === 'mot-engine-la');
      check('engineText(null)', AB.engineText(null) === '', `got "${AB.engineText(null)}"`);
      AB.setStaffNames({ ha: 'Hà Nguyễn' });
      check('engineText uses the stored staff names', AB.engineText('người:ha') === 'người Hà Nguyễn', `got "${AB.engineText('người:ha')}"`);
      check('engineText: names arg wins over the stored map', AB.engineText('người:ha', { ha: 'H' }) === 'người H', `got "${AB.engineText('người:ha', { ha: 'H' })}"`);
      AB.setStaffNames(null);
      check('setStaffNames(null) resets the map', AB.engineText('người:ha') === 'người ha', `got "${AB.engineText('người:ha')}"`);

      // fmtPercent: n = 0 → `—`; else rounded percent
      check('fmtPercent(1, 3)', AB.fmtPercent(1, 3) === '33%', `got "${AB.fmtPercent(1, 3)}"`);
      check('fmtPercent(1, 2)', AB.fmtPercent(1, 2) === '50%', `got "${AB.fmtPercent(1, 2)}"`);
      check('fmtPercent(2, 3)', AB.fmtPercent(2, 3) === '67%', `got "${AB.fmtPercent(2, 3)}"`);
      check('fmtPercent(0, 0)', AB.fmtPercent(0, 0) === '—', `got "${AB.fmtPercent(0, 0)}"`);
      check('fmtPercent(0, 5)', AB.fmtPercent(0, 5) === '0%', `got "${AB.fmtPercent(0, 5)}"`);
      check('fmtPercent(5, 5)', AB.fmtPercent(5, 5) === '100%', `got "${AB.fmtPercent(5, 5)}"`);
      check('fmtPercent(null, 3)', AB.fmtPercent(null, 3) === '—', `got "${AB.fmtPercent(null, 3)}"`);

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
    name: 'assign-team',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const assignJsSrc = fs.readFileSync(
        path.join(repoRoot, 'agent-brain/assets/dashboard/js/assign.js'),
        'utf8'
      );
      vm.runInContext(assignJsSrc, ctx, { filename: 'assign.js' });
      if (typeof factories.assign !== 'function') {
        check('assign-team: factory registered', false, 'factories.assign not found');
        return;
      }

      const ROLES_CLAUDE = { lead: 'claude', dev: 'claude', qa: 'claude', plan_review: 'claude', code_review: 'claude' };
      const TEAM_LIST = [
        { name: 'mac-dinh', source: 'builtin', description: 'Mọi vai trò dùng claude.', summary: 'claude: lead, dev, qa, review plan, review code', roles: ROLES_CLAUDE },
        { name: 'tiet-kiem', source: 'builtin', description: 'Dev dùng deepseek.', summary: 'claude: lead, qa, review plan, review code · deepseek: dev', roles: { ...ROLES_CLAUDE, dev: 'deepseek' } },
        { name: 'doi-thu', source: 'file', description: 'Thử.', summary: 'claude: lead, qa · người Hà: dev', roles: { ...ROLES_CLAUDE, dev: 'người:ha' } },
      ];
      const NOTE = "Đội hình 'x' của dự án không có trong teams.json — dùng mac-dinh.";
      const PROJECT_DEFAULTS = {
        web: { team: 'doi-thu', effective: 'doi-thu', note: null },
        app: { team: null, effective: 'mac-dinh', note: null },
        cu: { team: 'x', effective: 'mac-dinh', note: NOTE },
      };
      let teamsResponse;
      let enginesResponse;
      let staffResponse;
      const resetResponses = () => {
        teamsResponse = {
          ok: true,
          status: 200,
          data: { teams: TEAM_LIST, errors: [], builtin_names: ['mac-dinh', 'tiet-kiem'], project_defaults: PROJECT_DEFAULTS },
        };
        enginesResponse = {
          ok: true,
          status: 200,
          data: { engines: [
            { name: 'claude', readiness: 'san_sang', word: 'sẵn sàng' },
            { name: 'deepseek', readiness: 'thieu_key', word: 'thiếu key' },
            { name: 'agy', readiness: 'san_sang', word: 'sẵn sàng' },
            { name: 'người', readiness: 'san_sang', word: 'sẵn sàng' },
          ] },
        };
        staffResponse = { ok: true, status: 200, data: { staff: [{ id: 'ha', display_name: 'Hà' }], errors: [] } };
      };
      resetResponses();
      const posts = [];
      const savedApi = ctx.AB.api;
      ctx.AB.api = function (method, url, body) {
        if (method === 'GET' && url === '/api/teams') return Promise.resolve(teamsResponse);
        if (method === 'GET' && url === '/api/engines') return Promise.resolve(enginesResponse);
        if (method === 'GET' && url === '/api/staff') return Promise.resolve(staffResponse);
        if (method === 'POST' && url === '/api/tasks') {
          posts.push(body);
          return Promise.resolve({ ok: false, status: 422, data: { error: 'Đội hình không hợp lệ' } });
        }
        return Promise.resolve({ ok: false, status: 404, data: null });
      };

      const web = { id: 'web', name_vi: 'Web', profile: 'work', workflow: null };
      const app = { id: 'app', name_vi: 'App', profile: 'work', workflow: null };
      const cu = { id: 'cu', name_vi: 'Cũ', profile: 'work', workflow: null };
      try {
        const c = factories.assign();
        check('assign-team: teamsLoading starts true', c.teamsLoading === true, `got ${c.teamsLoading}`);
        check('assign-team: teamName starts mac-dinh', c.teamName === 'mac-dinh', `got "${c.teamName}"`);
        check('assign-team: loading → no team key in the request', !Object.prototype.hasOwnProperty.call(c.postBody, 'team'), `got ${JSON.stringify(c.postBody)}`);
        await c.loadTeams();
        check('assign-team: teams in server order', c.teams.map((t) => t.name).join(',') === 'mac-dinh,tiet-kiem,doi-thu', `got ${c.teams.map((t) => t.name).join(',')}`);
        check('assign-team: teamsLoading false after load', c.teamsLoading === false && c.teamsFailed === false);
        check('assign-team: no project → mac-dinh', c.teamName === 'mac-dinh', `got "${c.teamName}"`);

        // chips and option classes
        check('assign-team: builtin chips', JSON.stringify(c.teamChips(TEAM_LIST[0])) === JSON.stringify(['có sẵn']), `got ${JSON.stringify(c.teamChips(TEAM_LIST[0]))}`);
        check('assign-team: file chips', JSON.stringify(c.teamChips(TEAM_LIST[2])) === JSON.stringify(['từ teams.json']), `got ${JSON.stringify(c.teamChips(TEAM_LIST[2]))}`);
        check('assign-team: selected option class', c.teamOptionClass(TEAM_LIST[0]) === 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400', `got "${c.teamOptionClass(TEAM_LIST[0])}"`);
        check('assign-team: unselected option class', c.teamOptionClass(TEAM_LIST[1]).includes('ring-1 ring-slate-300') && !c.teamOptionClass(TEAM_LIST[1]).includes('indigo'), `got "${c.teamOptionClass(TEAM_LIST[1])}"`);
        check('assign-team: teamChecked', c.teamChecked(TEAM_LIST[0]) === true && c.teamChecked(TEAM_LIST[1]) === false);

        // default = project default
        c.choose(web);
        check('assign-team: choose web → doi-thu (project default)', c.teamName === 'doi-thu', `got "${c.teamName}"`);
        check('assign-team: teamTouched stays false after choose', c.teamTouched === false, `got ${c.teamTouched}`);
        check('assign-team: project default chip on doi-thu', JSON.stringify(c.teamChips(TEAM_LIST[2])) === JSON.stringify(['mặc định của dự án', 'từ teams.json']), `got ${JSON.stringify(c.teamChips(TEAM_LIST[2]))}`);
        check('assign-team: no default chip on tiet-kiem', JSON.stringify(c.teamChips(TEAM_LIST[1])) === JSON.stringify(['có sẵn']), `got ${JSON.stringify(c.teamChips(TEAM_LIST[1]))}`);

        // project change re-selects unless touched
        c.choose(app);
        check('assign-team: choose app → mac-dinh', c.teamName === 'mac-dinh', `got "${c.teamName}"`);
        c.choose(web);
        c.onTeamChange({ target: { value: 'tiet-kiem' } });
        check('assign-team: onTeamChange picks the radio value', c.teamName === 'tiet-kiem', `got "${c.teamName}"`);
        check('assign-team: onTeamChange sets teamTouched', c.teamTouched === true, `got ${c.teamTouched}`);
        c.choose(app);
        check('assign-team: touched selection survives a project change', c.teamName === 'tiet-kiem', `got "${c.teamName}"`);
        c.pickTeam('mac-dinh');
        c.choose(web);
        check('assign-team: pickTeam then choose keeps the pick', c.teamName === 'mac-dinh', `got "${c.teamName}"`);

        // readiness lines and warning callout (deepseek is thieu_key)
        check('assign-team: readiness line for tiet-kiem', JSON.stringify(c.teamReadinessLines(TEAM_LIST[1])) === JSON.stringify(['! deepseek: thiếu key']), `got ${JSON.stringify(c.teamReadinessLines(TEAM_LIST[1]))}`);
        check('assign-team: no readiness line for mac-dinh', c.teamReadinessLines(TEAM_LIST[0]).length === 0, `got ${JSON.stringify(c.teamReadinessLines(TEAM_LIST[0]))}`);
        check('assign-team: no warning while mac-dinh is selected', c.hasTeamWarning === false && c.teamWarningText === '', `got "${c.teamWarningText}"`);
        c.pickTeam('tiet-kiem');
        check('assign-team: warning callout text', c.teamWarningText === 'Đội hình tiet-kiem dùng engine chưa sẵn sàng: deepseek (thiếu key). Task vẫn giao được, nhưng bước dùng engine này sẽ dừng với lỗi.', `got "${c.teamWarningText}"`);
        check('assign-team: hasTeamWarning', c.hasTeamWarning === true);

        // human Dev role helper
        c.pickTeam('doi-thu');
        const HUMAN = 'Bước do Hà làm sẽ chờ ở trạng thái "Chờ Hà làm" cho tới khi bạn bấm Xong ở màn hình task.';
        check('assign-team: human helper line', c.teamHelpLines.length === 1 && c.teamHelpLines[0] === HUMAN, `got ${JSON.stringify(c.teamHelpLines)}`);
        check('assign-team: human role is not a readiness warning', c.hasTeamWarning === false, `got "${c.teamWarningText}"`);
        c.pickTeam('mac-dinh');
        check('assign-team: no helper for an all-claude team', c.teamHelpLines.length === 0, `got ${JSON.stringify(c.teamHelpLines)}`);

        // project default note, verbatim
        const cNote = factories.assign();
        await cNote.loadTeams();
        cNote.choose(cu);
        check('assign-team: unknown project default → mac-dinh', cNote.teamName === 'mac-dinh', `got "${cNote.teamName}"`);
        check('assign-team: project default note verbatim', cNote.teamHelpLines.length === 1 && cNote.teamHelpLines[0] === NOTE, `got ${JSON.stringify(cNote.teamHelpLines)}`);

        // the picker adds no disabled reason: the same reasons, whatever team is picked
        const noProject = factories.assign();
        await noProject.loadTeams();
        const reasonBefore = noProject.disabledReason;
        noProject.pickTeam('tiet-kiem');
        check('assign-team: no-project reason unchanged by the picker', reasonBefore === 'Chọn đúng một dự án trong danh sách' && noProject.disabledReason === reasonBefore, `got "${reasonBefore}" → "${noProject.disabledReason}"`);
        c.choose(web);
        check('assign-team: reason after a project is Nhập yêu cầu', c.disabledReason === 'Nhập yêu cầu', `got "${c.disabledReason}"`);
        c.request = 'sửa app';
        c.slug = 'sua-app';
        c.preflight = { can_start: true };
        c.preflightPending = false;
        c.pickTeam('tiet-kiem');
        check('assign-team: Giao việc stays enabled with a not-ready team', c.startDisabled === false, `got ${c.startDisabled}`);
        c.pickTeam('doi-thu');
        check('assign-team: Giao việc stays enabled with a human role', c.startDisabled === false, `got ${c.startDisabled}`);
        c.pickTeam('tiet-kiem');

        // request carries team once loaded
        check('assign-team: postBody.team == teamName', c.postBody.team === 'tiet-kiem' && c.postBody.project === 'web', `got ${JSON.stringify(c.postBody)}`);
        await c.submit();
        check('assign-team: submit posts once', posts.length === 1, `got ${posts.length}`);
        check('assign-team: submit POST body has team', posts[0] && posts[0].team === 'tiet-kiem' && posts[0].name === 'sua-app', `got ${JSON.stringify(posts[0])}`);

        // invalid teams.json: 12 lines → 10 + remainder, closing line names the built-ins
        const errs = [];
        for (let i = 1; i <= 12; i++) errs.push(`Dòng ${i}: lỗi ${i}`);
        teamsResponse = { ok: true, status: 200, data: { teams: TEAM_LIST.slice(0, 2), errors: errs, builtin_names: ['mac-dinh', 'tiet-kiem'], project_defaults: {} } };
        const c3 = factories.assign();
        await c3.loadTeams();
        check('assign-team: hasTeamErrors', c3.hasTeamErrors === true);
        check('assign-team: teamErrorLines has 11 lines', c3.teamErrorLines.length === 11, `got ${c3.teamErrorLines.length}`);
        check('assign-team: teamErrorLines first is server line 1', c3.teamErrorLines[0] === 'Dòng 1: lỗi 1', `got "${c3.teamErrorLines[0]}"`);
        check('assign-team: teamErrorLines tenth is server line 10', c3.teamErrorLines[9] === 'Dòng 10: lỗi 10', `got "${c3.teamErrorLines[9]}"`);
        check('assign-team: teamErrorLines last is the remainder', c3.teamErrorLines[10] === '… và 2 lỗi khác', `got "${c3.teamErrorLines[10]}"`);
        check('assign-team: invalid-file closing line', c3.teamErrorClosing === 'Đang dùng các đội hình có sẵn (mac-dinh, tiet-kiem) cho đến khi bạn sửa xong file.', `got "${c3.teamErrorClosing}"`);
        check('assign-team: built-ins still selectable with errors', c3.teams.length === 2 && c3.teamName === 'mac-dinh' && c3.postBody.team === 'mac-dinh', `got ${c3.teams.length} / "${c3.teamName}"`);

        // failed teams fetch: failure flag, no team key in the request
        resetResponses();
        teamsResponse = { ok: false, status: 0, data: null };
        const c4 = factories.assign();
        await c4.loadTeams();
        check('assign-team: teamsFailed on fetch failure', c4.teamsFailed === true && c4.teamsLoading === false, `got ${c4.teamsFailed} / ${c4.teamsLoading}`);
        c4.choose(web);
        check('assign-team: failed list → postBody has no team key', !Object.prototype.hasOwnProperty.call(c4.postBody, 'team'), `got ${JSON.stringify(c4.postBody)}`);
        check('assign-team: failed list → no warning, no helper', c4.hasTeamWarning === false && c4.teamHelpLines.length === 0);

        // the readiness and staff calls only feed the warnings: both failing hides them, the list stays
        resetResponses();
        enginesResponse = { ok: false, status: 500, data: null };
        staffResponse = { ok: false, status: 500, data: null };
        const c5 = factories.assign();
        await c5.loadTeams();
        c5.pickTeam('tiet-kiem');
        check('assign-team: engines failure hides readiness only', c5.teamsFailed === false && c5.teams.length === 3 && c5.teamReadinessLines(TEAM_LIST[1]).length === 0 && c5.hasTeamWarning === false);
        c5.pickTeam('doi-thu');
        check('assign-team: staff failure falls back to the id in the helper', c5.teamHelpLines.length === 1 && c5.teamHelpLines[0].indexOf('Bước do ha làm') === 0, `got ${JSON.stringify(c5.teamHelpLines)}`);
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
    name: 'board',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const boardJsPath = path.join(repoRoot, 'agent-brain/assets/dashboard/js/board.js');
      const boardJsSrc = fs.readFileSync(boardJsPath, 'utf8');
      vm.runInContext(boardJsSrc, ctx, { filename: 'board.js' });

      check('board: factory registered', typeof factories.board === 'function', 'factories.board not found');
      if (typeof factories.board !== 'function') return;

      let gate = null;
      // Run one named step; a thrown error is a failed check, not a crash.
      async function step(name, fn) {
        try {
          await fn();
        } catch (err) {
          gate = null;
          check(name, false, `threw: ${err && err.message ? err.message : err}`);
        }
      }
      const settle = async () => {
        for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0));
      };
      const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

      // Smoke-fixture board, epic=all (waves 01-01:0, 03:0, 01-02:1, 02-01:2, 02-02:3, 02-03:3).
      const mk = (key, epic, wave, status, extra) => Object.assign({
        key, title: key, epic, epic_name: 'Phase ' + epic, milestone: null, wave, fm_wave: null,
        status, critical: false, placeholder: false, requirements: [], plan_path: null,
        waits_on: [], waited_by: [],
      }, extra || {});
      const ITEMS_ALL = [
        mk('01-01', '01', 0, 'xong'),
        mk('03', '03', 0, 'san_sang', { placeholder: true }),
        mk('01-02', '01', 1, 'xong'),
        mk('02-01', '02', 2, 'san_sang'),
        mk('02-02', '02', 3, 'bi_chan'),
        mk('02-03', '02', 3, 'bi_chan'),
      ];
      const EDGES_ALL = [
        { from: '01-01', to: '01-02', kind: 'plan', critical: false },
        { from: '01-02', to: '02-01', kind: 'plan', critical: false },
        { from: '02-01', to: '02-02', kind: 'plan', critical: false },
        { from: '02-01', to: '02-03', kind: 'plan', critical: false },
      ];
      const EPICS = [
        { id: '01', name: 'Nền tảng', milestone: null, total: 2, done: 2, placeholder: false },
        { id: '02', name: 'Xuất báo cáo', milestone: null, total: 3, done: 0, placeholder: false },
        { id: '03', name: 'Hướng dẫn', milestone: null, total: 1, done: 0, placeholder: true },
      ];
      const PROJECTS = [
        { id: 'web-ban-hang', name_vi: 'Web bán hàng', profile: 'work', has_board: true },
        { id: 'khac', name_vi: 'Dự án khác', profile: 'work', has_board: true },
        { id: 'khong-co', name_vi: 'Không có bảng', profile: 'work', has_board: false },
      ];
      const PROJECT = { id: 'web-ban-hang', name_vi: 'Web bán hàng', profile: 'work', type: 'code' };
      const baseOk = (over) => Object.assign({
        state: 'ok', reason: null, message: null, expected_path: '/x/.planning/ROADMAP.md',
        project: PROJECT, projects: PROJECTS, epic: '02', default_epic: '02', epics: EPICS,
        milestone: null, milestones: [], items: [], edges: [], external: [],
        counts: { total: 0, xong: 0, san_sang: 0, bi_chan: 0, waves: 0 },
        warnings: [], truncated: false,
      }, over || {});
      const BOARD_ALL = baseOk({
        epic: 'all', items: ITEMS_ALL, edges: EDGES_ALL,
        counts: { total: 6, xong: 2, san_sang: 2, bi_chan: 2, waves: 4 },
      });
      const BOARD_02 = baseOk({
        items: [mk('02-01', '02', 0, 'san_sang'), mk('02-02', '02', 1, 'bi_chan'), mk('02-03', '02', 1, 'bi_chan')],
        edges: [
          { from: '02-01', to: '02-02', kind: 'plan', critical: false },
          { from: '02-01', to: '02-03', kind: 'plan', critical: false },
        ],
        counts: { total: 3, xong: 0, san_sang: 1, bi_chan: 2, waves: 2 },
      });

      // --- pure layout and geometry (golden numbers of the plan) ---
      await step('board: layout', async () => {
        const B = ctx.ABBoard;
        check('board: ABBoard exported', Boolean(B) && typeof B.layout === 'function' && typeof B.edgeGeometry === 'function' && typeof B.domId === 'function');
        check('board: constants', B.COL_W === 216 && B.RAIL_W === 96 && B.GAP_X === 32 && B.HEAD_H === 48 && B.ROW_GAP === 8 && B.CARD_H === 64 && B.CHIP_H === 32);
        const on = B.layout(ITEMS_ALL, true);
        check('board: collapse on columns', same(on.cols.map((c) => [c.x, c.w, c.rail]), [[0, 216, false], [248, 96, true], [376, 216, false], [624, 216, false]]), JSON.stringify(on.cols));
        const p = on.pos;
        check('board: collapse on 01-01 is a chip', p['01-01'].y === 56 && p['01-01'].h === 32 && p['01-01'].chip === true, JSON.stringify(p['01-01']));
        check('board: collapse on 03 is a card', p['03'].y === 96 && p['03'].h === 64 && p['03'].chip === false, JSON.stringify(p['03']));
        check('board: collapse on 01-02 is a rail chip', p['01-02'].y === 56 && p['01-02'].h === 32 && p['01-02'].rail === true, JSON.stringify(p['01-02']));
        check('board: collapse on 02-01/02-02/02-03', p['02-01'].y === 56 && p['02-01'].h === 64 && p['02-02'].y === 56 && p['02-03'].y === 128, JSON.stringify([p['02-01'], p['02-02'], p['02-03']]));
        check('board: collapse on width and height', on.width === 840 && on.height === 192, `${on.width}x${on.height}`);
        check('board: column counts', same(on.cols.map((c) => [c.wave, c.done, c.total]), [[0, 1, 2], [1, 1, 1], [2, 0, 1], [3, 0, 2]]), JSON.stringify(on.cols));
        const off = B.layout(ITEMS_ALL, false);
        check('board: collapse off columns', same(off.cols.map((c) => [c.x, c.w]), [[0, 216], [248, 216], [496, 216], [744, 216]]), JSON.stringify(off.cols));
        check('board: collapse off every card is 64 high', Object.keys(off.pos).every((k) => off.pos[k].h === 64 && off.pos[k].chip === false));
        check('board: collapse off 03 at y 128', off.pos['03'].y === 128, `${off.pos['03'].y}`);
        check('board: collapse off width and height', off.width === 960 && off.height === 192, `${off.width}x${off.height}`);
        const none = B.layout([], true);
        check('board: empty layout', none.width === 0 && none.height === 0 && none.cols.length === 0);

        const g = B.edgeGeometry(on.pos['01-01'], on.pos['01-02']);
        check('board: edge boxStyle', same(g.boxStyle, { left: '216px', top: '70px', width: '32px', height: '4px' }), JSON.stringify(g.boxStyle));
        check('board: edge path', g.d === 'M0 2 C16 2 16 2 32 2', g.d);
        check('board: domId', B.domId('05.4-03') === 'board-item-05_4-03' && B.domId('a b/c') === 'board-item-a_b_c', `${B.domId('05.4-03')}`);
      });

      // --- harness: Map-backed localStorage, call recorder, timer counters ---
      const store = new Map();
      const savedStorage = ctx.localStorage;
      const savedApi = ctx.AB.api;
      const savedInterval = ctx.setInterval;
      const savedEvery = ctx.AB.every;
      const calls = [];
      let intervalCalls = 0;
      let everyCalls = 0;
      const defaultAnswer = (url) => {
        if (url.indexOf('project=gone') !== -1) return { ok: false, status: 404, data: { error: 'Không tìm thấy dự án gone' } };
        if (url.indexOf('epic=all') !== -1) return { ok: true, status: 200, data: BOARD_ALL };
        return { ok: true, status: 200, data: BOARD_02 };
      };
      let answer = defaultAnswer;
      ctx.localStorage = {
        getItem(k) { return store.has(k) ? store.get(k) : null; },
        setItem(k, v) { store.set(k, String(v)); },
        removeItem(k) { store.delete(k); },
      };
      ctx.setInterval = function () { intervalCalls++; return savedInterval.apply(null, arguments); };
      ctx.AB.every = function () { everyCalls++; return savedEvery.apply(ctx.AB, arguments); };
      ctx.AB.api = function (method, url) {
        calls.push(method + ' ' + url);
        const res = answer(url);
        if (gate) {
          const g = gate;
          gate = null;
          return g.then(() => res);
        }
        return Promise.resolve(res);
      };
      const reset = () => { calls.length = 0; };
      const urls = () => calls.map((c) => c.replace(/^GET /, ''));

      try {
        // --- first load: one call, default epic, collapse default on ---
        await step('board: init', async () => {
          reset();
          const c = factories.board();
          check('board: collapse defaults to true', c.collapse === true, `got ${c.collapse}`);
          check('board: not loaded before init', c.loaded === false && c.isLoading === true);
          c.init();
          check('board: init makes exactly one call', calls.length === 1 && calls[0].startsWith('GET /api/board'), JSON.stringify(calls));
          await settle();
          check('board: loaded after the answer', c.loaded === true && c.loadedAttr === 'yes' && c.isLoading === false);
          check('board: epicSel takes the server epic', c.epicSel === '02', `got "${c.epicSel}"`);
          check('board: projectSel takes the server project', c.projectSel === 'web-ban-hang', `got "${c.projectSel}"`);
          check('board: hasBoard', c.hasBoard === true && c.isEmpty === false && c.noItems === false);
          check('board: three cards', c.cards.length === 3, `got ${c.cards.length}`);
          check('board: card ids and keys', c.cards[0].domId === 'board-item-02-01' && c.cards[0].key === '02-01');
          check('board: card status text', c.cards[0].statusText === '▶︎ sẵn sàng' && c.cards[1].statusText === '⟳︎ bị chặn', `${c.cards[0].statusText} / ${c.cards[1].statusText}`);
          check('board: card label', c.cardLabel(c.cards[0]) === '02-01 02-01, sẵn sàng, wave 0', c.cardLabel(c.cards[0]));
          check('board: card label critical', c.cardLabel(Object.assign({}, c.cards[0], { critical: true })).endsWith(', trên đường găng'));
          check('board: two edge views', c.edgeViews.length === 2 && c.edgeViews[0].id === '02-01__02-02' && c.edgeViews.every((e) => parseFloat(e.boxStyle.width) > 0), JSON.stringify(c.edgeViews.map((e) => e.id)));
          check('board: edge classes', c.edgeClass({ critical: false }).includes('stroke-slate-400') && c.edgeClass({ critical: true }).includes('stroke-yellow-600'));
          check('board: head labels', same(c.cols.map((k) => [k.title, k.sub, k.waveAttr, k.railAttr]), [['Wave 0', '0/1', '0', '0'], ['Wave 1', '0/2', '1', '0']]), JSON.stringify(c.cols));
          check('board: legend', same(c.legend.map((l) => l.text), ['✓︎ xong 0', '▶︎ sẵn sàng 1', '⟳︎ bị chặn 2']), JSON.stringify(c.legend));
          check('board: legendTotal', c.legendTotal === 'Tổng 3 · 2 wave', c.legendTotal);
        });

        await step('board: profile in the url', async () => {
          reset();
          store.set('ab.profile', 'work');
          const c = factories.board();
          c.init();
          await settle();
          check('board: profile=work in the url', calls.length === 1 && calls[0].includes('profile=work'), JSON.stringify(calls));
          store.delete('ab.profile');
        });

        // --- collapse switch ---
        await step('board: collapse switch', async () => {
          store.delete('ab.board.collapse');
          const c = factories.board();
          check('board: collapse stored 0 reads false', (() => { store.set('ab.board.collapse', '0'); const d = factories.board(); store.delete('ab.board.collapse'); return d.collapse === false; })());
          check('board: collapse stored 1 reads true', (() => { store.set('ab.board.collapse', '1'); const d = factories.board(); store.delete('ab.board.collapse'); return d.collapse === true; })());
          answer = () => ({ ok: true, status: 200, data: BOARD_ALL });
          await c.load();
          reset();
          check('board: collapseAttr on', c.collapseAttr === 'true' && c.collapseWord === 'bật', `${c.collapseAttr}/${c.collapseWord}`);
          const first = c.cards.find((k) => k.key === '01-01');
          check('board: 01-01 is a chip while collapsed', first.chip === true && first.style.height === '32px', JSON.stringify(first.style));
          check('board: 01-02 is a rail chip', c.cards.find((k) => k.key === '01-02').rail === true);
          check('board: rail column head', c.cols[1].railAttr === '1' && c.cols[1].sub === '1/1 ✓︎', JSON.stringify(c.cols[1]));
          c.toggleCollapse();
          check('board: toggle flips to false', c.collapse === false && c.collapseAttr === 'false' && c.collapseWord === 'tắt');
          check('board: toggle stores 0', store.get('ab.board.collapse') === '0', `got ${store.get('ab.board.collapse')}`);
          const open = c.cards.find((k) => k.key === '01-01');
          check('board: 01-01 is a full card when off', open.chip === false && open.style.height === '64px', JSON.stringify(open.style));
          check('board: nothing is a rail when off', c.cards.every((k) => k.rail === false) && c.cols.every((k) => k.railAttr === '0'));
          c.toggleCollapse();
          check('board: toggle back stores 1', c.collapse === true && store.get('ab.board.collapse') === '1');
          check('board: switch makes no api call', calls.length === 0, JSON.stringify(calls));
          store.delete('ab.board.collapse');
          answer = defaultAnswer;
        });

        // --- Đọc lại ---
        await step('board: reload', async () => {
          const c = factories.board();
          await c.load();
          reset();
          check('board: reloadLabel idle', c.reloadLabel === 'Đọc lại' && c.reloading === false, `${c.reloadLabel}`);
          let release;
          gate = new Promise((r) => { release = r; });
          const pending = c.reload();
          check('board: reload sets the in-flight label', c.reloadLabel === 'Đang đọc...' && c.reloading === true, `${c.reloadLabel}`);
          c.reload();
          check('board: second reload makes no extra call', calls.length === 1, JSON.stringify(calls));
          release();
          await pending;
          check('board: reload clears the flag', c.reloading === false && c.reloadLabel === 'Đọc lại');
          check('board: reload made exactly one call', calls.length === 1 && calls[0].startsWith('GET /api/board'), JSON.stringify(calls));
        });

        // --- filters ---
        await step('board: epic filter', async () => {
          const c = factories.board();
          await c.load();
          reset();
          c.epicSel = 'all';
          c.epicChanged();
          await settle();
          check('board: epicChanged sends epic=all', calls.length === 1 && urls()[0].includes('epic=all'), JSON.stringify(calls));
          check('board: epic all shows six cards', c.cards.length === 6, `got ${c.cards.length}`);
          check('board: epic all keeps epicSel all', c.epicSel === 'all', `got "${c.epicSel}"`);
        });

        await step('board: milestone filter', async () => {
          const c = factories.board();
          await c.load();
          reset();
          c.milestoneSel = 'v1.0';
          c.milestoneChanged();
          await settle();
          check('board: milestoneChanged sends milestone=v1.0', calls.length === 1 && urls()[0].includes('milestone=v1.0'), JSON.stringify(calls));
        });

        await step('board: project filter', async () => {
          store.delete('ab.board.project');
          const c = factories.board();
          await c.load();
          reset();
          c.projectSel = 'khac';
          c.projectChanged();
          await settle();
          check('board: projectChanged stores the id', store.get('ab.board.project') === 'khac', `got ${store.get('ab.board.project')}`);
          check('board: projectChanged sends project=khac without epic', calls.length === 1 && urls()[0].includes('project=khac') && !urls()[0].includes('epic='), JSON.stringify(calls));
        });

        await step('board: stored project', async () => {
          store.set('ab.board.project', 'khac');
          reset();
          const c = factories.board();
          c.init();
          await settle();
          check('board: stored project is sent on the first call', calls.length >= 1 && urls()[0].includes('project=khac'), JSON.stringify(calls));
          store.delete('ab.board.project');
        });

        await step('board: stale stored project', async () => {
          store.set('ab.board.project', 'gone');
          reset();
          const c = factories.board();
          c.init();
          await settle();
          check('board: 404 for a stored project retries once without project=', calls.length === 2 && urls()[0].includes('project=gone') && !urls()[1].includes('project='), JSON.stringify(calls));
          check('board: 404 removes the stored project', !store.has('ab.board.project'), `got ${store.get('ab.board.project')}`);
          check('board: retry ends with a board', c.hasBoard === true && c.loaded === true && c.hasError === false);
          reset();
          c.projectSel = 'gone';
          await c.load();
          check('board: the retry happens once per component', calls.length === 1 && c.hasError === true, JSON.stringify(calls));
        });

        await step('board: failed request', async () => {
          const keep = answer;
          answer = () => ({ ok: false, status: 500, data: { error: 'Lỗi nội bộ' } });
          const c = factories.board();
          await c.load();
          check('board: failed request shows the error card', c.hasError === true && c.loaded === true && c.errorBody.startsWith('Lỗi nội bộ'), `${c.errorBody}`);
          answer = keep;
          await c.load();
          check('board: Thử lại clears the error', c.hasError === false && c.hasBoard === true);
        });

        // --- toolbar getters ---
        await step('board: toolbar getters', async () => {
          const c = factories.board();
          check('board: no milestones hides the control', c.showMilestones === false);
          c.milestones = [];
          check('board: zero milestones', c.showMilestones === false);
          c.milestones = ['v1.0'];
          check('board: one milestone', c.showMilestones === false);
          c.milestones = ['v1.0', 'v1.1'];
          check('board: two milestones', c.showMilestones === true);
          await c.load();
          check('board: boardProjects lists projects with a board', same(c.boardProjects.map((p) => p.id), ['web-ban-hang', 'khac']), JSON.stringify(c.boardProjects.map((p) => p.id)));
          check('board: hasProjects', c.hasProjects === true);
          c.project = { id: 'khong-co', name_vi: 'Không có bảng', profile: 'work', type: 'code' };
          check('board: boardProjects keeps the current project', same(c.boardProjects.map((p) => p.id), ['web-ban-hang', 'khac', 'khong-co']), JSON.stringify(c.boardProjects.map((p) => p.id)));
          check('board: projectLabel', c.projectLabel({ id: 'a', name_vi: 'Tên' }) === 'Tên' && c.projectLabel({ id: 'a' }) === 'a');
          check('board: epicLabel', c.epicLabel(EPICS[1]) === '02 · Xuất báo cáo (0/3)', c.epicLabel(EPICS[1]));
          c.projectSel = 'khac';
          c.epicSel = 'all';
          c.milestoneSel = 'v1.1';
          check('board: isProjectSel', c.isProjectSel(PROJECTS[1]) === true && c.isProjectSel(PROJECTS[0]) === false);
          check('board: isEpicSel', c.isEpicSel('all') === true && c.isEpicSel('02') === false);
          check('board: isMilestoneSel', c.isMilestoneSel('v1.1') === true && c.isMilestoneSel('') === false);
        });

        // --- warnings ---
        await step('board: warnings', async () => {
          const c = factories.board();
          check('board: no warnings', c.hasWarnings === false);
          const ws = [];
          for (let i = 1; i <= 12; i++) ws.push({ path: `.planning/p${i}.md`, message: `lỗi ${i}` });
          c.apply(baseOk({ warnings: ws }));
          check('board: hasWarnings', c.hasWarnings === true);
          check('board: warningLines has 11 entries', c.warningLines.length === 11, `got ${c.warningLines.length}`);
          check('board: first warning line', c.warningLines[0] === '.planning/p1.md — lỗi 1', `got "${c.warningLines[0]}"`);
          check('board: warning remainder', c.warningLines[10] === '… và 2 cảnh báo khác', `got "${c.warningLines[10]}"`);
          c.apply(baseOk({ warnings: ws.slice(0, 3) }));
          check('board: three warnings are all shown', c.warningLines.length === 3 && c.warningLines[2] === '.planning/p3.md — lỗi 3');
          c.apply(baseOk());
          check('board: zero warnings', c.hasWarnings === false && c.warningLines.length === 0);
        });

        // --- empty state and empty board ---
        await step('board: empty state', async () => {
          const c = factories.board();
          c.apply({
            state: 'empty', reason: 'no_roadmap', message: 'Dự án này chưa có bảng công việc. Cần file /x/.planning/ROADMAP.md.',
            expected_path: '/x/.planning/ROADMAP.md', project: null, projects: [],
          });
          check('board: empty state', c.isEmpty === true && c.hasBoard === false && c.cards.length === 0);
          check('board: empty message verbatim', c.message === 'Dự án này chưa có bảng công việc. Cần file /x/.planning/ROADMAP.md.', c.message);
          check('board: empty expected path', c.expectedPath === '/x/.planning/ROADMAP.md');
          check('board: empty state has no project', c.projectSel === '' && c.hasProjects === false);
          c.apply(baseOk());
          check('board: ok with no items', c.hasBoard === true && c.noItems === true && c.isEmpty === false);
        });

        // --- no polling, no stream ---
        check('board: no interval timer was created', intervalCalls === 0, `setInterval calls: ${intervalCalls}`);
        check('board: no AB.every polling', everyCalls === 0, `AB.every calls: ${everyCalls}`);
        check('board: only GET /api/board was requested', calls.concat([]).every((x) => x.startsWith('GET /api/board')), JSON.stringify(calls));
      } finally {
        ctx.localStorage = savedStorage;
        ctx.AB.api = savedApi;
        ctx.setInterval = savedInterval;
        ctx.AB.every = savedEvery;
      }

      // --- source checks ---
      check('board: no arrow function in board.js', boardJsSrc.indexOf('=' + '>') === -1, 'found an arrow function');
      check('board: no template literal in board.js', boardJsSrc.indexOf(String.fromCharCode(96)) === -1, 'found a backtick');
      check('board: no timer or stream in board.js', !/setInterval|AB\.every|EventSource/.test(boardJsSrc), 'found a timer or stream');
    },
  },
  {
    name: 'board-select',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const boardJsPath = path.join(repoRoot, 'agent-brain/assets/dashboard/js/board.js');
      vm.runInContext(fs.readFileSync(boardJsPath, 'utf8'), ctx, { filename: 'board.js' });

      check('board-select: factory registered', typeof factories.board === 'function', 'factories.board not found');
      if (typeof factories.board !== 'function') return;

      async function step(name, fn) {
        try {
          await fn();
        } catch (err) {
          check(name, false, `threw: ${err && err.message ? err.message : err}`);
        }
      }
      const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
      const keysOf = (set) => Object.keys(set).sort();
      const mk = (key, epic, wave, status, extra) => Object.assign({
        key, title: 'Việc ' + key, epic, epic_name: 'Xuất báo cáo đơn hàng', milestone: null, wave, fm_wave: null,
        status, critical: false, placeholder: false, requirements: [], plan_path: null,
        waits_on: [], waited_by: [],
      }, extra || {});

      // SHOP fixture of plan 05.5-03 (plans 01-01 ... 01-14, 01-01 and 01-02 done), epic=all.
      const DEPS = {
        '01-01': [], '01-02': [], '01-03': [], '01-04': ['01-01'], '01-05': ['01-01', '01-02'],
        '01-06': [], '01-07': ['01-03', '01-04'], '01-08': ['01-07', '01-06'], '01-09': ['01-08'],
        '01-10': ['01-07'], '01-11': ['01-05', '01-07'], '01-12': ['01-08'], '01-13': ['01-09', '01-11'],
        '01-14': ['01-09'],
      };
      const WAVE = {
        '01-01': 0, '01-02': 0, '01-03': 0, '01-04': 1, '01-05': 1, '01-06': 0, '01-07': 2,
        '01-08': 3, '01-09': 4, '01-10': 3, '01-11': 3, '01-12': 4, '01-13': 5, '01-14': 5,
      };
      const STATUS = {
        '01-01': 'xong', '01-02': 'xong', '01-03': 'san_sang', '01-04': 'san_sang', '01-05': 'san_sang', '01-06': 'san_sang',
      };
      const CRIT_ITEMS = ['01-04', '01-07', '01-08', '01-09', '01-13'];
      const CRIT_EDGES = ['01-04__01-07', '01-07__01-08', '01-08__01-09', '01-09__01-13'];
      const ORDER = Object.keys(DEPS);
      const waitedBy = (k) => ORDER.filter((o) => DEPS[o].indexOf(k) !== -1);
      const shopItems = ORDER
        .slice()
        .sort((a, b) => WAVE[a] - WAVE[b] || ORDER.indexOf(a) - ORDER.indexOf(b))
        .map((k) => mk(k, '01', WAVE[k], STATUS[k] || 'bi_chan', {
          critical: CRIT_ITEMS.indexOf(k) !== -1,
          requirements: k === '01-07' ? ['BOARD-12', 'BOARD-09'] : [],
          plan_path: '.planning/phases/01-xuat-bao-cao/' + k + '-PLAN.md',
          waits_on: DEPS[k],
          waited_by: waitedBy(k),
        }));
      const shopEdges = [];
      ORDER.forEach((to) => DEPS[to].forEach((from) => {
        shopEdges.push({ from, to, kind: 'plan', critical: CRIT_EDGES.indexOf(from + '__' + to) !== -1 });
      }));
      const PROJECT = { id: 'web-ban-hang', name_vi: 'Web bán hàng', profile: 'work', type: 'code' };
      const baseOk = (over) => Object.assign({
        state: 'ok', reason: null, message: null, expected_path: '/x/.planning/ROADMAP.md',
        project: PROJECT, projects: [{ id: 'web-ban-hang', name_vi: 'Web bán hàng', profile: 'work', has_board: true }],
        epic: 'all', default_epic: '02', epics: [], milestone: null, milestones: [], items: [], edges: [], external: [],
        counts: { total: 0, xong: 0, san_sang: 0, bi_chan: 0, waves: 0 }, warnings: [], truncated: false,
      }, over || {});
      const SHOP = baseOk({ items: shopItems, edges: shopEdges, counts: { total: 14, xong: 2, san_sang: 4, bi_chan: 8, waves: 6 } });
      // Smoke-fixture default view (epic 02) with its one neighbour outside the view.
      const SMOKE_02 = baseOk({
        epic: '02',
        items: [
          mk('02-01', '02', 0, 'san_sang', { epic_name: 'Xuất báo cáo', waits_on: ['01-02'], waited_by: ['02-02', '02-03'], plan_path: '.planning/phases/02-xuat-bao-cao/02-01-PLAN.md' }),
          mk('02-02', '02', 1, 'bi_chan', { epic_name: 'Xuất báo cáo', waits_on: ['02-01'] }),
          mk('02-03', '02', 1, 'bi_chan', { epic_name: 'Xuất báo cáo', waits_on: ['02-01'] }),
        ],
        edges: [
          { from: '02-01', to: '02-02', kind: 'plan', critical: false },
          { from: '02-01', to: '02-03', kind: 'plan', critical: false },
        ],
        external: [{ key: '01-02', title: 'Truy vấn đơn hàng', epic: '01', status: 'xong' }],
        counts: { total: 3, xong: 0, san_sang: 1, bi_chan: 2, waves: 2 },
      });

      const savedApi = ctx.AB.api;
      const savedFocus = ctx.AB.focusLater;
      const savedStorage = ctx.localStorage;
      const focused = [];
      ctx.AB.focusLater = function (id, fallback) { focused.push(id + (fallback ? '|' + fallback : '')); };
      ctx.localStorage = {
        getItem() { return null; },
        setItem() {},
        removeItem() {},
      };
      ctx.AB.api = function () { return Promise.resolve({ ok: true, status: 200, data: SHOP }); };
      const make = (data) => {
        const c = factories.board();
        c.apply(data);
        return c;
      };
      const card = (c, key) => c.cards.find((k) => k.key === key);
      const edge = (c, id) => c.edgeViews.find((e) => e.id === id);
      const classOf = (c, key) => c.cardClass(card(c, key));

      try {
        await step('board-select: ABBoard.related', async () => {
          const B = ctx.ABBoard;
          check('board-select: related is exported', typeof B.related === 'function');
          const c = make(SHOP);
          const lit = B.related('01-07', c.upMap, c.downMap);
          const want = ['01-01', '01-03', '01-04', '01-07', '01-08', '01-09', '01-10', '01-11', '01-12', '01-13', '01-14'];
          check('board-select: related(01-07) is the eleven keys of the golden', same(keysOf(lit), want), JSON.stringify(keysOf(lit)));
          const lit12 = B.related('01-12', c.upMap, c.downMap);
          check('board-select: related(01-12) is its seven-key chain', same(keysOf(lit12), ['01-01', '01-03', '01-04', '01-06', '01-07', '01-08', '01-12']), JSON.stringify(keysOf(lit12)));
          check('board-select: related of an isolated key is just itself', same(keysOf(B.related('x', {}, {})), ['x']));
          check('board-select: a key named constructor is not lit by inheritance', B.related('x', {}, {}).constructor === undefined);

          const N = 3000;
          const up = {};
          const down = {};
          for (let i = 0; i < N; i++) {
            up['n' + i] = i > 0 ? ['n' + (i - 1)] : [];
            down['n' + i] = i < N - 1 ? ['n' + (i + 1)] : [];
          }
          const big = B.related('n1500', up, down);
          check('board-select: 3,000-node chain closes without a stack overflow', Object.keys(big).length === N, `got ${Object.keys(big).length}`);
          const cyc = B.related('a', { a: ['b'], b: ['a'] }, { a: ['b'], b: ['a'] });
          check('board-select: a cycle terminates', same(keysOf(cyc), ['a', 'b']), JSON.stringify(keysOf(cyc)));
        });

        await step('board-select: maps are restricted to visible cards', async () => {
          const c = make(SMOKE_02);
          check('board-select: upMap drops a neighbour outside the view', same(c.upMap['02-01'], []), JSON.stringify(c.upMap['02-01']));
          check('board-select: downMap keeps visible waiters', same(c.downMap['02-01'], ['02-02', '02-03']), JSON.stringify(c.downMap['02-01']));
        });

        await step('board-select: select(01-07) lights the chain and dims the rest', async () => {
          const c = make(SHOP);
          check('board-select: no selection at first', c.sel === '' && c.hasSelection === false && c.selectedItem === null);
          check('board-select: no card is dimmed or ringed before a selection', c.cards.every((k) => !c.cardClass(k).includes('opacity-25') && !c.cardClass(k).includes('ring-cyan')));
          check('board-select: edges keep their classes before a selection', c.edgeClass(edge(c, '01-07__01-08')).includes('stroke-yellow-600') && c.edgeClass(edge(c, '01-02__01-05')).includes('stroke-slate-400') && !c.edgeClass(edge(c, '01-02__01-05')).includes('opacity-25'));
          c.select('01-07');
          check('board-select: sel is 01-07', c.sel === '01-07' && c.hasSelection === true && c.selectedItem.key === '01-07');
          check('board-select: pressed is true only for 01-07', c.cards.every((k) => c.pressed(k) === (k.key === '01-07' ? 'true' : 'false')));
          check('board-select: selected card has the cyan ring', classOf(c, '01-07').includes('ring-2 ring-cyan-600 dark:ring-cyan-400'), classOf(c, '01-07'));
          check('board-select: the ring is only on the selected card', c.cards.every((k) => k.key === '01-07' || !c.cardClass(k).includes('ring-cyan')));
          for (const k of ['01-02', '01-05', '01-06']) {
            check(`board-select: ${k} is dimmed`, classOf(c, k).includes('opacity-25'), classOf(c, k));
          }
          for (const k of ['01-01', '01-03', '01-04', '01-07', '01-08', '01-09', '01-10', '01-11', '01-12', '01-13', '01-14']) {
            check(`board-select: ${k} is lit`, !classOf(c, k).includes('opacity-25'), classOf(c, k));
          }
          check('board-select: a chain edge is cyan', c.edgeClass(edge(c, '01-07__01-08')).includes('stroke-cyan-600') && c.edgeClass(edge(c, '01-07__01-08')).includes('dark:stroke-cyan-400') && c.edgeClass(edge(c, '01-07__01-08')).includes('stroke-2'), c.edgeClass(edge(c, '01-07__01-08')));
          check('board-select: a chain edge is not dimmed', !c.edgeClass(edge(c, '01-07__01-08')).includes('opacity-25'));
          check('board-select: an edge outside the chain is dimmed', c.edgeClass(edge(c, '01-02__01-05')).includes('opacity-25') && c.edgeClass(edge(c, '01-02__01-05')).includes('stroke-slate-400'), c.edgeClass(edge(c, '01-02__01-05')));
          check('board-select: a chain edge is raised, an outside edge is not', c.edgeZ(edge(c, '01-07__01-08')) === 'z-10' && c.edgeZ(edge(c, '01-02__01-05')) === 'z-0', `${c.edgeZ(edge(c, '01-07__01-08'))}/${c.edgeZ(edge(c, '01-02__01-05'))}`);
          check('board-select: a lit non-critical edge is raised', c.edgeZ(edge(c, '01-07__01-10')) === 'z-10');
          check('board-select: critical edge 04 to 07 is cyan when the chain holds it', c.edgeClass(edge(c, '01-04__01-07')).includes('stroke-cyan-600') && !c.edgeClass(edge(c, '01-04__01-07')).includes('stroke-yellow'));
          c.select('01-07');
          check('board-select: selecting 01-07 again clears the selection', c.sel === '' && c.hasSelection === false && c.selectedItem === null && c.cards.every((k) => c.pressed(k) === 'false'));
          check('board-select: after clearing no card class has opacity-25', c.cards.every((k) => !c.cardClass(k).includes('opacity-25')));
          check('board-select: after clearing edges are back to normal', c.edgeClass(edge(c, '01-07__01-08')).includes('stroke-yellow-600') && !c.edgeClass(edge(c, '01-02__01-05')).includes('opacity-25') && c.edgeZ(edge(c, '01-07__01-10')) === 'z-0');
          check('board-select: select ignores a key that is not on screen', (() => { c.select('99-99'); return c.sel === ''; })());
        });

        await step('board-select: critical edges in focus mode', async () => {
          const c = make(SHOP);
          c.select('01-12');
          check('board-select: 01-12 lights seven keys', same(keysOf(c.lit), ['01-01', '01-03', '01-04', '01-06', '01-07', '01-08', '01-12']), JSON.stringify(keysOf(c.lit)));
          const out = c.edgeClass(edge(c, '01-09__01-13'));
          check('board-select: critical edge outside the chain keeps yellow and is dimmed', out.includes('stroke-yellow-600') && out.includes('dark:stroke-yellow-400') && out.includes('opacity-25'), out);
          check('board-select: critical edge outside the chain stays at z-10', c.edgeZ(edge(c, '01-09__01-13')) === 'z-10');
          const inn = c.edgeClass(edge(c, '01-07__01-08'));
          check('board-select: critical edge inside the chain is cyan', inn.includes('stroke-cyan-600') && !inn.includes('stroke-yellow'), inn);
          check('board-select: no card gets a yellow ring', c.cards.every((k) => !c.cardClass(k).includes('ring-yellow')));
        });

        await step('board-select: panel fields', async () => {
          const c = make(SHOP);
          check('board-select: panel getters are quiet with no selection', c.panelKey === '' && c.panelDeps.length === 0 && c.panelWaiters.length === 0 && c.hasPanelDeps === false && c.hasPanelWaiters === false);
          c.select('01-07');
          check('board-select: panelKey', c.panelKey === '01-07', c.panelKey);
          check('board-select: panelTitle', c.panelTitle === 'Việc 01-07', c.panelTitle);
          check('board-select: panelEpic', c.panelEpic === '01 · Xuất báo cáo đơn hàng', c.panelEpic);
          check('board-select: panelMilestone falls back to a dash', c.panelMilestone === '—', c.panelMilestone);
          check('board-select: panelWave', c.panelWave === '2', c.panelWave);
          check('board-select: panelStatus is glyph and word', c.panelStatus === '⟳︎ bị chặn', c.panelStatus);
          check('board-select: panelCritical is có', c.panelCritical === 'có', c.panelCritical);
          check('board-select: panelRequirements', c.panelRequirements === 'BOARD-12, BOARD-09', c.panelRequirements);
          check('board-select: panelPath is repo-relative', c.panelPath === '.planning/phases/01-xuat-bao-cao/01-07-PLAN.md', c.panelPath);
          check('board-select: panelDeps keys', same(c.panelDeps.map((d) => d.key), ['01-03', '01-04']), JSON.stringify(c.panelDeps));
          check('board-select: panelDeps are all in view', c.panelDeps.every((d) => d.inView === true));
          check('board-select: panelDeps text is key and word', same(c.panelDeps.map((d) => d.text), ['01-03 · sẵn sàng', '01-04 · sẵn sàng']), JSON.stringify(c.panelDeps.map((d) => d.text)));
          check('board-select: panelWaiters keys', same(c.panelWaiters.map((d) => d.key), ['01-08', '01-10', '01-11']), JSON.stringify(c.panelWaiters));
          check('board-select: hasPanelDeps and hasPanelWaiters', c.hasPanelDeps === true && c.hasPanelWaiters === true);
          c.select('01-01');
          check('board-select: panelCritical is không for a non-critical item', c.panelCritical === 'không');
          check('board-select: no requirements reads as a dash', c.panelRequirements === '—');
          check('board-select: 01-01 has no dependencies', c.hasPanelDeps === false && c.panelDeps.length === 0);
          c.select('01-14');
          check('board-select: a leaf has no waiters', c.hasPanelWaiters === false && c.panelWaiters.length === 0);
        });

        await step('board-select: stand-in item', async () => {
          const c = make(baseOk({ items: [mk('03', '03', 0, 'san_sang', { placeholder: true })], counts: { total: 1, xong: 0, san_sang: 1, bi_chan: 0, waves: 1 } }));
          c.select('03');
          check('board-select: stand-in panelPath', c.panelPath === '— (phase chưa có plan)', c.panelPath);
          check('board-select: milestone on a plain item is a dash', c.panelMilestone === '—');
          c.apply(baseOk({ items: [mk('03', '03', 0, 'san_sang', { milestone: 'v1.1', placeholder: true })] }));
          check('board-select: panelMilestone shows the milestone', c.panelMilestone === 'v1.1', c.panelMilestone);
        });

        await step('board-select: neighbours outside the view', async () => {
          const c = make(SMOKE_02);
          c.select('02-01');
          check('board-select: out-of-view dependency is named', same(c.panelDeps, [{ key: '01-02', inView: false, text: '01-02 · xong · phase 01 (ngoài bộ lọc)' }]), JSON.stringify(c.panelDeps));
          check('board-select: in-view waiters are buttons', same(c.panelWaiters.map((d) => [d.key, d.inView]), [['02-02', true], ['02-03', true]]), JSON.stringify(c.panelWaiters));
          c.focusItem('01-02');
          check('board-select: focusItem ignores a key outside the view', c.sel === '02-01', `sel ${c.sel}`);
          const d = make(baseOk({ items: [mk('02-01', '02', 0, 'san_sang', { waits_on: ['ghost'] })] }));
          d.select('02-01');
          check('board-select: an unknown neighbour is still listed', same(d.panelDeps, [{ key: 'ghost', inView: false, text: 'ghost (ngoài bộ lọc)' }]), JSON.stringify(d.panelDeps));
        });

        await step('board-select: focus return', async () => {
          const c = make(SHOP);
          focused.length = 0;
          c.clearSelection();
          check('board-select: clearSelection with nothing selected does nothing', focused.length === 0 && c.sel === '');
          c.select('01-07');
          check('board-select: select does not move focus', focused.length === 0, JSON.stringify(focused));
          c.clearSelection();
          check('board-select: clearSelection clears', c.sel === '' && c.selectedItem === null && same(Object.keys(c.lit), []));
          check('board-select: clearSelection refocuses the card', same(focused, ['board-item-01-07']), JSON.stringify(focused));
          focused.length = 0;
          c.select('01-07');
          c.focusItem('01-04');
          check('board-select: focusItem selects the neighbour', c.sel === '01-04' && c.pressed(card(c, '01-04')) === 'true' && c.pressed(card(c, '01-07')) === 'false');
          check('board-select: focusItem moves focus', same(focused, ['board-item-01-04']), JSON.stringify(focused));
          focused.length = 0;
          c.focusItem('01-04');
          check('board-select: focusItem keeps an already selected card selected', c.sel === '01-04' && same(focused, ['board-item-01-04']), `${c.sel} ${JSON.stringify(focused)}`);
          c.clearSelection();
          check('board-select: Esc returns to the last selected card', same(focused, ['board-item-01-04', 'board-item-01-04']), JSON.stringify(focused));
        });

        await step('board-select: selection survives the collapse switch and a reload', async () => {
          const c = make(SHOP);
          c.select('01-07');
          const before = keysOf(c.lit);
          c.toggleCollapse();
          check('board-select: toggleCollapse keeps the selection', c.sel === '01-07' && c.selectedItem.key === '01-07' && same(keysOf(c.lit), before));
          check('board-select: the kept selection points at the rebuilt card', c.selectedItem === card(c, '01-07'));
          c.toggleCollapse();
          check('board-select: toggling back keeps it too', c.sel === '01-07' && classOf(c, '01-02').includes('opacity-25'));
          c.apply(SHOP);
          check('board-select: a reload that still has the key keeps the selection', c.sel === '01-07' && c.hasSelection === true && same(keysOf(c.lit), before));
          c.apply(baseOk({ items: [mk('01-01', '01', 0, 'xong')] }));
          check('board-select: a response without the key clears the selection', c.sel === '' && c.hasSelection === false && c.selectedItem === null && same(Object.keys(c.lit), []));
          const d = make(SHOP);
          d.select('01-07');
          await d.load();
          check('board-select: load() with the key present keeps it', d.sel === '01-07');
        });

        await step('board-select: card label', async () => {
          const c = make(SHOP);
          check('board-select: critical card label ends with the road text', c.cardLabel(card(c, '01-07')).endsWith(', trên đường găng'), c.cardLabel(card(c, '01-07')));
          check('board-select: plain card label has no road text', !c.cardLabel(card(c, '01-01')).includes('đường găng'));
        });
      } finally {
        ctx.AB.api = savedApi;
        ctx.AB.focusLater = savedFocus;
        ctx.localStorage = savedStorage;
      }

      // --- source checks ---
      const src = fs.readFileSync(boardJsPath, 'utf8');
      const html = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/screens/board.html'), 'utf8');
      check('board-select: board.js says ngoài bộ lọc and trên đường găng', src.includes('ngoài bộ lọc') && src.includes('trên đường găng'));
      check('board-select: no arrow function or template literal in board.js', src.indexOf('=' + '>') === -1 && src.indexOf(String.fromCharCode(96)) === -1);
      check('board-select: only read-only handlers in board.html', !/@click="(?!focusItem|clearSelection|select|reload|load|toggleCollapse)/.test(html), 'unexpected @click handler');
      check('board-select: the panel holds no form field', !/<aside[\s\S]*?<(input|textarea|select|form)\b[\s\S]*?<\/aside>/.test(html));
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
    name: 'task-load-seq',
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

      const resolvers = [];
      const urls = [];
      ctx.AB.api = function (method, url) {
        urls.push(method + ' ' + url);
        return new Promise((r) => {
          resolvers.push(r);
        });
      };
      const settle = async () => {
        await Promise.resolve();
        await Promise.resolve();
      };
      const mk = (rev) => ({ id: 1, slug: 'sua-app', status: 'planned', rev, steps: [], runs: [], actions: [], controls: [] });

      const c = factories.task();
      c.id = '1';
      c.pinned = true;
      c.loadPipeline = function () {};

      // Two polls in flight; the second (newer) answers first.
      c.load();
      c.load();
      check('seq: two GETs sent', resolvers.length === 2 && urls.every((u) => u === 'GET /api/tasks/1'), JSON.stringify(urls));
      const second = mk('R2');
      const first = mk('R1');
      resolvers[1]({ ok: true, status: 200, data: second });
      await settle();
      check('seq: newer response applied', c.task === second, `got rev ${c.task && c.task.rev}`);
      resolvers[0]({ ok: true, status: 200, data: first });
      await settle();
      check('seq: late older response dropped', c.task === second, `got rev ${c.task && c.task.rev}`);
      check('seq: _taskApplied is the newer seq', c._taskApplied === 2 && c._taskSeq === 2, `applied=${c._taskApplied} seq=${c._taskSeq}`);

      // A late older error does not mark the fresh state stale either.
      c.load();
      c.load();
      resolvers[3]({ ok: true, status: 200, data: mk('R4') });
      await settle();
      resolvers[2]({ ok: false, status: 500, data: null });
      await settle();
      check('seq: late older error dropped', c.task && c.task.rev === 'R4' && c.stale === false, `rev=${c.task && c.task.rev} stale=${c.stale}`);

      // A single load still applies.
      c.load();
      const single = mk('R5');
      resolvers[4]({ ok: true, status: 200, data: single });
      await settle();
      check('seq: single load applies', c.task === single && c.loaded === true && c.notFound === false, `rev=${c.task && c.task.rev}`);

      // In order (older first) both apply, the newer last.
      c.load();
      c.load();
      resolvers[5]({ ok: true, status: 200, data: mk('R6') });
      await settle();
      check('seq: in-order older applies', c.task && c.task.rev === 'R6', `rev=${c.task && c.task.rev}`);
      resolvers[6]({ ok: true, status: 200, data: mk('R7') });
      await settle();
      check('seq: in-order newer applies', c.task && c.task.rev === 'R7', `rev=${c.task && c.task.rev}`);

      // A 404 still sets notFound.
      c.load();
      resolvers[7]({ ok: false, status: 404, data: { error: 'không có' } });
      await settle();
      check('seq: 404 sets notFound', c.notFound === true && c.loaded === true, `notFound=${c.notFound}`);

      // A fresh component: first load error before anything applied shows the error.
      const d = factories.task();
      d.id = '1';
      d.pinned = true;
      d.loadPipeline = function () {};
      d.load();
      resolvers[8]({ ok: false, status: 500, data: null });
      await settle();
      check('seq: first error shown', d.error === 'HTTP 500' && d.loaded === false, `error="${d.error}"`);
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
        c.totalText === '4 lượt chạy · 12 phút · $3.75',
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
      check('totalText uses total (26 runs)', c.totalText === '26 lượt chạy · 13 phút · $2.60', `got "${c.totalText}"`);

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
  {
    // 06-17: team line, history, change-team dialog, engine lines, ledger labels, total helper,
    // quota banner and re-run dialog of the task screen.
    name: 'task-team-quota',
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

      const savedApi = ctx.AB.api;
      const savedFocus = ctx.AB.focusLater;
      const savedLock = ctx.AB.lockScroll;
      const calls = [];
      let respond = () => ({ ok: false, status: 0, data: null });
      ctx.AB.api = function (method, url, body) {
        calls.push({ method, url, body });
        return Promise.resolve(respond(method, url, body));
      };
      const focused = [];
      ctx.AB.focusLater = function (id) {
        focused.push(id);
      };
      let locked = false;
      ctx.AB.lockScroll = function (on) {
        locked = Boolean(on);
      };
      const ok = (data, status = 200) => ({ ok: status >= 200 && status < 300, status, data });

      try {
        const PRIMARY = 'bg-indigo-600';
        const RC = { lead: 'claude', dev: 'claude', qa: 'claude', plan_review: 'claude', code_review: 'claude' };
        const TEAMS = [
          { name: 'mac-dinh', source: 'builtin', description: 'Mọi vai trò dùng claude.', summary: 'tất cả: claude', roles: RC },
          { name: 'tiet-kiem', source: 'builtin', description: 'Dev dùng deepseek.', summary: 'claude: lead, qa, review plan, review code · deepseek: dev', roles: { ...RC, dev: 'deepseek' } },
          { name: 'doi-thu', source: 'file', description: 'Thử.', summary: 'claude: lead, qa · người Hà: dev', roles: { ...RC, dev: 'người:ha' } },
        ];
        const ENGINES = [
          { name: 'claude', readiness: 'san_sang', word: 'sẵn sàng' },
          { name: 'deepseek', readiness: 'thieu_key', word: 'thiếu key' },
          { name: 'agy', readiness: 'san_sang', word: 'sẵn sàng' },
        ];
        const STAFF = [{ id: 'ha', display_name: 'Hà' }];
        const routes = () => (method, url) => {
          if (url === '/api/teams') return ok({ teams: TEAMS, errors: [], builtin_names: ['mac-dinh', 'tiet-kiem'], project_defaults: {} });
          if (url === '/api/engines') return ok({ engines: ENGINES });
          if (url === '/api/staff') return ok({ staff: STAFF, errors: [] });
          return { ok: false, status: 0, data: null };
        };

        const APPROVE = { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null };
        const MORE = { id: 'auto_more_round', label: 'Chạy thêm 1 vòng', enabled: true, reason: null, run_id: null };
        const END = { id: 'auto_end', label: 'Dừng hẳn', enabled: true, reason: null, run_id: null };
        const RUN_DEV = { id: 'run_dev', label: 'Chạy Dev', enabled: true, reason: null };
        const RUN_QA = { id: 'run_qa', label: 'Chạy QA', enabled: true, reason: null };
        const CHANGE = { id: 'change_team', label: 'Đổi đội hình', enabled: true, reason: null, run_id: null };
        const RERUN = { id: 'quota_rerun', label: 'Chạy lại bước này bằng đội hình…', enabled: true, reason: null, run_id: null };
        const HDONE = { id: 'human_done', label: 'Xong', enabled: true, reason: null, run_id: null };
        const HRET = { id: 'human_return', label: 'Trả lại', enabled: true, reason: null, run_id: null };
        const QUOTA_DETAIL = 'Run 7 (dev) trên engine deepseek dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine.';
        const QUOTA = { run_id: 7, role: 'dev', stage: 'implement', step_name: 'Code', engine: 'deepseek', detail: QUOTA_DETAIL, retry_label: 'Chạy Dev' };
        const T = {
          id: 1,
          slug: 'sua-app',
          status: 'planned',
          current_step: 'dev',
          actions: [RUN_DEV, CHANGE, RERUN],
          controls: [],
          auto: { state: 'off', workflow_auto: false },
          workflow: { name: 'mac-dinh', summary: 'plan', auto: false },
          team: { name: 'doi-thu', summary: 'claude: lead, qa · người Hà: dev', description: 'Thử.', missing: false, legacy: false },
          team_history: [],
          quota: null,
          rev: 'R',
          steps: [],
          runs: [],
        };
        const fresh = (task, pipe) => {
          const c = factories.task();
          c.id = '1';
          c.task = task;
          if (pipe) c.pipe = pipe;
          return c;
        };

        // ---- team line (D-13) ----
        let c = fresh({ ...T });
        check('team: normal line', c.teamLineText === 'Đội hình: doi-thu · claude: lead, qa · người Hà: dev', `got "${c.teamLineText}"`);
        check('team: line shown, not missing', c.hasTeamLine === true && c.teamMissing === false);
        c.task = { ...T, team: { name: 'mac-dinh', summary: 'tất cả: claude', description: null, missing: false, legacy: true }, team_history: [] };
        check('team: legacy line', c.teamLineText === 'Đội hình: mac-dinh (task tạo trước Phase 6 — mọi vai trò chạy bằng claude)', `got "${c.teamLineText}"`);
        check('team: legacy is not "missing" and has no history', c.teamMissing === false && c.hasTeamHistory === false && c.teamHistoryLines.length === 0);
        c.task = { ...T, team: { name: 'cu', summary: null, description: null, missing: true, legacy: false } };
        check('team: removed-team line', c.teamLineText === 'Đội hình: cu — không còn trong teams.json', `got "${c.teamLineText}"`);
        check('team: removed-team hint', c.teamMissing === true && c.teamMissingText === 'Chọn đội hình khác bằng Đổi đội hình trước khi chạy bước tiếp.', c.teamMissingText);
        check('team: the button stays available for a removed team', c.teamControls.length === 1 && c.teamControls[0].id === 'change_team');
        c.task = { ...T, team: undefined };
        check('team: no team in the payload hides the line and the button', c.hasTeamLine === false && c.teamLineText === '');
        c.task = { ...T, actions: [RUN_DEV] };
        check('team: no change_team action, no button', c.teamControls.length === 0);

        // ---- history: oldest first, no cap, who in words ----
        const at1 = new Date(2026, 9, 5, 14, 7).getTime();
        const at2 = new Date(2026, 0, 12, 9, 30).getTime();
        c.task = { ...T, team_history: [
          { at: at1, by: 'dashboard', from: 'mac-dinh', to: 'tiet-kiem' },
          { at: at2, by: 'telegram', from: 'tiet-kiem', to: 'doi-thu' },
        ] };
        check('history: two lines in payload order', JSON.stringify(c.teamHistoryLines) === JSON.stringify([
          '05/10 14:07 · bạn qua dashboard: mac-dinh → tiet-kiem',
          '12/01 09:30 · bạn qua Telegram: tiet-kiem → doi-thu',
        ]), JSON.stringify(c.teamHistoryLines));
        check('history: shown when there is a change', c.hasTeamHistory === true);
        const many = [];
        for (let i = 0; i < 25; i++) many.push({ at: at1 + i * 60000, by: 'dashboard', from: 'a' + i, to: 'b' + i });
        c.task = { ...T, team_history: many };
        check('history: long history shows every line (no cap)', c.teamHistoryLines.length === 25);

        // ---- change-team dialog ----
        respond = routes();
        c = fresh({ ...T, team_history: [] });
        check('change: closed at start', c.teamDlgOpen === false && c.teamDlgOkLabel === 'Đổi đội hình');
        calls.length = 0;
        focused.length = 0;
        await c.openChangeTeam();
        check('change: opens, scroll locked', c.teamDlgOpen === true && locked === true);
        check('change: fetches teams, engines and staff when it opens', calls.map((x) => x.url).sort().join(',') === '/api/engines,/api/staff,/api/teams', calls.map((x) => x.url).join(','));
        check('change: title', c.teamDlgTitle === 'Đổi đội hình cho task sua-app', c.teamDlgTitle);
        check('change: body', c.teamDlgBody === 'Đội hình mới áp dụng từ bước kế tiếp; các run đã chạy giữ nguyên engine của chúng. Việc đổi được ghi lại trong lịch sử.', c.teamDlgBody);
        check('change: current team preselected', c.teamDlgPick === 'doi-thu' && c.teamDlgChecked(TEAMS[2]) === true && c.teamDlgChecked(TEAMS[0]) === false);
        check('change: the current team carries the chip đang dùng', c.teamDlgChips(TEAMS[2]).indexOf('đang dùng') >= 0 && c.teamDlgChips(TEAMS[0]).indexOf('đang dùng') === -1, JSON.stringify(c.teamDlgChips(TEAMS[2])));
        check('change: source chips', c.teamDlgChips(TEAMS[0]).join('|') === 'có sẵn' && c.teamDlgChips(TEAMS[2]).join('|') === 'đang dùng|từ teams.json', c.teamDlgChips(TEAMS[2]).join('|'));
        check('change: initial focus on the selected radio', focused[focused.length - 1] === 'ab-tdlg-radio-doi-thu', focused.join(','));
        check('change: confirm disabled while the selection is the current team', c.teamDlgOkDisabled === true && c.teamDlgReason === 'Chọn đội hình khác đội hình hiện tại.' && c.hasTeamDlgReason === true, c.teamDlgReason);
        check('change: readiness lines like Giao việc', JSON.stringify(c.teamDlgReadinessLines(TEAMS[1])) === JSON.stringify(['! deepseek: thiếu key']) && c.teamDlgReadinessLines(TEAMS[0]).length === 0, JSON.stringify(c.teamDlgReadinessLines(TEAMS[1])));
        check('change: staff names reach the engine texts', ctx.AB.engineText('người:ha') === 'người Hà', ctx.AB.engineText('người:ha'));
        check('change: no step line outside the re-run dialog', c.teamDlgStepLine(TEAMS[1]) === '');
        const before = calls.length;
        c.confirmChangeTeam();
        check('change: confirm on the current team sends nothing', calls.length === before);
        c.pickDlgTeam('tiet-kiem');
        check('change: another team enables the confirm', c.teamDlgOkDisabled === false && c.teamDlgReason === '' && c.hasTeamDlgReason === false);

        // in flight: label, disabled controls, one POST with {team, rev}
        let release;
        const gateP = new Promise((r) => { release = r; });
        respond = (method, url) => (method === 'POST' ? gateP.then(() => ok({ ok: true, from: 'doi-thu', to: 'tiet-kiem' })) : routes()(method, url));
        const posting = c.confirmChangeTeam();
        check('change: in flight reads Đang đổi...', c.teamDlgOkLabel === 'Đang đổi...' && c.teamDlgOkDisabled === true && c.busyAction === 'change_team');
        check('change: in flight blocks every task action', c.actionDisabled(RUN_DEV) === true);
        check('change: Escape is ignored while in flight', (c.teamDlgCancel(), c.teamDlgOpen === true));
        release();
        await posting;
        const post = calls.filter((x) => x.method === 'POST').pop();
        check('change: POST /api/tasks/1/team with {team, rev}', post.method === 'POST' && post.url === '/api/tasks/1/team' && JSON.stringify(post.body) === JSON.stringify({ team: 'tiet-kiem', rev: 'R' }), JSON.stringify(post));
        check('change: success closes the dialog, unlocks scroll, returns focus', c.teamDlgOpen === false && locked === false && focused[focused.length - 1] === 'ab-act-change_team', focused.join(','));
        check('change: success message', c.message === 'Đã đổi đội hình: doi-thu → tiet-kiem.', c.message);
        check('change: busy cleared after the answer', c.busyAction === '' && c.teamDlgBusy === false);

        // refusal: the server reason inside the dialog and on the button's reason line
        respond = routes();
        await c.openChangeTeam();
        c.pickDlgTeam('mac-dinh');
        const REFUSAL = 'Đang có run chạy — chờ run xong hoặc Kill rồi mới đổi đội hình.';
        respond = (method, url) => (method === 'POST' ? ok({ error: REFUSAL, kind: 'busy' }, 409) : routes()(method, url));
        await c.confirmChangeTeam();
        check('change: refusal stays in the dialog', c.teamDlgOpen === true && c.hasTeamDlgError === true && c.teamDlgError === REFUSAL, c.teamDlgError);
        check('change: refusal shows on the button reason line', c.changeTeamReason === REFUSAL && c.showReason(CHANGE) === true, c.changeTeamReason);
        c.teamDlgCancel();
        check('change: cancel closes without a second POST', c.teamDlgOpen === false && calls.filter((x) => x.method === 'POST').length === 2);
        const CHANGE_OFF = { ...CHANGE, enabled: false, reason: 'Đang có run chạy — chờ run xong hoặc Kill rồi mới đổi đội hình.' };
        c.task = { ...T, actions: [RUN_DEV, CHANGE_OFF, RERUN] };
        check('change: a disabled action shows the server reason, button disabled', c.actionDisabled(CHANGE_OFF) === true && c.changeTeamReason === CHANGE_OFF.reason);
        check('change: Đổi đội hình is secondary', !c.actionClass(CHANGE).includes(PRIMARY));

        // teams that cannot load
        respond = () => ({ ok: false, status: 0, data: null });
        c = fresh({ ...T });
        await c.openChangeTeam();
        check('change: a failed teams list shows the load error and no confirm', c.teamDlgLoadFailed === true && c.teamDlgTeams.length === 0 && c.teamDlgOkDisabled === true);
        c.teamDlgCancel();

        // ---- strip exclusions and the primary order ----
        c = fresh({ ...T, actions: [RUN_DEV, CHANGE, RERUN, HDONE, HRET, RUN_QA] });
        check('strip: excludes change_team, quota_rerun, human_done, human_return', JSON.stringify(c.stripActions.map((x) => x.id)) === JSON.stringify(['run_dev', 'run_qa']), JSON.stringify(c.stripActions.map((x) => x.id)));

        // ---- engine lines, ledger labels, human rows ----
        ctx.AB.setStaffNames({ ha: 'Hà' });
        c = fresh({ ...T });
        check('engine: planned card reads dự kiến deepseek', c.flowCardEngine({ key: 'code', engine: 'deepseek', engine_planned: true }) === 'dự kiến deepseek');
        check('engine: a card with a run shows the engine verbatim', c.flowCardEngine({ key: 'plan', engine: 'claude', engine_planned: false }) === 'claude' && c.flowCardHasEngine({ engine: 'claude' }) === true);
        check('engine: human run reads người Hà', c.flowCardEngine({ key: 'code', engine: 'người:ha', engine_planned: false }) === 'người Hà', c.flowCardEngine({ engine: 'người:ha' }));
        check('engine: planned human reads dự kiến người Hà', c.flowCardEngine({ engine: 'người:ha', engine_planned: true }) === 'dự kiến người Hà');
        check('engine: a card without an engine shows no line', c.flowCardHasEngine({ key: 'qa' }) === false && c.flowCardEngine(null) === '');
        check('engine: human card metrics while waiting', c.flowCardMetrics({ has_run: true, engine: 'người:ha', engine_planned: false, state: 'cho_nguoi', duration_ms: 90 * 60000 }) === 'đã chờ 1 giờ 30 phút', c.flowCardMetrics({ has_run: true, engine: 'người:ha', state: 'cho_nguoi', duration_ms: 90 * 60000 }));
        check('engine: human card metrics once done', c.flowCardMetrics({ has_run: true, engine: 'người:ha', engine_planned: false, state: 'xong', duration_ms: 5 * 60000 }) === '5 phút · không có token');
        check('engine: agent card metrics unchanged', c.flowCardMetrics({ has_run: true, engine: 'deepseek', state: 'xong', duration_ms: 120000, cost_usd: 0.5 }) === '2 phút · $0.50');
        check('ledger: label with the engine', c.rowLabel({ label: 'Vòng 1 · code', engine: 'deepseek' }) === 'Vòng 1 · code · deepseek', c.rowLabel({ label: 'Vòng 1 · code', engine: 'deepseek' }));
        check('ledger: human label', c.rowLabel({ label: 'Vòng 2 · code', engine: 'người:ha' }) === 'Vòng 2 · code · người Hà');
        check('ledger: a row with no engine keeps the server label', c.rowLabel({ label: 'Vòng 1 · code' }) === 'Vòng 1 · code');
        const HUMAN_ROW = { run_id: 9, label: 'Vòng 2 · code', engine: 'người:ha', human: true, state: 'xong', verdict: 'xong', turns: null, duration_ms: (3 * 24 + 2) * 3600000, cost_usd: null, input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, counts: null };
        check('ledger: human row metrics for a 3-day wait', c.rowMetrics(HUMAN_ROW) === '3 ngày 2 giờ · không có token', c.rowMetrics(HUMAN_ROW));
        const AGENT_ROW = { run_id: 8, label: 'Vòng 1 · code', engine: 'deepseek', human: false, state: 'xong', verdict: 'xong', turns: 6, duration_ms: 180000, cost_usd: 1.25, input_tokens: 10, output_tokens: 5, cache_read_tokens: 0, cache_write_tokens: 0, counts: null };
        check('ledger: agent row metrics unchanged', c.rowMetrics(AGENT_ROW) === '6 lượt · 3 phút · $1.25', c.rowMetrics(AGENT_ROW));
        c.runId = 9;
        check('ledger: selected human row token line', c.tokenLineFor(HUMAN_ROW) === 'Token: không có — run do người làm.', c.tokenLineFor(HUMAN_ROW));
        c.runId = 8;
        check('ledger: selected agent row token line unchanged', c.tokenLineFor(AGENT_ROW) === 'Token: vào 10 · ra 5 · cache 0 / 0', c.tokenLineFor(AGENT_ROW));
        const LEDGER = (n) => ({ groups: [], total: { runs: 3, duration_ms: 600000, cost_usd: 2.5, cost_unknown: 0, human_runs: n } });
        c.pipe = { groups: [], cards: {}, ledger: LEDGER(1) };
        check('total: new wording without claude', c.totalText === '3 lượt chạy · 10 phút · $2.50', c.totalText);
        check('total: human helper', c.humanRunsNote === 'Không tính thời gian chờ của 1 lượt do người làm.' && c.hasHumanRunsNote === true, c.humanRunsNote);
        c.pipe = { groups: [], cards: {}, ledger: LEDGER(0) };
        check('total: no helper without human runs', c.humanRunsNote === '' && c.hasHumanRunsNote === false);

        // ---- quota banner (D-15 / D-16 / D-18) ----
        c = fresh({ ...T, quota: QUOTA, actions: [RUN_DEV, CHANGE, RERUN] });
        check('quota: banner shows for a manual task', c.hasQuota === true && c.showBanner === true);
        check('quota: heading', c.bannerHeading === 'Hết quota engine deepseek', c.bannerHeading);
        check('quota: body 1 is the server detail verbatim', c.bannerBody === QUOTA_DETAIL, c.bannerBody);
        check('quota: body 2', c.quotaBody2 === 'Chạy lại bằng đội hình khác sẽ tiếp tục trên thư mục làm việc hiện tại: giữ nguyên diff đang có, đọc lại .plan.md (cùng .handover.md nếu có).', c.quotaBody2);
        check('quota: buttons without autopilot', JSON.stringify(c.bannerControls.map((x) => x.id)) === JSON.stringify(['quota_rerun', 'quota_later']), JSON.stringify(c.bannerControls.map((x) => x.id)));
        check('quota: button labels', c.bannerControls[0].label === 'Chạy lại bước này bằng đội hình…' && c.bannerControls[1].label === 'Thử lại sau');
        check('quota: re-run is primary, Thử lại sau secondary', c.actionClass(RERUN).includes(PRIMARY) && !c.actionClass(c.bannerControls[1]).includes(PRIMARY));
        check('quota: the strip keeps the step button and is secondary under the primary', JSON.stringify(c.stripActions.map((x) => x.id)) === JSON.stringify(['run_dev']) && !c.actionClass(RUN_DEV).includes(PRIMARY), JSON.stringify(c.stripActions.map((x) => x.id)));
        check('quota: the D-03 callouts are not rendered for this cause', c.isBlocked === false && c.isQaFix === false);
        c.task = { ...T, status: 'blocked', quota: QUOTA };
        check('quota: no D-03 blocked callout even for a blocked status', c.isBlocked === false);

        // Dừng hẳn only with autopilot's auto_end
        c.task = { ...T, quota: QUOTA, controls: [END], auto: { state: 'stopped', workflow_auto: true, stop_reason: 'het_quota', stop_word: 'hết quota', stop_detail: QUOTA_DETAIL, stopped_in: 'build' } };
        check('quota: Dừng hẳn appears with auto_end', JSON.stringify(c.bannerControls.map((x) => x.id)) === JSON.stringify(['quota_rerun', 'quota_later', 'auto_end']), JSON.stringify(c.bannerControls.map((x) => x.id)));
        check('quota: the autopilot stop banner gives way to the quota banner', c.bannerHeading === 'Hết quota engine deepseek' && c.bannerBody === QUOTA_DETAIL);
        check('quota: autopilot badge reads chờ bạn · hết quota', c.autoBadgeText === 'Autopilot: chờ bạn · hết quota', c.autoBadgeText);
        const RELEAD = { id: 'relead', label: 'Chạy lại Lead', enabled: true, reason: null };
        c.task = { ...c.task, actions: [RUN_DEV, RELEAD, CHANGE, RERUN] };
        check('quota: the strip keeps Chạy lại Lead while the quota banner shows', c.stripActions.some((x) => x.id === 'relead') === true, JSON.stringify(c.stripActions.map((x) => x.id)));

        // Thử lại sau: client only, collapses to the hint, nothing is sent
        c = fresh({ ...T, quota: QUOTA, actions: [RUN_DEV, CHANGE, RERUN] });
        calls.length = 0;
        c.onAction(c.bannerControls[1]);
        check('postpone: no API call (D-16)', calls.length === 0, JSON.stringify(calls));
        check('postpone: collapses to the hint', c.quotaPostponed === true && c.bannerBody === 'Đã hoãn. Khi quota hồi phục, bấm Chạy Dev ở dưới để chạy lại bằng engine deepseek.', c.bannerBody);
        check('postpone: the buttons and body 2 are gone, the heading stays', c.bannerControls.length === 0 && c.quotaBody2 === '' && c.bannerHeading === 'Hết quota engine deepseek');
        check('postpone: no dialog, no busy action', c.teamDlgOpen === false && c.busyAction === '');
        check('postpone: the step button becomes the strip primary', c.actionClass(RUN_DEV).includes(PRIMARY), c.actionClass(RUN_DEV));
        const reloaded = fresh({ ...T, quota: QUOTA, actions: [RUN_DEV, CHANGE, RERUN] });
        check('postpone: a reload shows the full banner again', reloaded.quotaPostponed === false && reloaded.bannerControls.length === 2);
        c.task = { ...T, quota: { ...QUOTA, run_id: 8 }, actions: [RUN_DEV, CHANGE, RERUN] };
        check('postpone: a newer quota stop shows the full banner', c.quotaPostponed === false && c.bannerControls.length === 2);
        c.task = { ...T, quota: { ...QUOTA, retry_label: null }, actions: [RUN_DEV, CHANGE, RERUN] };
        c.quotaPostponedRun = 7;
        check('postpone: hint without a named step button', c.bannerBody === 'Đã hoãn. Khi quota hồi phục, bấm nút chạy lại của bước ở dưới để chạy lại bằng engine deepseek.', c.bannerBody);

        // re-run dialog
        respond = routes();
        c = fresh({ ...T, team: { name: 'tiet-kiem', summary: 's', description: 'd', missing: false, legacy: false }, quota: QUOTA, actions: [RUN_DEV, CHANGE, RERUN] });
        calls.length = 0;
        focused.length = 0;
        await c.openQuotaRerun();
        check('rerun: opens as a dialog with a team list fetched on open', c.teamDlgOpen === true && c.rerunOpen === true && calls.map((x) => x.url).sort().join(',') === '/api/engines,/api/staff,/api/teams', calls.map((x) => x.url).join(','));
        check('rerun: title', c.teamDlgTitle === 'Chạy lại bước Code bằng đội hình nào?', c.teamDlgTitle);
        check('rerun: body', c.teamDlgBody === 'Đội hình bạn chọn thành đội hình của task từ bước này trở đi. Engine mới tiếp tục từ thư mục làm việc hiện tại, không làm lại từ đầu.', c.teamDlgBody);
        check('rerun: no preselection, even for the current team', c.teamDlgPick === '' && TEAMS.every((t) => c.teamDlgChecked(t) === false));
        check('rerun: the current team is not chipped đang dùng here', c.teamDlgChips(TEAMS[1]).indexOf('đang dùng') === -1);
        check('rerun: confirm disabled until a team is chosen', c.teamDlgOkDisabled === true && c.teamDlgReason === 'Chọn một đội hình.' && c.teamDlgOkLabel === 'Chạy lại bước này' && c.teamDlgCancelLabel === 'Không chạy lại', c.teamDlgReason);
        check('rerun: initial focus on the first radio', focused[focused.length - 1] === 'ab-tdlg-radio-mac-dinh', focused.join(','));
        check('rerun: option line for tiet-kiem', c.teamDlgStepLine(TEAMS[1]) === 'Bước Code sẽ chạy bằng: deepseek', c.teamDlgStepLine(TEAMS[1]));
        check('rerun: option line for mac-dinh', c.teamDlgStepLine(TEAMS[0]) === 'Bước Code sẽ chạy bằng: claude');
        check('rerun: option line names a person', c.teamDlgStepLine(TEAMS[2]) === 'Bước Code sẽ chạy bằng: người Hà', c.teamDlgStepLine(TEAMS[2]));
        check('rerun: chip vẫn là deepseek for tiet-kiem only', c.teamDlgChips(TEAMS[1]).indexOf('vẫn là deepseek') >= 0 && c.teamDlgChips(TEAMS[0]).every((x) => x.indexOf('vẫn là') === -1) && c.teamDlgChips(TEAMS[2]).every((x) => x.indexOf('vẫn là') === -1), JSON.stringify(c.teamDlgChips(TEAMS[1])));
        c.pickRerunTeam('mac-dinh');
        check('rerun: a pick enables the confirm', c.teamDlgOkDisabled === false && c.teamDlgReason === '');

        // a review-stage quota maps the role key (plan-review → plan_review)
        const cr = fresh({ ...T, quota: { ...QUOTA, role: 'code-review', stage: 'code-review', step_name: 'Review code', engine: 'claude' }, actions: [CHANGE, RERUN] });
        await cr.openQuotaRerun();
        check('rerun: review roles map to the team role keys', cr.teamDlgStepLine(TEAMS[1]) === 'Bước Review code sẽ chạy bằng: claude' && cr.teamDlgChips(TEAMS[1]).indexOf('vẫn là claude') >= 0, cr.teamDlgStepLine(TEAMS[1]));
        cr.teamDlgCancel();

        // refusal inside the dialog and under the banner button
        const RERUN_REFUSAL = 'Dev đang chờ commit — không chạy lại được lúc này.';
        respond = (method, url) => (method === 'POST' ? ok({ error: RERUN_REFUSAL, kind: 'not_allowed' }, 409) : routes()(method, url));
        await c.confirmQuotaRerun();
        check('rerun: refusal stays in the dialog', c.teamDlgOpen === true && c.teamDlgError === RERUN_REFUSAL && c.teamDlgBusy === false, c.teamDlgError);
        check('rerun: refusal shows under the banner button', c.reasonText(RERUN) === RERUN_REFUSAL && c.showReason(RERUN) === true, c.reasonText(RERUN));
        let postBody = null;
        let release2;
        const gate2 = new Promise((r) => { release2 = r; });
        respond = (method, url, body) => {
          if (method === 'POST') {
            postBody = { url, body };
            return gate2.then(() => ok({ job_id: 5 }, 202));
          }
          return routes()(method, url);
        };
        const rerunning = c.confirmQuotaRerun();
        check('rerun: in flight reads Đang giao...', c.teamDlgOkLabel === 'Đang giao...' && c.teamDlgOkDisabled === true && c.busyAction === 'quota_rerun');
        release2();
        await rerunning;
        check('rerun: POST /api/tasks/1/quota-rerun with {team, rev}', postBody && postBody.url === '/api/tasks/1/quota-rerun' && JSON.stringify(postBody.body) === JSON.stringify({ team: 'mac-dinh', rev: 'R' }), JSON.stringify(postBody));
        check('rerun: success message', c.message === 'Đã đổi sang đội hình mac-dinh và giao lại bước Code (job 5).', c.message);
        check('rerun: success closes, returns focus to the banner button', c.teamDlgOpen === false && c.busyAction === '' && focused[focused.length - 1] === 'ab-act-quota_rerun' && c.quotaRefusal === '', focused.join(','));

        // ---- primary priority: Duyệt plan > quota re-run > Chạy thêm 1 vòng > first strip action ----
        c = fresh({ ...T, quota: QUOTA, controls: [APPROVE, MORE, END], actions: [RUN_DEV, CHANGE, RERUN], auto: { state: 'stopped', workflow_auto: true, stop_reason: 'het_quota', stop_word: 'hết quota', stop_detail: QUOTA_DETAIL, stopped_in: 'build' } },
          { groups: [], cards: {}, gate: { present: true, state: 'cho_ban', result: 'chờ bạn', at_ms: Date.now() }, current_card: 'ban', ledger: { groups: [], total: null } });
        check('priority: Duyệt plan is the only primary when both show', c.actionClass(APPROVE).includes(PRIMARY) && !c.actionClass(RERUN).includes(PRIMARY) && !c.actionClass(MORE).includes(PRIMARY) && !c.actionClass(RUN_DEV).includes(PRIMARY));
        c = fresh({ ...T, quota: QUOTA, controls: [MORE, END], actions: [RUN_DEV, CHANGE, RERUN], auto: { state: 'stopped', workflow_auto: true, stop_reason: 'het_quota', stop_word: 'hết quota', stop_detail: QUOTA_DETAIL, stopped_in: 'build' } });
        check('priority: the quota re-run beats Chạy thêm 1 vòng and the strip', c.actionClass(RERUN).includes(PRIMARY) && !c.actionClass(MORE).includes(PRIMARY) && !c.actionClass(RUN_DEV).includes(PRIMARY));
        c = fresh({ ...T, controls: [MORE, END], actions: [RUN_DEV, CHANGE, RERUN], auto: { state: 'stopped', workflow_auto: true, stop_reason: 'het_vong', stop_word: 'hết vòng', stop_detail: 'x', stopped_in: 'build' } });
        check('priority: without a quota stop Chạy thêm 1 vòng stays the primary', c.actionClass(MORE).includes(PRIMARY) && !c.actionClass(RUN_DEV).includes(PRIMARY));
        c = fresh({ ...T });
        check('priority: with nothing above, the first enabled strip action is primary', c.actionClass(RUN_DEV).includes(PRIMARY));
        check('colour: the new controls use no red', ![RERUN, CHANGE, { id: 'quota_later' }].some((a) => c.actionClass(a).includes('rose')));
        check('quota: no banner when the payload has none', c.hasQuota === false && c.showBanner === false && c.bannerControls.length === 0);
      } finally {
        ctx.AB.api = savedApi;
        ctx.AB.focusLater = savedFocus;
        ctx.AB.lockScroll = savedLock;
        ctx.AB.setStaffNames({});
      }
    },
  },
  {
    // 06-18: the human step callout of the task screen (waiting / rejected, Xong / Trả lại, copy
    // buttons, push failure, autopilot badge).
    name: 'task-human',
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

      const savedApi = ctx.AB.api;
      const savedFocus = ctx.AB.focusLater;
      const savedLock = ctx.AB.lockScroll;
      const calls = [];
      let respond = () => ({ ok: false, status: 0, data: null });
      ctx.AB.api = function (method, url, body) {
        calls.push({ method, url, body });
        return Promise.resolve(respond(method, url, body));
      };
      ctx.AB.focusLater = function () {};
      ctx.AB.lockScroll = function () {};
      const ok = (data, status = 200) => ({ ok: status >= 200 && status < 300, status, data });
      const copied = [];
      ctx.navigator = { clipboard: { writeText: (t) => { copied.push(t); return Promise.resolve(); } } };
      const tick = async () => { for (let i = 0; i < 4; i++) await Promise.resolve(); };

      try {
        const PRIMARY = 'bg-indigo-600';
        const WAIT_MS = (3 * 24 + 2) * 3600000;
        const BRANCH = 'task/sua-app';
        const HW = {
          mode: 'waiting', staff_id: 'ha', staff_name: 'Hà', notes: 'Zalo 09xx', step: 'implement', step_name: 'Code', round: 1,
          branch: BRANCH, since_ms: 1, waiting_ms: WAIT_MS, report_paths: [],
          push: { ok: true, command: 'git push -u origin ' + BRANCH, error: null },
        };
        const HR = { ...HW, mode: 'rejected', step: 'fix', step_name: 'Sửa', round: 2, report_paths: ['.qa-report.md'] };
        const HDONE = { id: 'human_done', label: 'Xong', enabled: true, reason: null, run_id: null };
        const HRET = { id: 'human_return', label: 'Trả lại', enabled: true, reason: null, run_id: null };
        const HDONE_OFF = { ...HDONE, enabled: false, reason: 'Bấm Trả lại trước: kết quả lần trước của Hà chưa đạt.' };
        const HRET_OFF = { ...HRET, enabled: false, reason: 'Chỉ trả lại khi kết quả của Hà chưa đạt.' };
        const RUN_DEV = { id: 'run_dev', label: 'Chạy Dev', enabled: false, reason: 'Đang chờ Hà làm — bấm Xong khi họ báo xong.' };
        const KILL = { id: 'kill', label: 'Kill', enabled: true, reason: null, run_id: 3 };
        const APPROVE = { id: 'approve', label: 'Duyệt plan', enabled: true, reason: null, run_id: null };
        const T = {
          id: 1,
          slug: 'sua-app',
          status: 'planned',
          current_step: 'dev',
          group: 'running',
          actions: [RUN_DEV, HDONE, HRET_OFF],
          controls: [],
          auto: { state: 'off', workflow_auto: false },
          workflow: { name: 'mac-dinh', summary: 'plan', auto: false },
          team: { name: 'doi-nguoi', summary: 'người Hà: dev', description: null, missing: false, legacy: false },
          team_history: [],
          quota: null,
          human: HW,
          rev: 'R',
          steps: [],
          runs: [],
        };
        const fresh = (task, pipe) => {
          const c = factories.task();
          c.id = '1';
          c.task = task;
          c.taskAt = Date.now();
          if (pipe) c.pipe = pipe;
          return c;
        };

        // ---- waiting mode: heading, body, contact, wait time ----
        let c = fresh({ ...T });
        check('human: callout shows for a waiting person', c.hasHuman === true && c.humanWaiting === true && c.humanRejected === false);
        check('human: waiting heading', c.humanHeading === 'Chờ Hà làm bước Code', c.humanHeading);
        check('human: waiting body', c.humanBody === 'Nhánh task/sua-app đã có .plan.md đã commit. Hà làm trên máy của họ rồi đẩy nhánh này lên remote. Khi họ báo xong, bấm Xong để lấy nhánh về và chạy tiếp quy trình.', c.humanBody);
        check('human: branch and contact', c.humanBranch === BRANCH && c.humanContact === 'Liên hệ: Zalo 09xx' && c.hasHumanContact === true, c.humanContact);
        c.task = { ...T, human: { ...HW, notes: undefined } };
        check('human: no contact line without notes', c.humanContact === '' && c.hasHumanContact === false);
        c.task = { ...T, human: { ...HW, round: 2 } };
        check('human: second round reads làm lại', c.humanHeading === 'Chờ Hà làm lại bước Code', c.humanHeading);
        c.task = { ...T, human: { ...HW, step: 'fix', step_name: 'Sửa' } };
        check('human: a fix step reads làm lại', c.humanHeading === 'Chờ Hà làm lại bước Sửa', c.humanHeading);
        c.task = { ...T, human: { ...HW, staff_name: 'Nguyễn Văn Ế' } };
        check('human: a long name with diacritics is kept whole', c.humanHeading === 'Chờ Nguyễn Văn Ế làm bước Code', c.humanHeading);
        c.task = { ...T };
        check('human: three-day wait', c.humanWaitText === 'Đã chờ 3 ngày 2 giờ.', c.humanWaitText);
        c.task = { ...T, human: { ...HW, waiting_ms: 59 * 60000 } };
        c.taskAt = Date.now() - 2 * 60000;
        check('human: the wait keeps counting between polls', c.humanWaitText === 'Đã chờ 1 giờ 01 phút.', c.humanWaitText);
        c.task = { ...T, human: { ...HW, waiting_ms: 5 * 60000 } };
        c.taskAt = Date.now();
        check('human: minutes under an hour', c.humanWaitText === 'Đã chờ 5 phút.', c.humanWaitText);

        // ---- tint, glyph, role ----
        c = fresh({ ...T });
        check('human: waiting tint is the blue cho_nguoi card string', c.humanCalloutClass.includes('border-blue-300') && c.humanCalloutClass.includes('bg-blue-50') && c.humanCalloutClass.includes('rounded-xl p-4 space-y-4') && !c.humanCalloutClass.includes('amber'), c.humanCalloutClass);
        check('human: waiting glyph is ◐ without a pulse', c.humanGlyph === ctx.AB.stepGlyph('cho_nguoi') && c.humanGlyph.indexOf('◐') === 0 && c.humanGlyphClass.indexOf('animate-pulse') === -1, c.humanGlyph);
        check('human: the task banner takes the cho_nguoi tint', c.bannerState === 'cho_nguoi' && c.bannerClass === ctx.AB.stepClass('cho_nguoi') && c.bannerGlyph === ctx.AB.stepGlyph('cho_nguoi'));

        // ---- buttons: one per mode, outside the strip, primary order ----
        check('human: waiting offers Xong only', JSON.stringify(c.humanButtons.map((a) => a.id)) === JSON.stringify(['human_done']) && c.hasHumanButtons === true);
        check('human: Xong label and helper', c.actionText(HDONE) === 'Xong' && c.humanHelper(HDONE) === 'Lấy nhánh task/sua-app từ remote vào thư mục làm việc rồi chạy tiếp quy trình.', c.humanHelper(HDONE));
        check('human: the strip excludes Xong and Trả lại', JSON.stringify(c.stripActions.map((a) => a.id)) === JSON.stringify(['run_dev']), JSON.stringify(c.stripActions.map((a) => a.id)));
        check('human: Xong is the primary, the strip step is not', c.actionClass(HDONE).includes(PRIMARY) && !c.actionClass(RUN_DEV).includes(PRIMARY));
        c = fresh({ ...T, controls: [APPROVE], actions: [RUN_DEV, HDONE, HRET_OFF] }, { groups: [], cards: {}, gate: { present: true, state: 'cho_ban', result: 'chờ bạn', at_ms: Date.now() }, current_card: 'ban', ledger: { groups: [], total: null } });
        check('human: Duyệt plan outranks Xong', c.actionClass(APPROVE).includes(PRIMARY) && !c.actionClass(HDONE).includes(PRIMARY));
        c = fresh({ ...T, actions: [KILL, RUN_DEV, HDONE, HRET_OFF] });
        check('human: Kill stays red, Xong is the only primary', c.actionClass(KILL).includes('bg-rose-600') && c.actionClass(HDONE).includes(PRIMARY) && !c.actionClass(RUN_DEV).includes(PRIMARY));
        c = fresh({ ...T, actions: [RUN_DEV, HDONE_OFF, HRET_OFF] });
        check('human: a disabled Xong shows the server reason', c.actionDisabled(HDONE_OFF) === true && c.reasonText(HDONE_OFF) === HDONE_OFF.reason && c.showReason(HDONE_OFF) === true);
        c = fresh({ ...T, actions: [RUN_DEV] });
        check('human: no Xong in the payload, no button', c.hasHumanButtons === false);

        // ---- Xong: in flight, success, refusal, no dialog ----
        c = fresh({ ...T });
        calls.length = 0;
        let release;
        const gate = new Promise((r) => { release = r; });
        respond = (method, url) => (method === 'POST' ? gate.then(() => ok({ job_id: 5 }, 202)) : { ok: false, status: 0, data: null });
        const pending = c.humanDone();
        check('Xong: in flight reads Đang lấy nhánh...', c.busyAction === 'human_done' && c.actionText(HDONE) === 'Đang lấy nhánh...', c.actionText(HDONE));
        check('Xong: every task action is disabled while it runs', c.actionDisabled(HDONE) === true && c.actionDisabled({ ...RUN_DEV, enabled: true }) === true && c.actionDisabled(KILL) === true);
        release();
        await pending;
        const post = calls.filter((x) => x.method === 'POST');
        check('Xong: POST /api/tasks/1/human/done with {rev}', post.length === 1 && post[0].url === '/api/tasks/1/human/done' && JSON.stringify(post[0].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(post));
        check('Xong: success message', c.message === 'Đã lấy nhánh về — quy trình chạy tiếp (job 5).', c.message);
        check('Xong: busy cleared, no refusal', c.busyAction === '' && c.humanRefusal === '');

        const REFUSAL = 'Chưa thấy nhánh task/sua-app trên remote — nhờ Hà đẩy nhánh lên rồi bấm Xong lại.';
        respond = (method) => (method === 'POST' ? ok({ error: REFUSAL, kind: 'not_allowed' }, 409) : { ok: false, status: 0, data: null });
        c.message = '';
        await c.humanDone();
        check('Xong: the server reason shows under the button verbatim', c.humanRefusal === REFUSAL && c.reasonText(HDONE) === REFUSAL && c.showReason(HDONE) === true, c.humanRefusal);
        check('Xong: the callout stays after a refusal', c.hasHumanButtons === true && c.hasHuman === true && c.busyAction === '' && c.message === '', c.message);

        calls.length = 0;
        respond = (method) => (method === 'POST' ? ok({ job_id: 6 }, 202) : { ok: false, status: 0, data: null });
        c.onAction(HDONE);
        await tick();
        check('Xong: onAction sends the POST and opens no dialog', calls.some((x) => x.method === 'POST' && x.url === '/api/tasks/1/human/done') && c.dialogOpen === false && c.teamDlgOpen === false && c.rawOpen === false);
        check('Xong: a success clears an earlier refusal', c.humanRefusal === '', c.humanRefusal);

        // a disabled Xong sends nothing
        calls.length = 0;
        c.task = { ...T, actions: [RUN_DEV, HDONE_OFF, HRET_OFF] };
        await c.humanDone();
        check('Xong: disabled sends nothing', calls.filter((x) => x.method === 'POST').length === 0);

        // ---- rejected mode ----
        c = fresh({ ...T, group: 'waiting', human: HR, actions: [RUN_DEV, HDONE_OFF, HRET] });
        check('rejected: mode flags', c.hasHuman === true && c.humanRejected === true && c.humanWaiting === false);
        check('rejected: heading', c.humanHeading === 'Kết quả của Hà chưa đạt', c.humanHeading);
        check('rejected: QA body', c.humanBody === 'QA kết luận cần sửa. Xem báo cáo bên dưới rồi bấm Trả lại để Hà làm lại.', c.humanBody);
        c.task = { ...c.task, human: { ...HR, report_paths: ['.code-review.md'] } };
        check('rejected: Review code body', c.humanBody === 'Review code kết luận sửa lại. Xem báo cáo bên dưới rồi bấm Trả lại để Hà làm lại.', c.humanBody);
        c.task = { ...c.task, human: HR };
        check('rejected: amber callout, ! glyph', c.humanCalloutClass.includes('bg-amber-50') && c.humanCalloutClass.includes('ring-amber-300') && !c.humanCalloutClass.includes('border-blue') && c.humanGlyph === ctx.AB.stepGlyph('vuong_mac'), c.humanCalloutClass);
        check('rejected: the report block', c.hasHumanReport === true && c.humanReportLabel === 'Báo cáo cần gửi cho Hà:' && c.humanReportText === '.qa-report.md' && JSON.stringify(c.humanReportPaths) === JSON.stringify(['.qa-report.md']));
        check('rejected: Trả lại only, primary, with its helper', JSON.stringify(c.humanButtons.map((a) => a.id)) === JSON.stringify(['human_return']) && c.actionClass(HRET).includes(PRIMARY) && c.humanHelper(HRET) === 'Đưa việc về cho Hà kèm báo cáo; bạn gửi báo cáo cho họ.', c.humanHelper(HRET));
        check('rejected: the task banner is not the blue tint', c.bannerState !== 'cho_nguoi');
        check('rejected: the strip excludes Trả lại', c.stripActions.every((a) => a.id !== 'human_return' && a.id !== 'human_done'));
        c.task = { ...c.task, human: { ...HR, report_paths: ['.qa-report.md', '.code-review.md'] } };
        check('rejected: two report paths are joined by a line break', c.humanReportText === '.qa-report.md\n.code-review.md', JSON.stringify(c.humanReportText));
        c.task = { ...c.task, human: HR };

        calls.length = 0;
        let release2;
        const gate2 = new Promise((r) => { release2 = r; });
        respond = (method) => (method === 'POST' ? gate2.then(() => ok({ job_id: 7 }, 202)) : { ok: false, status: 0, data: null });
        const returning = c.humanReturn();
        check('Trả lại: in flight reads Đang trả...', c.busyAction === 'human_return' && c.actionText(HRET) === 'Đang trả...' && c.actionDisabled(HRET) === true, c.actionText(HRET));
        release2();
        await returning;
        const post2 = calls.filter((x) => x.method === 'POST');
        check('Trả lại: POST /api/tasks/1/human/return with {rev}', post2.length === 1 && post2[0].url === '/api/tasks/1/human/return' && JSON.stringify(post2[0].body) === JSON.stringify({ rev: 'R' }), JSON.stringify(post2));
        check('Trả lại: success message', c.message === 'Đã trả việc lại cho Hà.', c.message);
        check('Trả lại: no dialog', c.dialogOpen === false && c.teamDlgOpen === false);
        respond = (method) => (method === 'POST' ? ok({ error: 'Đang có run chạy — chờ run xong hoặc bấm Kill', kind: 'busy' }, 409) : { ok: false, status: 0, data: null });
        await c.humanReturn();
        check('Trả lại: a refusal shows under the button', c.reasonText(HRET) === 'Đang có run chạy — chờ run xong hoặc bấm Kill' && c.humanRefusal === c.reasonText(HRET));

        // ---- push failure (D-25) ----
        c = fresh({ ...T });
        check('push: ok hides the block', c.humanPushFailed === false);
        c.task = { ...T, human: { ...HW, push: { ok: false, command: 'git push -u origin ' + BRANCH, error: 'permission denied' } } };
        check('push: failure shows the command and the line', c.humanPushFailed === true && c.humanPushCommand === 'git push -u origin task/sua-app' && c.humanPushLine === 'Chưa đẩy được nhánh lên remote — chạy lệnh này trong thư mục dự án rồi gửi nhánh cho Hà.', c.humanPushLine);
        check('push: the branch stays visible next to the command', c.humanBranch === BRANCH && c.humanBody.indexOf(BRANCH) >= 0);
        c.task = { ...T, human: { ...HW, push: null } };
        check('push: no push result hides the block', c.humanPushFailed === false && c.humanPushCommand === '');

        // ---- copy buttons ----
        c = fresh({ ...T });
        copied.length = 0;
        c.copyBranch();
        await tick();
        check('copy: branch name', copied[0] === BRANCH && c.humanCopyMessage === 'Đã sao chép tên nhánh' && c.hasHumanCopyMessage === true, c.humanCopyMessage);
        c.task = { ...T, human: { ...HR, report_paths: ['.qa-report.md', '.code-review.md'] } };
        c.copyReportPaths();
        await tick();
        check('copy: report paths', copied[1] === '.qa-report.md\n.code-review.md' && c.humanCopyMessage === 'Đã sao chép đường dẫn', c.humanCopyMessage);
        c.task = { ...T, human: { ...HW, push: { ok: false, command: 'git push -u origin ' + BRANCH, error: 'x' } } };
        c.copyPushCommand();
        await tick();
        check('copy: push command', copied[2] === 'git push -u origin task/sua-app' && c.humanCopyMessage === 'Đã sao chép lệnh', c.humanCopyMessage);
        const savedNav = ctx.navigator;
        ctx.navigator = {};
        c.humanCopyMessage = '';
        c.copyBranch();
        check('copy: without a clipboard the text is selected and the message still shows', c.humanCopyMessage === 'Đã sao chép tên nhánh', c.humanCopyMessage);
        ctx.navigator = { clipboard: { writeText: () => Promise.reject(new Error('denied')) } };
        c.humanCopyMessage = '';
        c.copyBranch();
        await tick();
        check('copy: a denied clipboard falls back to selecting', c.humanCopyMessage === 'Đã sao chép tên nhánh', c.humanCopyMessage);
        ctx.navigator = savedNav;

        // ---- autopilot badge ----
        c = fresh({ ...T, auto: { state: 'waiting_human', workflow_auto: true } });
        check('badge: Autopilot: chờ Hà làm', c.autoBadgeText === 'Autopilot: chờ Hà làm', c.autoBadgeText);
        check('badge: cho_nguoi tint, ◐ without a pulse', c.autoBadgeClass === ctx.AB.stepClass('cho_nguoi') && c.autoBadgeGlyph === ctx.AB.stepGlyph('cho_nguoi') && c.autoBadgeGlyphClass.indexOf('animate-pulse') === -1);
        c = fresh({ ...T, auto: { state: 'running', workflow_auto: true }, human: null });
        check('badge: other states are unchanged', c.autoBadgeText === 'Autopilot: đang chạy', c.autoBadgeText);

        // ---- staff names for the engine texts ----
        ctx.AB.setStaffNames({});
        c = fresh({ ...T });
        c.syncHumanName();
        check('names: the payload feeds `người:ha`', ctx.AB.engineText('người:ha') === 'người Hà', ctx.AB.engineText('người:ha'));
        ctx.AB.setStaffNames({ binh: 'Bình' });
        c.syncHumanName();
        check('names: existing names are kept', ctx.AB.engineText('người:binh') === 'người Bình' && ctx.AB.engineText('người:ha') === 'người Hà');
        const before = c.staffVer;
        c.syncHumanName();
        check('names: no churn when nothing changed', c.staffVer === before);

        // ---- a three-day wait on the card ----
        c = fresh({ ...T }, { groups: [], cards: {}, ledger: { groups: [], total: null } });
        const HCARD = { key: 'code', state: 'cho_nguoi', has_run: true, engine: 'người:ha', engine_planned: false, duration_ms: WAIT_MS, cost_usd: null };
        check('card: three-day wait reads 3 ngày 2 giờ', c.flowCardMetrics(HCARD) === 'đã chờ 3 ngày 2 giờ', c.flowCardMetrics(HCARD));

        // ---- no payload ----
        c = fresh({ ...T, human: null, actions: [RUN_DEV] });
        check('none: no callout, no buttons, no wait text', c.hasHuman === false && c.humanButtons.length === 0 && c.humanWaitText === '' && c.humanHeading === '' && c.bannerState !== 'cho_nguoi');
        check('none: the strip keeps its own button', c.stripActions.length === 1);
      } finally {
        ctx.AB.api = savedApi;
        ctx.AB.focusLater = savedFocus;
        ctx.AB.lockScroll = savedLock;
        ctx.AB.setStaffNames({});
        delete ctx.navigator;
      }
    },
  },
  {
    // 06-18: Thống kê `Theo engine` and the QA rate by Dev engine.
    name: 'stats-engines',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const statsJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/stats.js'), 'utf8');
      vm.runInContext(statsJsSrc, ctx, { filename: 'stats.js' });
      check('stats-engines: factory registered', typeof factories.stats === 'function');
      if (typeof factories.stats !== 'function') return;

      const savedApi = ctx.AB.api;
      try {
        const REPORT = {
          total: { runs: 10, input_tokens: 180000, output_tokens: 30000, cache_read_tokens: 1000, cache_write_tokens: 200, cost_usd: 1.5 },
          by_day: [], by_profile: [], by_project: [], by_role: [],
          engines: [
            { engine: 'claude', runs: 5, input: 120000, output: 30000, cache_read: 1000, cache_write: 200, cost: 1.5, priced: true },
            { engine: 'deepseek', runs: 3, input: 60000, output: 0, cache_read: 0, cache_write: 0, cost: 0, priced: false },
            { engine: 'người', runs: 2, input: 0, output: 0, cache_read: 0, cache_write: 0, cost: 0, priced: false },
          ],
          qa_by_dev_engine: [
            { engine: 'claude', qa_fix: 1, qa_total: 3 },
            { engine: 'deepseek', qa_fix: 0, qa_total: 2 },
            { engine: 'người', qa_fix: 2, qa_total: 2 },
          ],
        };
        const c = factories.stats();
        c.loaded = true;
        c.report = REPORT;

        // ---- Theo engine ----
        const rows = c.engineRows;
        check('engines: one row per engine in the server order', JSON.stringify(rows.map((r) => r.name)) === JSON.stringify(['claude', 'deepseek', 'người']), JSON.stringify(rows.map((r) => r.name)));
        check('engines: priced row line 1', rows[0].line1 === 'vào 120.000 · ra 30.000', rows[0].line1);
        check('engines: priced row line 2 shows the cost without hover', rows[0].line2 === '≈ $1.5000 · 5 run', rows[0].line2);
        check('engines: cache figures live in the title', rows[0].title === 'claude · cache 1.000 / 200 · ≈ $1.5000', rows[0].title);
        check('engines: the largest row fills the track', JSON.stringify(rows[0].style) === JSON.stringify({ width: '100%' }) && rows[0].hasBar === true && rows[0].noBar === false, JSON.stringify(rows[0].style));
        check('engines: bar width is the share of the largest in + out', JSON.stringify(rows[1].style) === JSON.stringify({ width: '40%' }), JSON.stringify(rows[1].style));
        check('engines: unpriced engine reads chưa có giá, never ≈ $0', rows[1].line2 === 'chưa có giá · 3 run' && rows.every((r) => r.line2.indexOf('≈ $0') === -1), rows[1].line2);
        check('engines: unpriced title has no cost', rows[1].title === 'deepseek · cache —', rows[1].title);
        check('engines: the person row has no bar and no tokens', rows[2].hasBar === false && rows[2].noBar === true && rows[2].line1 === 'không có token' && rows[2].line2 === '2 lượt do người làm', JSON.stringify(rows[2]));
        check('engines: hasEngineRows', c.hasEngineRows === true);
        check('engines: every row has a key for x-for', new Set(rows.map((r) => r.key)).size === 3);
        c.report = { ...REPORT, engines: [{ engine: 'claude', runs: 1, input: 0, output: 0, cache_read: 0, cache_write: 0, cost: 0, priced: true }] };
        check('engines: a lone row with no tokens gets an empty bar, no division by zero', c.engineRows[0].style.width === '0%', JSON.stringify(c.engineRows[0].style));
        const long = 'x'.repeat(60);
        c.report = { ...REPORT, engines: [{ engine: long, runs: 1, input: 5, output: 5, cache_read: 0, cache_write: 0, cost: 0, priced: false }] };
        check('engines: a long name is kept whole for the title attribute', c.engineRows[0].name === long);
        c.report = { ...REPORT, engines: undefined };
        check('engines: no engines list gives no rows', c.engineRows.length === 0 && c.hasEngineRows === false);

        // ---- QA rate ----
        c.report = REPORT;
        const rates = c.rateRows;
        check('rates: one row per entry in the server order', JSON.stringify(rates.map((r) => r.name)) === JSON.stringify(['claude', 'deepseek', 'người']));
        check('rates: 1 / 3 reads 33%', rates[0].line1 === '33%' && rates[0].line2 === '1 / 3 lần QA', JSON.stringify(rates[0]));
        check('rates: 0 / 2 reads 0% with an empty bar', rates[1].line1 === '0%' && rates[1].line2 === '0 / 2 lần QA' && JSON.stringify(rates[1].style) === JSON.stringify({ width: '0%' }), JSON.stringify(rates[1]));
        check('rates: 2 / 2 fills the track', rates[2].line1 === '100%' && JSON.stringify(rates[2].style) === JSON.stringify({ width: '100%' }), JSON.stringify(rates[2]));
        check('rates: bar width is the rate', Math.abs(parseFloat(rates[0].style.width) - 100 / 3) < 0.0001, rates[0].style.width);
        check('rates: hasRates', c.hasRates === true && c.noRates === false);
        c.report = { ...REPORT, qa_by_dev_engine: [] };
        check('rates: no verdicts leaves the card with its empty line', c.hasRates === false && c.noRates === true && c.rateRows.length === 0);
        c.report = { ...REPORT, qa_by_dev_engine: undefined };
        check('rates: no list is the same as an empty one', c.hasRates === false && c.rateRows.length === 0);
        c.report = { ...REPORT, qa_by_dev_engine: [{ engine: 'claude', qa_fix: 0, qa_total: 0 }] };
        check('rates: a zero total never divides', c.rateRows[0].line1 === '—' && c.rateRows[0].style.width === '0%', JSON.stringify(c.rateRows[0]));

        // ---- empty window: the empty-state card replaces the body ----
        c.report = { ...REPORT, total: { ...REPORT.total, runs: 0 }, engines: [], qa_by_dev_engine: [] };
        check('empty: no runs shows the empty state and hides every card', c.isEmpty === true && c.hasData === false);
        c.report = REPORT;
        check('empty: with runs the cards show', c.isEmpty === false && c.hasData === true);

        // ---- filter change keeps the previous figures until the response; an error keeps them too ----
        let release;
        const gate = new Promise((r) => { release = r; });
        const urls = [];
        ctx.AB.api = (method, url) => {
          urls.push(url);
          return gate;
        };
        c.since = '7d';
        c.load();
        check('filter: the request carries the window', urls.length === 1 && urls[0].indexOf('since=7d') >= 0, urls.join(','));
        check('filter: previous figures stay while loading', c.report === REPORT && c.engineRows.length === 3 && c.rateRows.length === 3);
        const NEXT = { ...REPORT, engines: [REPORT.engines[0]], qa_by_dev_engine: [REPORT.qa_by_dev_engine[0]] };
        release({ ok: true, status: 200, data: NEXT });
        await gate;
        await Promise.resolve();
        check('filter: the response replaces both cards', c.engineRows.length === 1 && c.rateRows.length === 1, String(c.engineRows.length));
        ctx.AB.api = () => Promise.resolve({ ok: false, status: 500, data: { error: 'lỗi' } });
        c.load();
        await Promise.resolve();
        await Promise.resolve();
        check('error: after a first load a failure keeps the figures and shows the retry line', c.stale === true && c.hasError === false && c.engineRows.length === 1);
        const fresh = factories.stats();
        ctx.AB.api = () => Promise.resolve({ ok: false, status: 500, data: { error: 'Lỗi máy chủ' } });
        fresh.load();
        await Promise.resolve();
        await Promise.resolve();
        check('error: a failed first load shows the global error card, not the cards', fresh.hasError === true && fresh.hasData === false && fresh.isEmpty === false);
      } finally {
        ctx.AB.api = savedApi;
      }
    },
  },
  {
    // 06-18: Tổng quan hint, the blue banner for a person's step, the quota reason line.
    name: 'overview-human',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const overviewJsSrc = fs.readFileSync(path.join(repoRoot, 'agent-brain/assets/dashboard/js/overview.js'), 'utf8');
      vm.runInContext(overviewJsSrc, ctx, { filename: 'overview.js' });
      check('overview-human: factory registered', typeof factories.overview === 'function');
      if (typeof factories.overview !== 'function') return;

      const c = factories.overview();
      const AB = ctx.AB;
      const QD = 'Run 7 (dev) trên engine deepseek dừng vì hết quota hoặc bị giới hạn tốc độ. Agent-brain không tự đổi engine.';
      const human = { id: 4, group: 'running', auto_state: 'off', human_mode: 'waiting', human_name: 'Hà', banner: 'Chờ Hà làm', stop_detail: null, quota_detail: null, updated_at: 4000 };
      const rejected = { id: 5, group: 'waiting', auto_state: 'off', human_mode: 'rejected', human_name: 'Hà', banner: 'Chờ bạn · trả lại cho Hà', stop_detail: null, quota_detail: null, updated_at: 3000 };
      const quotaManual = { id: 3, group: 'waiting', auto_state: 'off', human_mode: null, banner: 'Chờ bạn · hết quota', stop_detail: null, quota_detail: QD, updated_at: 2000 };
      const quotaAuto = { id: 6, group: 'waiting', auto_state: 'stopped', human_mode: null, banner: 'Chờ bạn · hết quota', stop_detail: 'Autopilot dừng: ' + QD, quota_detail: QD, updated_at: 1900 };
      const gate = { id: 1, group: 'waiting', auto_state: 'gate', human_mode: null, banner: 'Chờ duyệt plan', stop_detail: null, quota_detail: null, updated_at: 1000 };
      c.tasks = [human, rejected, quotaManual, quotaAuto, gate];

      const running = c.groups.find((g) => g.key === 'running');
      check('overview-human: the Đang chạy hint names people too', running && running.hint === 'Agent hoặc nhân sự đang làm việc', running && running.hint);
      check('overview-human: the other hints are unchanged', c.groups.find((g) => g.key === 'waiting').hint === 'Duyệt plan hoặc xử lý chỗ autopilot dừng' && c.groups.find((g) => g.key === 'attention').hint === 'Vướng mắc, cần sửa, bị dừng hoặc lỗi');
      check('overview-human: a waiting person sits in Đang chạy', running && running.items.length === 1 && running.items[0].id === 4);
      check('overview-human: a quota stop sits in Chờ bạn', c.groups.find((g) => g.key === 'waiting').items.some((t) => t.id === 3));

      // ---- banner tint and glyph ----
      const cls = c.bannerClass(human);
      check('overview-human: the banner takes the cho_nguoi text tint', cls.includes(AB.stateText('cho_nguoi')) && cls.includes('text-blue-700'), cls);
      check('overview-human: no pulse on the banner', cls.indexOf('animate-pulse') === -1);
      check('overview-human: ◐ glyph shows for a waiting person', c.bannerHasGlyph(human) === true && c.bannerGlyph(human) === AB.stepGlyph('cho_nguoi') && c.bannerGlyph(human).indexOf('◐') === 0, c.bannerGlyph(human));
      for (const t of [rejected, quotaManual, gate, { group: 'done', banner: 'QA: Đạt' }]) {
        check('overview-human: other banners keep the plain class and no glyph (' + (t.banner || '') + ')', c.bannerClass(t).indexOf('text-blue') === -1 && c.bannerHasGlyph(t) === false && c.bannerGlyph(t) === '');
      }

      // ---- reason line ----
      check('overview-human: a manual quota stop shows quota_detail', c.hasReason(quotaManual) === true && c.reasonText(quotaManual) === QD, c.reasonText(quotaManual));
      check('overview-human: an autopilot stop shows stop_detail', c.hasReason(quotaAuto) === true && c.reasonText(quotaAuto) === 'Autopilot dừng: ' + QD, c.reasonText(quotaAuto));
      check('overview-human: the gate has no reason line', c.hasReason(gate) === false && c.reasonText(gate) === '');
      check('overview-human: a waiting person has no reason line', c.hasReason(human) === false);
      check('overview-human: a rejected result has no quota line', c.hasReason(rejected) === false);
      check('overview-human: no task is no reason', c.hasReason(null) === false && c.reasonText(undefined) === '');
      check('overview-human: autopilot stopped without a detail has no line', c.hasReason({ group: 'waiting', auto_state: 'stopped', stop_detail: null, quota_detail: null }) === false);
    },
  },
  {
    name: 'settings',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const settingsJsPath = path.join(repoRoot, 'agent-brain/assets/dashboard/js/settings.js');
      vm.runInContext(fs.readFileSync(settingsJsPath, 'utf8'), ctx, { filename: 'settings.js' });

      check('settings: factory registered', typeof factories.settings === 'function', 'factories.settings not found');
      if (typeof factories.settings !== 'function') return;

      const PLANTED = 'planted-key-value-0123456789';
      const ROLES = (dev) => ({ lead: 'claude', dev, qa: 'claude', plan_review: 'claude', code_review: 'claude' });
      const teamRow = (name, source, dev, extra) => Object.assign({
        name,
        source,
        description: 'Mô tả ' + name,
        roles: ROLES(dev),
        summary: 'tóm tắt ' + name,
        warnings: [],
        default_for: [],
        used_by: { projects: [], open_tasks: 0 },
      }, extra || {});
      const TEAMS = {
        teams: [
          teamRow('mac-dinh', 'builtin', 'claude'),
          teamRow('tiet-kiem', 'builtin', 'deepseek'),
          teamRow('doi-thu', 'file', 'người:ha', { warnings: ['Nhân sự ma không còn trong danh sách.'], used_by: { projects: ['web-ban-hang'], open_tasks: 2 } }),
          teamRow('ghi-de', 'override', 'claude'),
        ],
        errors: [],
        version: 'tv1',
        builtin_names: ['mac-dinh', 'tiet-kiem'],
      };
      const WORKFLOWS = {
        workflows: [
          { name: 'mac-dinh', source: 'builtin', summary: 's1', auto: true, limits: '3 vòng · 60 phút · $20', default_for: [], stages: [], edit: { stages: ['plan', 'plan-review', 'gate:you', 'implement', 'code-review', 'qa', 'fix'], loops: { plan: 'plan-review', implement: 'code-review', qa: 'fix' }, auto: true, max_rounds: 3, budget_minutes: 60, budget_usd: 20 } },
          { name: 'nhanh', source: 'file', summary: 's2', auto: false, limits: '60 phút · $20', default_for: ['Web bán hàng'], stages: [], edit: { stages: ['plan', 'gate:you', 'implement', 'qa'], loops: {}, auto: false, max_rounds: 3, budget_minutes: 60, budget_usd: 20 } },
        ],
        errors: [],
        version: 'wv1',
      };
      const STAFF = {
        staff: [{ id: 'ha', display_name: 'Hà', notes: 'Zalo 09xx', used_in: ['doi-b'] }],
        errors: [],
        version: 'sv1',
      };
      const secretEngine = (e) => Object.assign({ env: { DEEPSEEK_API_KEY: PLANTED }, secret_value: PLANTED, argv: ['--key', PLANTED] }, e);
      const ENGINES = {
        engines: [
          secretEngine({ name: 'claude', kind: 'claude', usage: 'tokens', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: 'claude', key_env: null, model: null }),
          secretEngine({ name: 'deepseek', kind: 'anthropic', usage: 'tokens', readiness: 'thieu_key', word: 'thiếu key', reason: 'Chưa đặt biến môi trường DEEPSEEK_API_KEY.', command: 'claude', key_env: 'DEEPSEEK_API_KEY', model: 'deepseek-chat' }),
          secretEngine({ name: 'agy', kind: 'agy', usage: 'tokens', readiness: 'thieu_lenh', word: 'thiếu lệnh', reason: 'Không tìm thấy lệnh agy trên máy.', command: 'agy', key_env: null, model: null }),
          secretEngine({ name: 'người', kind: 'người', usage: 'none', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: null, key_env: null, model: null, detail: '1 nhân sự' }),
        ],
      };

      let routes;
      let calls;
      const ok = (data) => ({ ok: true, status: 200, data: JSON.parse(JSON.stringify(data)) });
      const defaults = () => ({
        'GET /api/teams': () => ok(TEAMS),
        'GET /api/workflows': () => ok(WORKFLOWS),
        'GET /api/staff': () => ok(STAFF),
        'GET /api/engines': () => ok(ENGINES),
      });
      const savedApi = ctx.AB.api;
      ctx.AB.api = function (method, url, body) {
        calls.push({ method, url, body });
        const h = routes[method + ' ' + url];
        if (!h) return Promise.resolve({ ok: false, status: 404, data: null });
        return Promise.resolve(typeof h === 'function' ? h(body) : h);
      };
      async function fresh(over) {
        routes = Object.assign(defaults(), over || {});
        calls = [];
        const c = factories.settings();
        await c.load();
        await c.loadEngines();
        return c;
      }

      try {
        // ---- load, rows and chips ----
        let c = await fresh();
        check('settings: loaded after the three reads', c.loaded === true && c.loadedAttr === 'yes', `got ${c.loadedAttr}`);
        check('settings: starts on the Đội hình section', c.section === 'teams' && c.showTeams === true);
        const chips = (name) => JSON.stringify(c.teamRows.filter((r) => r.name === name)[0].chips);
        check('settings: builtin chips', chips('mac-dinh') === JSON.stringify(['có sẵn']), chips('mac-dinh'));
        check('settings: file chips', chips('doi-thu') === JSON.stringify(['từ teams.json']), chips('doi-thu'));
        check('settings: override chips', chips('ghi-de') === JSON.stringify(['ghi đè đội hình có sẵn']), chips('ghi-de'));
        const rowOf = (name) => c.teamRows.filter((r) => r.name === name)[0];
        check('settings: builtin row only copies', rowOf('mac-dinh').canEdit === false && rowOf('doi-thu').canEdit === true && rowOf('ghi-de').canEdit === true);
        check('settings: aria-label names kind and name', rowOf('tiet-kiem').copyLabel === 'Nhân bản đội hình tiet-kiem' && rowOf('doi-thu').editLabel === 'Sửa đội hình doi-thu' && rowOf('doi-thu').delLabel === 'Xóa đội hình doi-thu');
        check('settings: not-ready engine line from /api/engines', JSON.stringify(rowOf('tiet-kiem').warnings) === JSON.stringify(['deepseek: thiếu key']), JSON.stringify(rowOf('tiet-kiem').warnings));
        check('settings: missing staff line from the server', rowOf('doi-thu').warnings.indexOf('Nhân sự ma không còn trong danh sách.') >= 0, JSON.stringify(rowOf('doi-thu').warnings));
        check('settings: custom team exists so no helper', c.noCustomTeam === false);
        check('settings: workflow chips', JSON.stringify(c.workflowRows[1].chips) === JSON.stringify(['từ workflows.json', 'chạy tay']), JSON.stringify(c.workflowRows[1].chips));
        check('settings: workflow default line', c.workflowRows[1].defaultFor === 'Mặc định của: Web bán hàng', c.workflowRows[1].defaultFor);
        check('settings: staff usage line', c.staffRows[0].usage === 'Dùng trong đội hình: doi-b', c.staffRows[0].usage);

        // ---- copy names ----
        c.copyTeam('tiet-kiem');
        check('settings: copy name', c.tName === 'tiet-kiem-ban-sao' && c.teamMode === 'create', `got "${c.tName}"`);
        check('settings: copy prefills the source', c.tDev === 'deepseek' && c.tDesc === 'Mô tả tiet-kiem');
        c.cancelTeam();
        c.teams.push(teamRow('tiet-kiem-ban-sao', 'file', 'claude'));
        c.copyTeam('tiet-kiem');
        check('settings: second copy name', c.tName === 'tiet-kiem-ban-sao-2', `got "${c.tName}"`);
        c.cancelTeam();

        // ---- team editor rules ----
        c = await fresh();
        c.newTeam();
        check('settings: new team defaults every role to claude', ['tLead', 'tDev', 'tQa', 'tPlanReview', 'tCodeReview'].every((k) => c[k] === 'claude'));
        check('settings: first reason is the empty name', c.saveReason('team') === 'Nhập tên đội hình.', c.saveReason('team'));
        c.tName = 'Bad Name';
        check('settings: format reason', c.saveReason('team') === 'Tên chỉ gồm chữ thường không dấu, số và dấu gạch ngang (ví dụ tiet-kiem).', c.saveReason('team'));
        c.tName = 'doi-thu';
        check('settings: duplicate reason', c.saveReason('team') === "Đã có đội hình tên 'doi-thu'.", c.saveReason('team'));
        c.tName = 'mac-dinh';
        check('settings: builtin reason', c.saveReason('team') === "'mac-dinh' là đội hình có sẵn, không ghi đè được. Đặt tên khác.", c.saveReason('team'));
        c.tName = 'moi-1';
        check('settings: empty description reason', c.saveReason('team') === 'Nhập mô tả ngắn để chọn đội hình cho đúng.', c.saveReason('team'));
        c.tDesc = '   ';
        check('settings: blank description is empty', c.saveReason('team') === 'Nhập mô tả ngắn để chọn đội hình cho đúng.');
        c.tDesc = 'Đội hình mới';
        check('settings: valid draft has no reason', c.saveReason('team') === '' && c.saveDisabled('team') === false, c.saveReason('team'));
        c.tQa = 'deepseek';
        check('settings: QA must be claude', c.saveReason('team') === 'QA phải dùng engine claude.' && c.saveDisabled('team') === true, c.saveReason('team'));
        check('settings: QA error shows at once', c.fieldMsg('team', 'qa') === 'QA phải dùng engine claude.' && c.invalidAttr('team', 'qa') === 'true');
        check('settings: untouched empty name shows no error', c.fieldMsg('team', 'name') === '' && c.invalidAttr('team', 'name') === false);
        c.tQa = 'claude';
        check('settings: valid ring vs invalid ring', c.fieldClass('team', 'name').indexOf('ring-slate-500') >= 0);
        c.tName = 'Sai';
        c.onField('team', 'name');
        check('settings: touched invalid name gets the rose ring', c.fieldClass('team', 'name') === 'ring-2 ring-rose-600 dark:ring-rose-400', c.fieldClass('team', 'name'));
        c.tName = 'tên-có-dấu';
        check('settings: diacritics are refused', c.saveReason('team').indexOf('Tên chỉ gồm chữ thường') === 0);
        c.tName = 'a'.repeat(41);
        check('settings: 41 characters are refused', c.saveReason('team').indexOf('Tên chỉ gồm chữ thường') === 0);
        c.tName = 'doi-thu-moi';
        const devOpts = c.roleOptions('dev').map((o) => o.label);
        const leadOpts = c.roleOptions('lead').map((o) => o.label);
        check('settings: Dev options include người Hà', devOpts.indexOf('người Hà') >= 0, JSON.stringify(devOpts));
        check('settings: Lead options do not include a person', leadOpts.every((l) => l.indexOf('người') !== 0), JSON.stringify(leadOpts));
        check('settings: QA / reviews have no person either', ['qa', 'plan_review', 'code_review'].every((r) => c.roleOptions(r).every((o) => o.value.indexOf('người:') !== 0)));
        check('settings: not-ready option carries its word', leadOpts.indexOf('deepseek — thiếu key') >= 0 && leadOpts.indexOf('agy — thiếu lệnh') >= 0, JSON.stringify(leadOpts));
        check('settings: human value stays visible on a non-Dev role', (() => { c.tLead = 'người:ha'; const o = c.roleOptions('lead').map((x) => x.label); c.tLead = 'claude'; return o.indexOf('người Hà') >= 0; })());
        check('settings: editor lock line for a new draft', c.lockLine('team') === 'Đang sửa doi-thu-moi — lưu hoặc hủy trước khi sửa mục khác.', c.lockLine('team'));
        check('settings: actions locked while a draft is open', c.actionsLocked('team') === true && c.actionsLocked('staff') === false);
        check('settings: collision note when the name is also a workflow', (() => { c.tName = 'nhanh'; const n = c.collisionNote('team'); c.tName = 'doi-thu-moi'; return n.indexOf('Tên này cũng là một quy trình.') === 0; })());
        check('settings: save sends version, create and entry', (() => {
          routes['POST /api/teams/save'] = () => ok(Object.assign({ ok: true }, TEAMS));
          return true;
        })());
        await c.saveTeam();
        const post = calls.filter((x) => x.method === 'POST')[0];
        check('settings: POST /api/teams/save body', post && post.url === '/api/teams/save' && post.body.version === 'tv1' && post.body.create === true && post.body.entry.name === 'doi-thu-moi' && post.body.entry.roles.plan_review === 'claude' && post.body.entry.roles.code_review === 'claude', JSON.stringify(post));
        check('settings: success closes the editor and says so', c.teamMode === '' && c.statusText === 'Đã lưu đội hình doi-thu-moi.', `mode "${c.teamMode}" status "${c.statusText}"`);
        check('settings: editor closed unlocks the section', c.actionsLocked('team') === false);

        // ---- edit mode ----
        c.editTeam('doi-thu');
        check('settings: edit keeps the name and prefills', c.teamMode === 'edit' && c.tName === 'doi-thu' && c.tDev === 'người:ha' && c.readonlyAttr('team') === 'true');
        check('settings: edit mode has no name reason', c.saveReason('team') === '', c.saveReason('team'));
        check('settings: edit title and cancel label', c.editorTitle('team') === 'Sửa đội hình doi-thu' && c.cancelText('team') === 'Hủy sửa');
        check('settings: edited row is hidden', c.teamRows.filter((r) => r.name === 'doi-thu')[0].visible === false);
        c.cancelTeam();
        c.editTeam('mac-dinh');
        check('settings: a builtin cannot be edited', c.teamMode === '');

        // ---- conflict keeps the draft ----
        routes['POST /api/teams/save'] = { ok: false, status: 409, data: { error: 'teams.json vừa bị sửa ở nơi khác', code: 'conflict' } };
        c.newTeam();
        c.tName = 'xung-dot';
        c.tDesc = 'Bản nháp giữ lại';
        calls.length = 0;
        await c.saveTeam();
        check('settings: conflict keeps tName and tDesc', c.tName === 'xung-dot' && c.tDesc === 'Bản nháp giữ lại' && c.teamMode === 'create');
        check('settings: conflict heading', c.hasConflict('team') === true && c.conflictHeading('team') === 'teams.json vừa bị sửa ở nơi khác', c.conflictHeading('team'));
        check('settings: conflict body', c.conflictBody() === 'Đã tải lại danh sách. Bản nháp của bạn vẫn còn trong form — kiểm tra rồi lưu lại.');
        check('settings: conflict reloads the teams', calls.some((x) => x.method === 'GET' && x.url === '/api/teams'));
        check('settings: staff and workflow conflict headings', c.conflictHeading('workflow') === 'workflows.json vừa bị sửa ở nơi khác' && c.conflictHeading('staff') === 'file nhân sự vừa bị sửa ở nơi khác');
        c.cancelTeam();

        // ---- 422 refusal: field errors, at most 10 lines ----
        const errs = [{ field: 'name', message: 'Tên sai.' }];
        for (let i = 1; i <= 11; i++) errs.push({ field: null, message: 'Lỗi ' + i });
        routes['POST /api/teams/save'] = { ok: false, status: 422, data: { error: 'Không lưu được đội hình', errors: errs } };
        c.newTeam();
        c.tName = 'sai-ten';
        c.tDesc = 'x';
        await c.saveTeam();
        check('settings: refusal keeps the draft', c.teamMode === 'create' && c.tName === 'sai-ten');
        check('settings: refusal shows 10 lines then the remainder', c.refusalLines('team').length === 11 && c.refusalLines('team')[10] === '… và 2 lỗi khác', JSON.stringify(c.refusalLines('team')));
        check('settings: refusal heading', c.refusalHeading('team') === 'Không lưu được đội hình');
        check('settings: field error under its field', c.fieldMsg('team', 'name') === 'Tên sai.' && c.fieldInvalid('team', 'name') === true);
        c.onField('team', 'name');
        check('settings: typing clears that field error', c.fieldMsg('team', 'name') === '');
        c.cancelTeam();
        check('settings: cancel clears refusal', c.hasRefusal('team') === false && c.hasConflict('team') === false);

        // ---- invalid file blocks writes ----
        c = await fresh({ 'GET /api/teams': () => ok(Object.assign({}, TEAMS, { errors: ['Dòng 3: khóa lạ', 'Dòng 7: thiếu mô tả'] })) });
        check('settings: invalid file disables creation', c.actionsLocked('team') === true);
        check('settings: invalid file reason', c.fileReason('team') === 'teams.json đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi.', c.fileReason('team'));
        c.newTeam();
        check('settings: newTeam does nothing on an invalid file', c.teamMode === '');
        c.copyTeam('mac-dinh');
        check('settings: copy does nothing on an invalid file', c.teamMode === '');
        check('settings: built-ins stay listed', c.teamRows.length === 4);
        check('settings: invalid file lines', JSON.stringify(c.fileErrorLines('team')) === JSON.stringify(['Dòng 3: khóa lạ', 'Dòng 7: thiếu mô tả']));
        check('settings: closing line names the built-ins', c.teamFileClosing === 'Đang dùng các đội hình có sẵn (mac-dinh, tiet-kiem) cho đến khi bạn sửa xong file.', c.teamFileClosing);
        check('settings: other sections stay writable', c.actionsLocked('workflow') === false && c.actionsLocked('staff') === false);
        const manyErrs = [];
        for (let i = 1; i <= 12; i++) manyErrs.push('Dòng ' + i + ': lỗi');
        c = await fresh({ 'GET /api/teams': () => ok(Object.assign({}, TEAMS, { errors: manyErrs })) });
        check('settings: invalid file lines are capped at 10', c.fileErrorLines('team').length === 11 && c.fileErrorLines('team')[10] === '… và 2 lỗi khác');

        // ---- workflow editor ----
        c = await fresh();
        c.newWorkflow();
        const codeLoop = c.loops[1];
        check('settings: seven stages in the fixed order', c.stages.map((s) => s.key).join(',') === 'plan,plan-review,gate:you,implement,code-review,qa,fix');
        check('settings: loop disabled until both stages are checked', c.loopDisabled(codeLoop) === true && c.loopReason(codeLoop) === 'Cần chọn cả Code và Review code ở mục Các bước.', c.loopReason(codeLoop));
        check('settings: other loop reasons', c.loopReason(c.loops[0]) === 'Cần chọn cả Lên plan và Review plan ở mục Các bước.' && c.loopReason(c.loops[2]) === 'Cần chọn cả QA và Sửa ở mục Các bước.');
        check('settings: rounds disabled without a loop', c.roundsDisabled === true && c.roundsReason === 'Chưa bật vòng lặp nào.');
        c.toggleStage('code-review');
        check('settings: loop enabled with both stages', c.loopEnabled(codeLoop) === true && c.loopReason(codeLoop) === '');
        c.toggleLoop(codeLoop);
        check('settings: loop checked enables rounds', c.loopChecked(codeLoop) === true && c.roundsDisabled === false && c.roundsReason === '');
        c.wName = 'wf-moi';
        c.wMaxRounds = '11';
        check('settings: rounds 11 is refused', c.saveReason('workflow') === 'Số vòng phải là số nguyên từ 1 đến 10.', c.saveReason('workflow'));
        c.wMaxRounds = '0';
        check('settings: rounds 0 is refused', c.saveReason('workflow') === 'Số vòng phải là số nguyên từ 1 đến 10.');
        c.wMaxRounds = '2';
        c.wBudgetMinutes = '0';
        check('settings: minutes message', c.saveReason('workflow') === 'Ngân sách thời gian phải là số nguyên dương.');
        c.wBudgetMinutes = '1.5';
        check('settings: fractional minutes refused', c.saveReason('workflow') === 'Ngân sách thời gian phải là số nguyên dương.');
        c.wBudgetMinutes = '45';
        c.wBudgetUsd = '0';
        check('settings: usd message', c.saveReason('workflow') === 'Ngân sách chi phí phải là số dương.');
        c.wBudgetUsd = '12.5';
        check('settings: valid workflow draft', c.saveReason('workflow') === '', c.saveReason('workflow'));
        c.toggleStage('code-review');
        check('settings: unchecking a stage unchecks its loop', c.loopChecked(codeLoop) === false && c.anyLoopOn === false);
        c.toggleStage('plan');
        c.toggleStage('gate:you');
        c.toggleStage('implement');
        c.toggleStage('qa');
        check('settings: no stage reason', c.saveReason('workflow') === 'Chọn ít nhất một bước.', c.saveReason('workflow'));
        c.toggleStage('plan');
        c.toggleStage('plan-review');
        c.toggleLoop(c.loops[0]);
        c.toggleAuto();
        const entry = c.buildEntry('workflow');
        check('settings: workflow entry', JSON.stringify(entry) === JSON.stringify({ name: 'wf-moi', stages: ['plan', 'plan-review'], loops: { plan: 'plan-review' }, auto: true, budget_minutes: 45, budget_usd: 12.5, max_rounds: 2 }), JSON.stringify(entry));
        c.wName = 'nhanh';
        check('settings: workflow duplicate reason', c.saveReason('workflow') === "Đã có quy trình tên 'nhanh'.", c.saveReason('workflow'));
        c.wName = 'mac-dinh';
        check('settings: workflow builtin reason', c.saveReason('workflow') === "'mac-dinh' là quy trình có sẵn, không ghi đè được. Đặt tên khác.", c.saveReason('workflow'));
        c.wName = 'ghi-de';
        check('settings: workflow collision note with a team', c.collisionNote('workflow').indexOf('Tên này cũng là một đội hình.') === 0);
        c.cancelWorkflow();
        c.editWorkflow('nhanh');
        check('settings: workflow edit prefill', c.wMode === 'edit' && c.stageChecked('qa') === true && c.stageChecked('fix') === false && c.wMaxRounds === '3' && c.wBudgetUsd === '20');
        c.cancelWorkflow();
        c.copyWorkflow('mac-dinh');
        check('settings: workflow copy name and loops', c.wName === 'mac-dinh-ban-sao' && c.loopChecked(c.loops[2]) === true && c.stageChecked('fix') === true);
        c.cancelWorkflow();
        check('settings: workflow conflict heading', c.conflictHeading('workflow') === 'workflows.json vừa bị sửa ở nơi khác');
        check('settings: builtin workflow cannot be deleted', (() => { c.askDelete('workflow', 'mac-dinh'); return c.delKind === ''; })());

        // ---- staff editor and delete ----
        c = await fresh();
        c.askDelete('staff', 'ha');
        check('settings: staff delete dialog title', c.isDeleting('staff') && c.delTitle('staff') === 'Xóa nhân sự Hà?', c.delTitle('staff'));
        check('settings: staff confirm disabled with the usage reason', c.delConfirmDisabled('staff') === true && c.delConfirmReason('staff') === 'Đang dùng trong đội hình: doi-b. Đổi engine của vai trò đó trước rồi xóa.', c.delConfirmReason('staff'));
        check('settings: staff delete labels', c.delConfirmText('staff') === 'Xóa nhân sự' && c.delKeepText('staff') === 'Giữ nhân sự');
        await c.confirmDelete();
        check('settings: a disabled confirm sends nothing', calls.filter((x) => x.method === 'POST').length === 0);
        c.closeDelete();
        check('settings: closeDelete clears the dialog', c.delKind === '');
        routes['GET /api/staff'] = () => ok({ staff: [{ id: 'ha', display_name: 'Hà', notes: '', used_in: [] }], errors: [], version: 'sv2' });
        await c.loadStaff();
        c.askDelete('staff', 'ha');
        check('settings: unused staff confirm is enabled', c.delConfirmDisabled('staff') === false && c.staffRows[0].usage === 'Chưa dùng trong đội hình nào');
        routes['POST /api/staff/delete'] = () => ok({ ok: true, staff: [], errors: [], version: 'sv3' });
        calls.length = 0;
        await c.confirmDelete();
        const del = calls.filter((x) => x.method === 'POST')[0];
        check('settings: staff delete body', del && del.url === '/api/staff/delete' && del.body.id === 'ha' && del.body.version === 'sv2', JSON.stringify(del));
        check('settings: delete success message', c.statusText === 'Đã xóa nhân sự Hà.' && c.delKind === '' && c.staff.length === 0, c.statusText);
        check('settings: staff empty state', c.staffEmpty === true && c.staffHasRows === false);
        c.newStaff();
        check('settings: staff first reason', c.saveReason('staff') === 'Nhập mã nhân sự.');
        c.sId = 'Ha';
        check('settings: staff id format', c.saveReason('staff') === 'Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang.');
        c.sId = 'ha';
        check('settings: staff name reason', c.saveReason('staff') === 'Nhập tên hiển thị.');
        c.sName = 'Hà';
        check('settings: staff valid', c.saveReason('staff') === '');
        routes['GET /api/staff'] = () => ok(STAFF);
        await c.loadStaff();
        check('settings: staff duplicate id', c.saveReason('staff') === "Đã có nhân sự mã 'ha'.", c.saveReason('staff'));
        c.cancelStaff();
        routes['POST /api/teams/delete'] = { ok: false, status: 422, data: { error: 'Không lưu được đội hình', errors: [{ field: 'name', message: 'Đội hình có sẵn chỉ đọc — bấm Nhân bản để tạo bản riêng.' }] } };
        c.askDelete('team', 'doi-thu');
        check('settings: team delete extra lines', JSON.stringify(c.delExtra('team')) === JSON.stringify(['Đang là đội hình mặc định của: web-ban-hang.', '2 task chưa xong đang dùng đội hình này; sau khi xóa, các task đó cần Đổi đội hình trước khi chạy bước kế tiếp.']), JSON.stringify(c.delExtra('team')));
        await c.confirmDelete();
        check('settings: refused delete shows the server line and stays open', c.delError === 'Đội hình có sẵn chỉ đọc — bấm Nhân bản để tạo bản riêng.' && c.isDeleting('team'), c.delError);
        c.closeDelete();
        c.askDelete('team', 'ghi-de');
        check('settings: override delete note', c.delExtra('team').indexOf('Đội hình có sẵn ghi-de sẽ được dùng lại.') >= 0, JSON.stringify(c.delExtra('team')));
        c.closeDelete();

        // ---- reload ----
        c = await fresh();
        calls.length = 0;
        await c.reload();
        check('settings: Đọc lại re-reads teams, workflows and staff but not engines', calls.map((x) => x.url).sort().join(',') === '/api/staff,/api/teams,/api/workflows', calls.map((x) => x.url).join(','));
        check('settings: reload label idle', c.reloadLabel === 'Đọc lại');
        let release;
        const gate = new Promise((r) => { release = r; });
        const prevTeams = routes['GET /api/teams'];
        routes['GET /api/teams'] = () => gate.then(() => prevTeams());
        const pending = c.reload();
        check('settings: reload label in flight keeps the data', c.reloadLabel === 'Đang đọc...' && c.teams.length === 4 && c.loaded === true);
        release();
        await pending;
        check('settings: reload label back', c.reloadLabel === 'Đọc lại');
        routes['GET /api/teams'] = { ok: false, status: 500, data: { error: 'Lỗi nội bộ khi đọc cài đặt' } };
        await c.reload();
        check('settings: a failed reload keeps the data and marks it stale', c.stale === true && c.teams.length === 4 && c.loadError === '');
        routes['GET /api/teams'] = prevTeams;
        await c.reload();
        check('settings: a good reload clears stale', c.stale === false);
        routes['GET /api/workflows'] = { ok: false, status: 0, data: null };
        const c2 = factories.settings();
        await c2.load();
        check('settings: first load failure shows the error card', c2.hasError === true && c2.loaded === false && c2.loadedAttr === 'no' && c2.errorBody === 'Mất kết nối tới agent-brain serve. Bấm Thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.', c2.errorBody);
        check('settings: sections stay hidden until loaded', c2.showTeams === false);
        routes['GET /api/workflows'] = () => ok(WORKFLOWS);
        await c2.retry();
        check('settings: Thử lại recovers', c2.loaded === true && c2.hasError === false && c2.showTeams === true);

        // ---- sub-nav ----
        check('settings: four sections', c.sections.map((s) => s.label).join(',') === 'Đội hình,Quy trình,Nhân sự,Engine');
        c.showSection('staff');
        check('settings: showSection switches', c.showStaff === true && c.showTeams === false && c.navCurrent(c.sections[2]) === 'true' && c.navCurrent(c.sections[0]) === false);
        check('settings: active nav class', c.navClass(c.sections[2]).indexOf('bg-indigo-100') === 0 && c.navClass(c.sections[0]).indexOf('hover:bg-slate-100') >= 0);
        c.showSection('khong-co');
        check('settings: an unknown section is ignored', c.section === 'staff');

        // ---- engines: read-only, never a secret ----
        c = await fresh();
        const rows = c.engineRows;
        check('settings: four engine rows in order', rows.map((r) => r.name).join(',') === 'claude,deepseek,agy,người');
        check('settings: readiness keys and words', rows.map((r) => r.key + ':' + r.word).join(',') === 'san_sang:sẵn sàng,thieu_key:thiếu key,thieu_lenh:thiếu lệnh,san_sang:sẵn sàng', rows.map((r) => r.key).join(','));
        const detailOf = (r) => r.details.map((d) => d.label + d.value);
        check('settings: deepseek detail lines', JSON.stringify(detailOf(rows[1])) === JSON.stringify(['Lệnh: claude', 'Biến key: DEEPSEEK_API_KEY', 'Model: deepseek-chat']), JSON.stringify(detailOf(rows[1])));
        check('settings: claude and agy show only the command', JSON.stringify(detailOf(rows[0])) === JSON.stringify(['Lệnh: claude']) && JSON.stringify(detailOf(rows[2])) === JSON.stringify(['Lệnh: agy']));
        check('settings: người shows the staff count', JSON.stringify(detailOf(rows[3])) === JSON.stringify(['1 nhân sự']), JSON.stringify(detailOf(rows[3])));
        check('settings: server reason for a not-ready engine only', rows[1].reason === 'Chưa đặt biến môi trường DEEPSEEK_API_KEY.' && rows[2].reason === 'Không tìm thấy lệnh agy trên máy.' && rows[0].reason === '' && rows[0].hasReason === false);
        check('settings: exact engine descriptions', rows[0].description === 'Dùng profile của dự án (work hoặc personal), không có tài khoản riêng.' && rows[1].description === 'claude chạy trên máy với một endpoint định dạng Anthropic.' && rows[2].description === 'Antigravity chạy không giao diện (headless).' && rows[3].description === 'Nhân sự làm trên máy của họ và đẩy nhánh task lên remote.');
        const rendered = JSON.stringify({ rows, teams: c.teamRows, optionLabels: c.roleOptions('dev'), staff: c.staffRows });
        check('settings: a planted key value never reaches the engine rows or any other view', rendered.indexOf(PLANTED) === -1, 'planted value found in a rendered view');
        check('settings: the key variable NAME is shown, not its value', rendered.indexOf('DEEPSEEK_API_KEY') >= 0);
        routes['GET /api/engines'] = { ok: false, status: 500, data: null };
        const c3 = factories.settings();
        await c3.load();
        await c3.loadEngines();
        check('settings: engines failure keeps every row in chưa kiểm', c3.engineRows.every((r) => r.key === 'chua_kiem' && r.word === 'chưa kiểm') && c3.enginesFailed === true && c3.loaded === true);
        check('settings: no readiness warnings without engine data', c3.teamRows.every((r) => r.warnings.every((w) => w.indexOf('thiếu') === -1)));
        routes['GET /api/engines'] = () => ok(ENGINES);
        let release2;
        const gate2 = new Promise((r) => { release2 = r; });
        routes['GET /api/engines'] = () => gate2.then(() => ok(ENGINES));
        const checking = c3.checkEngines();
        check('settings: Kiểm tra lại in flight', c3.checkLabel === 'Đang kiểm tra...' && c3.enginesChecking === true);
        release2();
        await checking;
        check('settings: Kiểm tra lại done', c3.checkLabel === 'Kiểm tra lại' && c3.enginesFailed === false && c3.engineRows[1].key === 'thieu_key');
        check('settings: no engine write route is ever called', calls.every((x) => x.url.indexOf('/api/engines') !== 0 || x.method === 'GET'));
      } finally {
        ctx.AB.api = savedApi;
      }
    },
  },
  {
    // 06-24 (D-30, D-37, D-40, D-43, D-44): the Settings Engine section, the team editor options,
    // Giao việc and Thống kê are built from the configured engine list (GET /api/engines).
    name: 'engine-list',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const dash = (f) => path.join(repoRoot, 'agent-brain/assets/dashboard/js', f);
      for (const f of ['settings.js', 'assign.js', 'stats.js']) {
        vm.runInContext(fs.readFileSync(dash(f), 'utf8'), ctx, { filename: f });
      }
      const allThere = ['settings', 'assign', 'stats'].every((n) => typeof factories[n] === 'function');
      check('engine-list: factories registered', allThere);
      if (!allThere) return;

      const PLANTED = 'planted-gateway-key-9876543210';
      const withSecrets = (e) => Object.assign({ env: { GW_API_KEY: PLANTED }, secret_value: PLANTED, argv: ['--key', PLANTED], base_url: 'https://gw.invalid/x?token=' + PLANTED }, e);
      const E = {
        claude: withSecrets({ name: 'claude', kind: 'claude', usage: 'tokens', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: 'claude' }),
        deepseek: withSecrets({ name: 'deepseek', kind: 'anthropic', usage: 'tokens', readiness: 'thieu_key', word: 'thiếu key', reason: 'Chưa đặt biến môi trường DEEPSEEK_API_KEY.', command: 'claude', key_env: 'DEEPSEEK_API_KEY', model: 'deepseek-chat' }),
        gwThu: withSecrets({ name: 'gw-thu', kind: 'anthropic', usage: 'tokens', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: 'claude', key_env: 'GW_API_KEY', model: 'm-thu', label: 'Cổng thử nghiệm của công ty với một cái tên rất dài để kiểm tra việc xuống dòng', evidence: '2026-10-28 E-09' }),
        gwChua: withSecrets({ name: 'gw-chua', kind: 'anthropic', usage: 'tokens', readiness: 'chua_kiem', word: 'chưa kiểm', reason: 'Endpoint chưa có bằng chứng E-09 — chạy spikes/e09/run.sh rồi ghi "verified" cho engine này trong config.json.', command: 'claude', key_env: 'GW2_API_KEY', model: 'm-chua' }),
        claudeMax: withSecrets({ name: 'claude-max', kind: 'anthropic', usage: 'tokens', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: 'claude', pin_profile: 'personal', allowed_profiles: ['personal'] }),
        agy: withSecrets({ name: 'agy', kind: 'agy', usage: 'tokens', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: 'agy', allowed_profiles: ['work', 'personal'] }),
        dsh: withSecrets({ name: 'dsh', kind: 'dsh', usage: 'none', readiness: 'thieu_lenh', word: 'thiếu lệnh', reason: 'Container fake-dsh chưa chạy.', command: 'docker', model: 'headless', container: 'fake-dsh', allowed_profiles: ['personal'] }),
        nguoi: withSecrets({ name: 'người', kind: 'người', usage: 'none', readiness: 'san_sang', word: 'sẵn sàng', reason: null, command: null, detail: '1 nhân sự' }),
      };
      const ENGINES = { engines: [E.claude, E.deepseek, E.gwThu, E.gwChua, E.claudeMax, E.agy, E.dsh, E.nguoi] };
      const ROLES = (dev, extra) => Object.assign({ lead: 'claude', dev, qa: 'claude', plan_review: 'claude', code_review: 'claude' }, extra || {});
      const team = (name, dev, extra, warnings) => Object.assign({
        name, source: 'file', description: 'Mô tả ' + name, roles: ROLES(dev), summary: 'tóm tắt', warnings: warnings || [], default_for: [], used_by: { projects: [], open_tasks: 0 },
      }, extra || {});
      const TEAMS = {
        teams: [
          team('tiet-kiem', 'deepseek', { source: 'builtin' }, ["Engine 'deepseek' chưa khai báo trong config.json."]),
          team('chi-chua', 'gw-chua'),
          team('hon-hop', 'deepseek', { roles: ROLES('deepseek', { lead: 'gw-chua' }) }),
          team('ca-ba', 'gw-thu', { roles: ROLES('gw-thu', { lead: 'gw-chua', plan_review: 'dsh' }) }),
        ],
        errors: [],
        version: 'tv1',
        builtin_names: ['tiet-kiem'],
        project_defaults: {},
      };
      const STAFF = { staff: [{ id: 'ha', display_name: 'Hà', notes: '', used_in: [] }], errors: [], version: 'sv1' };
      const ok = (data) => ({ ok: true, status: 200, data: JSON.parse(JSON.stringify(data)) });
      let engines = ENGINES;
      const calls = [];
      const savedApi = ctx.AB.api;
      ctx.AB.api = function (method, url, body) {
        calls.push({ method, url, body });
        if (method === 'GET' && url === '/api/engines') return Promise.resolve(ok(engines));
        if (method === 'GET' && url === '/api/teams') return Promise.resolve(ok(TEAMS));
        if (method === 'GET' && url === '/api/workflows') return Promise.resolve(ok({ workflows: [], errors: [], version: 'w' }));
        if (method === 'GET' && url === '/api/staff') return Promise.resolve(ok(STAFF));
        return Promise.resolve({ ok: false, status: 404, data: null });
      };
      try {
        // ---- Settings: rows ----
        const c = factories.settings();
        await c.load();
        await c.loadEngines();
        const rows = c.engineRows;
        const by = (n) => rows.filter((r) => r.name === n)[0];
        const detailOf = (r) => r.details.map((d) => d.label + d.value);
        check('engine-list: rows follow the API order', rows.map((r) => r.name).join(',') === 'claude,deepseek,gw-thu,gw-chua,claude-max,agy,dsh,người', rows.map((r) => r.name).join(','));
        check('engine-list: kind chips', rows.map((r) => r.chip).join(',') === 'có sẵn,anthropic,anthropic,anthropic,anthropic,agy,dsh,có sẵn', rows.map((r) => r.chip).join(','));
        check('engine-list: gw-thu shows label and the E-09 evidence', by('gw-thu').hasLabel === true && by('gw-thu').label.indexOf('Cổng thử nghiệm') === 0 && detailOf(by('gw-thu')).indexOf('Bằng chứng E-09: 2026-10-28 E-09') >= 0, JSON.stringify(detailOf(by('gw-thu'))));
        check('engine-list: a row without label has no label line but a description', by('claude').hasLabel === false && by('claude').description !== '');
        check('engine-list: gw-chua shows the server reason', by('gw-chua').hasReason === true && by('gw-chua').reason === E.gwChua.reason && by('gw-chua').word === 'chưa kiểm', JSON.stringify(by('gw-chua')));
        check('engine-list: the ready rows have no reason', by('gw-thu').hasReason === false && by('claude').hasReason === false);
        check('engine-list: dsh shows container, no-token line and the container reason', JSON.stringify(detailOf(by('dsh'))) === JSON.stringify(['Lệnh: docker', 'Chỉ cho dự án profile: personal', 'Container: fake-dsh', 'Không đếm token (chỉ văn bản).']) && by('dsh').reason === 'Container fake-dsh chưa chạy.' && by('dsh').key === 'thieu_lenh', JSON.stringify(detailOf(by('dsh'))));
        check('engine-list: claude-max shows the pinned profile once', detailOf(by('claude-max')).indexOf('Profile cố định: personal') >= 0 && detailOf(by('claude-max')).every((l) => l.indexOf('Chỉ cho dự án profile') === -1), JSON.stringify(detailOf(by('claude-max'))));
        check('engine-list: agy shows its allowed profiles', detailOf(by('agy')).indexOf('Chỉ cho dự án profile: work, personal') >= 0, JSON.stringify(detailOf(by('agy'))));
        check('engine-list: anthropic descriptions', by('gw-thu').description === 'claude chạy trên máy với một endpoint định dạng Anthropic.' && by('claude-max').description === 'claude chạy trên máy bằng tài khoản Claude của profile dự án.');
        check('engine-list: dsh description', by('dsh').description === 'DeepSeek Harness trong container, chạy như hộp đen: chỉ văn bản, không đếm token.');
        check('engine-list: người shows the staff count only', JSON.stringify(detailOf(by('người'))) === JSON.stringify(['1 nhân sự']));
        check('engine-list: only the variable NAME is shown for a key', detailOf(by('gw-thu')).indexOf('Biến key: GW_API_KEY') >= 0);
        const everything = JSON.stringify({ rows, teams: c.teamRows, dev: c.roleOptions('dev'), lead: c.roleOptions('lead') });
        check('engine-list: a planted key value or endpoint never reaches a rendered view', everything.indexOf(PLANTED) === -1 && everything.indexOf('gw.invalid') === -1, 'planted value found');

        // ---- Settings: never empty, no hard-coded names ----
        engines = { engines: [] };
        const empty = factories.settings();
        await empty.load();
        await empty.loadEngines();
        check('engine-list: with no payload claude and người still have rows', empty.engineRows.map((r) => r.name).join(',') === 'claude,người' && empty.engineRows.every((r) => r.description !== ''));
        engines = ENGINES;

        // ---- Settings: team editor options ----
        const labels = (role) => c.roleOptions(role).map((o) => o.label);
        const values = (role) => c.roleOptions(role).map((o) => o.value);
        check('engine-list: lead options, no dsh, no people', values('lead').join(',') === 'claude,deepseek,gw-thu,gw-chua,claude-max,agy', values('lead').join(','));
        check('engine-list: dev options add dsh and the people', values('dev').join(',') === 'claude,deepseek,gw-thu,gw-chua,claude-max,agy,dsh,người:ha', values('dev').join(','));
        check('engine-list: qa offers the same list as lead', values('qa').join(',') === values('lead').join(','));
        check('engine-list: a not-ready option reads {id} — {word}', labels('lead').indexOf('gw-chua — chưa kiểm') >= 0 && labels('lead').indexOf('deepseek — thiếu key') >= 0 && labels('dev').indexOf('dsh — thiếu lệnh') >= 0 && labels('lead').indexOf('gw-thu') >= 0, JSON.stringify(labels('dev')));
        c.tDev = 'khong-co';
        check('engine-list: a hand-edited value stays visible and selected', values('dev').indexOf('khong-co') >= 0 && c.optionSelected('dev', 'khong-co') === true);
        c.tDev = 'claude';
        c.tQa = 'gw-thu';
        check('engine-list: QA keeps its inline error for a non-claude choice', c.fieldMsg('team', 'qa') === 'QA phải dùng engine claude.' || c.saveReason('team') === 'QA phải dùng engine claude.', c.fieldMsg('team', 'qa') + ' / ' + c.saveReason('team'));
        c.tQa = 'claude';

        // ---- Settings: team row warnings ----
        const row = (n) => c.teamRows.filter((r) => r.name === n)[0];
        check('engine-list: the server warning is a team row line (D-43)', row('tiet-kiem').warnings.indexOf("Engine 'deepseek' chưa khai báo trong config.json.") >= 0, JSON.stringify(row('tiet-kiem').warnings));
        check('engine-list: chưa kiểm is also a readiness line on a team row', row('chi-chua').warnings.indexOf('gw-chua: chưa kiểm') >= 0, JSON.stringify(row('chi-chua').warnings));

        // ---- Giao việc ----
        const a = factories.assign();
        await a.loadTeams();
        check('engine-list: assign keeps the API order of engines', a.engineOrder.join(',') === 'claude,deepseek,gw-thu,gw-chua,claude-max,agy,dsh,người', a.engineOrder.join(','));
        const t = (n) => TEAMS.teams.filter((x) => x.name === n)[0];
        check('engine-list: soft-only team gets the soft callout, never "sẽ dừng với lỗi"', (() => { a.pickTeam('chi-chua'); return a.teamWarningText === 'Đội hình chi-chua dùng endpoint chưa có bằng chứng E-09: gw-chua. Task vẫn chạy; log của bước đó ghi "chưa kiểm E-09".' && a.teamWarningText.indexOf('sẽ dừng với lỗi') === -1; })(), a.teamWarningText);
        check('engine-list: a mix lists only the hard engines, then the soft sentence', (() => { a.pickTeam('hon-hop'); return a.teamWarningText === 'Đội hình hon-hop dùng engine chưa sẵn sàng: deepseek (thiếu key). Task vẫn giao được, nhưng bước dùng engine này sẽ dừng với lỗi. Endpoint chưa có bằng chứng E-09: gw-chua. Task vẫn chạy; log của bước đó ghi "chưa kiểm E-09".'; })(), a.teamWarningText);
        check('engine-list: readiness lines follow the API order', JSON.stringify(a.teamReadinessLines(t('hon-hop'))) === JSON.stringify(['! deepseek: thiếu key', '! gw-chua: chưa kiểm']) && JSON.stringify(a.teamReadinessLines(t('ca-ba'))) === JSON.stringify(['! gw-chua: chưa kiểm', '! dsh: thiếu lệnh']), JSON.stringify(a.teamReadinessLines(t('ca-ba'))));
        check('engine-list: the hard text of a three-engine team lists only the hard engine', (() => { a.pickTeam('ca-ba'); return a.teamWarningText.indexOf('dsh (thiếu lệnh)') >= 0 && a.teamWarningText.indexOf('gw-chua (') === -1 && a.teamWarningText.indexOf('Endpoint chưa có bằng chứng E-09: gw-chua.') >= 0; })(), a.teamWarningText);
        check('engine-list: no key value reaches the assign state', JSON.stringify({ o: a.engineOrder, r: a.engineReadiness, s: a.engineStates }).indexOf(PLANTED) === -1);

        // ---- Thống kê ----
        const st = factories.stats();
        st.loaded = true;
        st.report = {
          total: { runs: 6, input_tokens: 1000, output_tokens: 100, cache_read_tokens: 0, cache_write_tokens: 0, cost_usd: 0 },
          by_day: [], by_profile: [], by_project: [], by_role: [],
          engines: [
            { engine: 'claude', runs: 2, input: 1000, output: 100, cache_read: 0, cache_write: 0, cost: 0.5, priced: true, usage: 'tokens' },
            { engine: 'gw-chua', runs: 2, input: 500, output: 0, cache_read: 0, cache_write: 0, cost: 0, priced: false, usage: 'tokens' },
            { engine: 'dsh', runs: 2, input: 0, output: 0, cache_read: 0, cache_write: 0, cost: 0, priced: false, usage: 'none' },
          ],
          qa_by_dev_engine: [],
        };
        const srows = st.engineRows;
        check('engine-list: a usage none engine reads không đếm token · {n} run', srows[2].line2 === 'không đếm token · 2 run', srows[2].line2);
        check('engine-list: a token engine without a price still reads chưa có giá', srows[1].line2 === 'chưa có giá · 2 run' && srows[0].line2 === '≈ $0.5000 · 2 run', srows[1].line2 + ' / ' + srows[0].line2);
        check('engine-list: no engine route is written', calls.every((x) => x.url.indexOf('/api/engines') !== 0 || x.method === 'GET'));
      } finally {
        ctx.AB.api = savedApi;
      }
    },
  },
  {
    // 06.2-02: the read-only Telegram screen (GET /api/telegram).
    name: 'telegram',
    async run(ctx) {
      const factories = {};
      ctx.Alpine = {
        data(name, f) {
          factories[name] = f;
        },
      };
      const dash = (f) => path.join(repoRoot, 'agent-brain/assets/dashboard', f);
      vm.runInContext(fs.readFileSync(dash('js/telegram.js'), 'utf8'), ctx, { filename: 'telegram.js' });
      check('telegram: factory registered', typeof factories.telegram === 'function');
      if (typeof factories.telegram !== 'function') return;

      const PLANTED = '4242:planted-telegram-token-123456';
      const base = { message: null, bot_username: null, chat_id: null, paired_at: null, last_ok_at: null, outbox_pending: 0, tasks_today: 0, tasks_per_day_limit: 10, config_path: '/home/x/.config/agent-brain/telegram.json' };
      const payload = (over) => Object.assign({ state: 'off' }, base, over, { token: PLANTED, url: 'https://api.telegram.org/bot' + PLANTED + '/getMe' });
      const savedApi = ctx.AB.api;
      let current = payload({});
      let reply = () => Promise.resolve({ ok: true, status: 200, data: JSON.parse(JSON.stringify(current)) });
      const calls = [];
      ctx.AB.api = function (method, url) {
        calls.push({ method, url });
        return reply();
      };
      const make = async (over) => {
        current = payload(over);
        const c = factories.telegram();
        await c.load();
        return c;
      };
      try {
        // ---- Badge words and colours per state ----
        const words = { off: 'Chưa cấu hình', error: 'Lỗi cấu hình', starting: 'Đang khởi động', running: 'Đang chạy', degraded: 'Mất kết nối' };
        for (const state of Object.keys(words)) {
          const c = await make({ state, message: state === 'error' || state === 'degraded' ? 'Lý do ' + state : null });
          check('telegram: badge word for ' + state, c.stateWord === words[state], c.stateWord);
          check('telegram: badge for ' + state + ' shares the readiness colours and glyph', typeof c.stateBadgeClass === 'string' && c.stateBadgeClass.indexOf('rounded-full') >= 0 && c.stateGlyph.length > 0, c.stateBadgeClass);
        }
        const running = await make({ state: 'running' });
        const errored = await make({ state: 'error', message: 'x' });
        const degraded = await make({ state: 'degraded', message: 'x' });
        check('telegram: running is green, error is red, degraded is amber', running.stateBadgeClass.indexOf('emerald') >= 0 && errored.stateBadgeClass.indexOf('rose') >= 0 && degraded.stateBadgeClass.indexOf('amber') >= 0);
        check('telegram: a state the server does not know reads like off', (await make({ state: 'zzz' })).stateWord === 'Chưa cấu hình');

        // ---- Rows are hidden when there is no data ----
        const labelsOf = (c) => c.rows.map((r) => r.label);
        const off = await make({ state: 'off' });
        check('telegram: off shows only the config file row', JSON.stringify(labelsOf(off)) === JSON.stringify(['Tệp cấu hình']), JSON.stringify(labelsOf(off)));
        check('telegram: off shows the setup text and the command', off.isOff === true && off.offText === 'Bot Telegram chưa bật. Chạy "agent-brain telegram setup" trong terminal để ghép cặp bot, rồi khởi động lại agent-brain serve.' && off.setupCommand === 'agent-brain telegram setup', off.offText);
        check('telegram: off has no alert message', off.hasMessage === false);
        const starting = await make({ state: 'starting', chat_id: 5555, paired_at: 1700000000000 });
        check('telegram: starting hides Bot and last contact until they exist', JSON.stringify(labelsOf(starting)) === JSON.stringify(['Chat ID', 'Đã ghép cặp lúc', 'Tin chờ gửi', 'Task giao qua Telegram hôm nay', 'Tệp cấu hình']), JSON.stringify(labelsOf(starting)));
        check('telegram: starting is not the off view', starting.isOff === false);
        const full = await make({ state: 'running', bot_username: 'ab_bot', chat_id: 5555, paired_at: 1700000000000, last_ok_at: 1700000100000, outbox_pending: 3, tasks_today: 2, tasks_per_day_limit: 10 });
        const row = (c, id) => c.rows.filter((r) => r.id === id)[0];
        check('telegram: running shows every row', labelsOf(full).join('|') === 'Bot|Chat ID|Đã ghép cặp lúc|Lần liên lạc thành công gần nhất|Tin chờ gửi|Task giao qua Telegram hôm nay|Tệp cấu hình', labelsOf(full).join('|'));
        check('telegram: Bot row reads @username', row(full, 'bot').value === '@ab_bot');
        check('telegram: Chat ID, outbox and config rows', row(full, 'chat').value === '5555' && row(full, 'outbox').value === '3' && row(full, 'config').value === '/home/x/.config/agent-brain/telegram.json');
        check('telegram: tasks today reads {n}/{limit}', row(full, 'tasks').value === '2/10', row(full, 'tasks').value);
        const zero = await make({ state: 'running', bot_username: 'ab_bot', tasks_today: 0, tasks_per_day_limit: 0 });
        check('telegram: limit 0 means giao việc is off, not unlimited', row(zero, 'tasks').value.indexOf('0/0') === 0 && row(zero, 'tasks').value.indexOf('đang tắt') >= 0 && row(zero, 'tasks').value.indexOf('không giới hạn') === -1, row(zero, 'tasks').value);
        const noCount = await make({ state: 'running', bot_username: 'ab_bot', tasks_today: null });
        check('telegram: no task row when the count is unknown', row(noCount, 'tasks') === undefined);

        // ---- Message verbatim for error and degraded ----
        const msg = '/x/telegram.json có quyền đọc cho nhóm/người khác — chạy: chmod 600 /x/telegram.json';
        const e = await make({ state: 'error', message: msg });
        check('telegram: error shows the server message verbatim', e.hasMessage === true && e.messageText === msg && e.messageClass.indexOf('rose') >= 0, e.messageText);
        const d = await make({ state: 'degraded', message: 'Không tới được Telegram (x)', bot_username: 'ab_bot' });
        check('telegram: degraded shows the server message in the amber alert', d.hasMessage === true && d.messageText === 'Không tới được Telegram (x)' && d.messageClass.indexOf('amber') >= 0);
        check('telegram: running never shows a message', (await make({ state: 'running', message: 'cũ', bot_username: 'a' })).hasMessage === false);

        // ---- The token never reaches a rendered view ----
        const shown = JSON.stringify({ full: full.rows, e: e.rows, d: d.rows, off: off.rows });
        check('telegram: a token in the payload is never rendered', shown.indexOf(PLANTED) === -1 && shown.indexOf('planted-telegram-token') === -1, 'planted token found');

        // ---- Polling: GET /api/telegram every 5 s, timer cleared on leave ----
        const before = ctx.AB._timers.length;
        calls.length = 0;
        const polled = factories.telegram();
        polled.init();
        check('telegram: init loads GET /api/telegram once', calls.length === 1 && calls[0].method === 'GET' && calls[0].url === '/api/telegram', JSON.stringify(calls));
        check('telegram: a poll timer is registered with the router', ctx.AB._timers.length === before + 1);
        ctx.AB._timers.splice(before).forEach((t) => ctx.clearInterval(t));
        check('telegram: only GET calls were made', calls.every((x) => x.method === 'GET'));

        // ---- Failures reuse the existing behaviour ----
        reply = () => Promise.resolve({ ok: false, status: 0, data: null });
        const down = factories.telegram();
        await down.load();
        check('telegram: first failure shows the error with a retry', down.hasError === true && down.errorBody.indexOf('Mất kết nối tới agent-brain serve') === 0);
        reply = () => Promise.resolve({ ok: true, status: 200, data: JSON.parse(JSON.stringify(payload({ state: 'running', bot_username: 'a' }))) });
        await down.load();
        check('telegram: it recovers on the next poll', down.hasError === false && down.stateWord === 'Đang chạy');
        reply = () => Promise.resolve({ ok: false, status: 0, data: null });
        await down.load();
        check('telegram: a later failure keeps the data and marks it stale', down.stale === true && down.hasStatus === true && down.stateWord === 'Đang chạy');
      } finally {
        ctx.AB.api = savedApi;
      }

      // ---- Markup: link, route, title, registered assets, no way to enter or change a token ----
      const html = fs.readFileSync(dash('screens/telegram.html'), 'utf8');
      const index = fs.readFileSync(dash('index.html'), 'utf8');
      const appJs = fs.readFileSync(dash('js/app.js'), 'utf8');
      check('telegram: the screen has no input, textarea, select or form', !/<(input|textarea|select|form)\b/i.test(html));
      check('telegram: the screen has no token field or x-model', !/token/i.test(html.replace(/token không bao giờ hiện ở đây/i, '')) && html.indexOf('x-model') === -1 && html.indexOf('x-html') === -1 && !/\sstyle=/.test(html));
      check('telegram: the only button is the retry button', (html.match(/<button\b/g) || []).length === 1 && html.indexOf('@click="reload"') >= 0);
      check('telegram: nav link with navClass and navCurrent', index.indexOf('href="#/telegram"') >= 0 && index.indexOf("navClass('telegram')") >= 0 && index.indexOf("navCurrent('telegram')") >= 0 && />Telegram<\/a>/.test(index));
      check('telegram: app.js has the route and the title', appJs.indexOf("'#/telegram'") >= 0 && appJs.indexOf("telegram: 'Telegram'") >= 0);
    },
  },
];

// A promise that never settles would let node exit 0 with no output; fail loudly instead.
let finished = false;
process.on('exit', (code) => {
  if (!finished && code === 0) {
    console.error('LỖI JS check dừng sớm: có promise không bao giờ hoàn thành');
    process.exitCode = 1;
  }
});

async function main() {
  const ctx = makeContext();
  for (const section of sections) {
    await section.run(ctx);
  }

  finished = true;
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

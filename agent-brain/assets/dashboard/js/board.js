// js/board.js — Bảng công việc (#/board), read-only
(function () {
  var COL_W = 216;
  var RAIL_W = 96;
  var GAP_X = 32;
  var HEAD_H = 48;
  var ROW_GAP = 8;
  var CARD_H = 64;
  var CHIP_H = 32;

  // Full literal class strings: Tailwind finds them by scanning this file.
  var BOARD_CARD = {
    xong: 'border-emerald-300 bg-emerald-50 border-l-4 border-l-emerald-600 dark:border-emerald-700 dark:bg-emerald-950 dark:border-l-emerald-400',
    san_sang: 'border-slate-300 bg-white border-l-4 border-l-emerald-600 dark:border-slate-600 dark:bg-slate-800 dark:border-l-emerald-400',
    bi_chan: 'border-dashed border-slate-400 bg-white dark:border-slate-500 dark:bg-slate-800',
  };
  var CARD_LAYOUT = 'flex-col justify-center gap-1 p-2';
  var CHIP_LAYOUT = 'flex-row items-center gap-2 px-2 py-1';
  var EDGE_DEFAULT = 'stroke-slate-400 dark:stroke-slate-500 stroke-1';
  var EDGE_CRITICAL = 'stroke-yellow-600 dark:stroke-yellow-400 stroke-2';
  var SWITCH_ON = 'bg-indigo-100 text-indigo-800 ring-indigo-300 dark:bg-indigo-900/40 dark:text-indigo-200 dark:ring-indigo-700';
  var SWITCH_OFF = 'text-slate-900 ring-slate-300 hover:bg-slate-100 dark:text-slate-100 dark:ring-slate-600 dark:hover:bg-slate-700';

  function boardLayout(items, collapse) {
    var list = items || [];
    var order = [];
    var byWave = {};
    var i;
    for (i = 0; i < list.length; i++) {
      var w = list[i].wave;
      if (!Object.prototype.hasOwnProperty.call(byWave, w)) {
        byWave[w] = [];
        order.push(w);
      }
      byWave[w].push(list[i]);
    }
    order.sort(function (a, b) { return a - b; });

    var x = 0;
    var cols = [];
    var pos = {};
    var height = 0;
    for (i = 0; i < order.length; i++) {
      var wave = order[i];
      var group = byWave[wave];
      var done = 0;
      var j;
      for (j = 0; j < group.length; j++) {
        if (group[j].status === 'xong') done += 1;
      }
      var allDone = done === group.length;
      var rail = Boolean(collapse) && allDone;
      var colW = rail ? RAIL_W : COL_W;
      var y = HEAD_H + ROW_GAP;
      for (j = 0; j < group.length; j++) {
        var chip = rail || (Boolean(collapse) && group[j].status === 'xong');
        var h = chip ? CHIP_H : CARD_H;
        pos[group[j].key] = { x: x, y: y, w: colW, h: h, chip: chip, rail: rail };
        if (y + h > height) height = y + h;
        y += h + ROW_GAP;
      }
      cols.push({ wave: wave, x: x, w: colW, rail: rail, done: done, total: group.length });
      x += colW + GAP_X;
    }
    return { cols: cols, pos: pos, width: Math.max(x - GAP_X, 0), height: height };
  }

  function edgeGeometry(a, b) {
    var x1 = a.x + a.w;
    var y1 = a.y + a.h / 2;
    var x2 = b.x;
    var y2 = b.y + b.h / 2;
    var top = Math.min(y1, y2) - 2;
    var width = x2 - x1;
    var height = Math.abs(y2 - y1) + 4;
    var ly1 = y1 - top;
    var ly2 = y2 - top;
    var mx = width / 2;
    var d = 'M0 ' + ly1 + ' C' + mx + ' ' + ly1 + ' ' + mx + ' ' + ly2 + ' ' + width + ' ' + ly2;
    return {
      boxStyle: { left: x1 + 'px', top: top + 'px', width: width + 'px', height: height + 'px' },
      d: d,
    };
  }

  function boardDomId(key) {
    return 'board-item-' + String(key).replace(/[^A-Za-z0-9_-]/g, '_');
  }

  window.ABBoard = {
    COL_W: COL_W,
    RAIL_W: RAIL_W,
    GAP_X: GAP_X,
    HEAD_H: HEAD_H,
    ROW_GAP: ROW_GAP,
    CARD_H: CARD_H,
    CHIP_H: CHIP_H,
    layout: boardLayout,
    edgeGeometry: edgeGeometry,
    domId: boardDomId,
  };

  function arr(v) {
    return Array.isArray(v) ? v : [];
  }

  function obj(v) {
    return (v && typeof v === 'object' && !Array.isArray(v)) ? v : {};
  }

  function readStore(key) {
    try {
      return localStorage.getItem(key);
    } catch (e) {
      return null;
    }
  }

  function writeStore(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch (e) {}
  }

  function removeStore(key) {
    try {
      localStorage.removeItem(key);
    } catch (e) {}
  }

  window.AB.onReady('board', function () {
    return {
      loaded: false,
      loadError: '',
      reloading: false,
      projectRetried: false,
      reqSeq: 0,
      state: '',
      reason: '',
      message: '',
      expectedPath: '',
      project: null,
      projects: [],
      projectSel: readStore('ab.board.project') || '',
      epic: '',
      defaultEpic: '',
      epics: [],
      epicSel: '',
      milestone: '',
      milestones: [],
      milestoneSel: '',
      items: [],
      edges: [],
      external: [],
      counts: {},
      warnings: [],
      truncated: false,
      collapse: readStore('ab.board.collapse') !== '0',
      cols: [],
      cards: [],
      edgeViews: [],
      canvasStyle: {},

      init: function () {
        var self = this;
        this.load();
        var profileHandler = function () {
          self.load();
        };
        window.addEventListener('ab:profile', profileHandler);
        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
        });
      },

      buildUrl: function () {
        var parts = [];
        if (this.projectSel) parts.push('project=' + encodeURIComponent(this.projectSel));
        if (this.epicSel) parts.push('epic=' + encodeURIComponent(this.epicSel));
        if (this.milestoneSel) parts.push('milestone=' + encodeURIComponent(this.milestoneSel));
        var q = window.AB.profileQuery();
        if (q) parts.push(q);
        return '/api/board' + (parts.length > 0 ? '?' + parts.join('&') : '');
      },

      load: function () {
        var self = this;
        this.reqSeq += 1;
        var seq = this.reqSeq;
        return window.AB.api('GET', this.buildUrl()).then(function (res) {
          if (seq !== self.reqSeq) return;
          if (res && res.status === 404 && self.projectSel && !self.projectRetried) {
            // The remembered project no longer exists: forget it and ask for the default once.
            self.projectRetried = true;
            removeStore('ab.board.project');
            self.projectSel = '';
            self.epicSel = '';
            self.milestoneSel = '';
            return self.load();
          }
          if (res && res.ok && res.data) {
            self.apply(res.data);
          } else {
            self.loadError = window.AB.errorText(res);
          }
          self.loaded = true;
        });
      },

      apply: function (d) {
        this.state = d.state || '';
        this.reason = d.reason || '';
        this.message = d.message || '';
        this.expectedPath = d.expected_path || '';
        this.project = d.project || null;
        this.projects = arr(d.projects);
        this.epic = d.epic || '';
        this.defaultEpic = d.default_epic || '';
        this.epics = arr(d.epics);
        this.milestone = d.milestone || '';
        this.milestones = arr(d.milestones);
        this.items = arr(d.items);
        this.edges = arr(d.edges);
        this.external = arr(d.external);
        this.counts = obj(d.counts);
        this.warnings = arr(d.warnings);
        this.truncated = Boolean(d.truncated);
        this.projectSel = d.project && d.project.id ? d.project.id : '';
        this.epicSel = d.epic || '';
        this.milestoneSel = d.milestone || '';
        this.loadError = '';
        this.rebuild();
      },

      rebuild: function () {
        var AB = window.AB;
        var lay = window.ABBoard.layout(this.items, this.collapse);
        var cards = [];
        var i;
        for (i = 0; i < this.items.length; i++) {
          var it = this.items[i];
          var p = lay.pos[it.key];
          if (!p) continue;
          cards.push({
            key: it.key,
            title: it.title,
            status: it.status,
            critical: Boolean(it.critical),
            placeholder: Boolean(it.placeholder),
            wave: it.wave,
            epic: it.epic,
            epic_name: it.epic_name,
            milestone: it.milestone,
            requirements: arr(it.requirements),
            plan_path: it.plan_path,
            waits_on: arr(it.waits_on),
            waited_by: arr(it.waited_by),
            fm_wave: it.fm_wave,
            domId: window.ABBoard.domId(it.key),
            chip: p.chip,
            rail: p.rail,
            style: { left: p.x + 'px', top: p.y + 'px', width: p.w + 'px', height: p.h + 'px' },
            glyph: AB.stepGlyph(it.status),
            statusText: AB.stepGlyph(it.status) + ' ' + AB.stateWord(it.status),
            textClass: AB.stateText(it.status),
          });
        }

        var edgeViews = [];
        for (i = 0; i < this.edges.length; i++) {
          var e = this.edges[i];
          var a = lay.pos[e.from];
          var b = lay.pos[e.to];
          if (!a || !b) continue;
          var geo = window.ABBoard.edgeGeometry(a, b);
          edgeViews.push({
            id: e.from + '__' + e.to,
            from: e.from,
            to: e.to,
            critical: Boolean(e.critical),
            kind: e.kind,
            zClass: e.critical ? 'z-10' : 'z-0',
            boxStyle: geo.boxStyle,
            d: geo.d,
          });
        }

        var cols = [];
        for (i = 0; i < lay.cols.length; i++) {
          var c = lay.cols[i];
          cols.push({
            wave: c.wave,
            x: c.x,
            w: c.w,
            rail: c.rail,
            done: c.done,
            total: c.total,
            headStyle: { left: c.x + 'px', width: c.w + 'px' },
            title: 'Wave ' + c.wave,
            sub: c.done + '/' + c.total + (c.rail ? ' ✓︎' : ''),
            label: 'Wave ' + c.wave + ' · ' + c.done + '/' + c.total + ' xong',
            waveAttr: String(c.wave),
            railAttr: c.rail ? '1' : '0',
          });
        }

        this.cards = cards;
        this.edgeViews = edgeViews;
        this.cols = cols;
        this.canvasStyle = {
          width: Math.max(lay.width, 1) + 'px',
          height: Math.max(lay.height, 1) + 'px',
        };
      },

      reload: function () {
        var self = this;
        if (this.reloading) return Promise.resolve();
        this.reloading = true;
        return this.load().then(function () {
          self.reloading = false;
        });
      },

      toggleCollapse: function () {
        this.collapse = !this.collapse;
        writeStore('ab.board.collapse', this.collapse ? '1' : '0');
        this.rebuild();
      },

      projectChanged: function () {
        writeStore('ab.board.project', this.projectSel);
        this.epicSel = '';
        this.milestoneSel = '';
        return this.load();
      },

      epicChanged: function () {
        return this.load();
      },

      milestoneChanged: function () {
        return this.load();
      },

      projectLabel: function (p) {
        return p.name_vi || p.id;
      },

      epicLabel: function (e) {
        return e.id + ' · ' + e.name + ' (' + e.done + '/' + e.total + ')';
      },

      isProjectSel: function (p) {
        return this.projectSel === p.id;
      },

      isEpicSel: function (id) {
        return this.epicSel === id;
      },

      isMilestoneSel: function (m) {
        return this.milestoneSel === m;
      },

      cardClass: function (it) {
        var status = BOARD_CARD[it.status] || BOARD_CARD.bi_chan;
        return status + ' ' + (it.chip ? CHIP_LAYOUT : CARD_LAYOUT);
      },

      edgeClass: function (e) {
        return e.critical ? EDGE_CRITICAL : EDGE_DEFAULT;
      },

      cardLabel: function (it) {
        return it.key + ' ' + it.title + ', ' + window.AB.stateWord(it.status) +
          ', wave ' + it.wave + (it.critical ? ', trên đường găng' : '');
      },

      get loadedAttr() {
        return this.loaded ? 'yes' : 'no';
      },

      get isLoading() {
        return !this.loaded;
      },

      get hasError() {
        return this.loaded && this.loadError !== '';
      },

      get errorBody() {
        return this.loadError + '. Bấm Thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get isEmpty() {
        return this.state === 'empty';
      },

      get hasBoard() {
        return this.state === 'ok';
      },

      get noItems() {
        return this.state === 'ok' && this.items.length === 0;
      },

      get legend() {
        var AB = window.AB;
        var ids = ['xong', 'san_sang', 'bi_chan'];
        var out = [];
        for (var i = 0; i < ids.length; i++) {
          var n = this.counts[ids[i]] || 0;
          out.push({
            id: ids[i],
            text: AB.stepGlyph(ids[i]) + ' ' + AB.stateWord(ids[i]) + ' ' + n,
            cls: AB.stateText(ids[i]),
          });
        }
        return out;
      },

      get reloadLabel() {
        return this.reloading ? 'Đang đọc...' : 'Đọc lại';
      },

      get collapseAttr() {
        return this.collapse ? 'true' : 'false';
      },

      get collapseWord() {
        return this.collapse ? 'bật' : 'tắt';
      },

      get collapseClass() {
        return this.collapse ? SWITCH_ON : SWITCH_OFF;
      },

      get showMilestones() {
        return this.milestones.length >= 2;
      },

      get boardProjects() {
        var out = [];
        var curId = this.project && this.project.id ? this.project.id : '';
        var seen = false;
        for (var i = 0; i < this.projects.length; i++) {
          var p = this.projects[i];
          if (p.has_board || p.id === curId) out.push(p);
          if (p.id === curId) seen = true;
        }
        if (curId && !seen) {
          out.push({ id: curId, name_vi: this.project.name_vi, profile: this.project.profile, has_board: false });
        }
        return out;
      },

      get hasProjects() {
        return this.boardProjects.length > 0;
      },

      get hasWarnings() {
        return this.warnings.length > 0;
      },

      get warningLines() {
        var lines = [];
        var n = this.warnings.length;
        var shown = Math.min(n, 10);
        for (var i = 0; i < shown; i++) {
          lines.push(this.warnings[i].path + ' — ' + this.warnings[i].message);
        }
        if (n > 10) lines.push('… và ' + (n - 10) + ' cảnh báo khác');
        return lines;
      },

      get legendTotal() {
        return 'Tổng ' + (this.counts.total || 0) + ' · ' + (this.counts.waves || 0) + ' wave';
      },
    };
  });
})();

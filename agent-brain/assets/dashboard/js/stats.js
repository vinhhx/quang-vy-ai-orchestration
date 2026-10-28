// js/stats.js — Thống kê screen component
(function () {
  window.AB.onReady('stats', function () {
    return {
      since: '30d',
      report: null,
      loaded: false,
      error: '',
      stale: false,
      profile: window.AB.profile(),
      showAllProjects: false,

      init: function () {
        var self = this;
        this.load();

        var profileHandler = function (e) {
          self.profile = (e.detail && e.detail.profile) ? e.detail.profile : window.AB.profile();
          self.load();
        };
        window.addEventListener('ab:profile', profileHandler);
        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
        });
      },

      sinceChanged: function () {
        this.load();
      },

      load: function () {
        var self = this;
        var parts = [];
        var pq = window.AB.profileQuery();
        if (pq) {
          parts.push(pq);
        }
        if (self.since) {
          parts.push('since=' + encodeURIComponent(self.since));
        }
        var url = '/api/stats' + (parts.length > 0 ? '?' + parts.join('&') : '');

        window.AB.api('GET', url).then(function (res) {
          if (res.ok && res.data) {
            self.report = res.data;
            self.loaded = true;
            self.stale = false;
            self.error = '';
          } else {
            if (!self.loaded) {
              self.error = window.AB.errorText(res);
            } else {
              self.stale = true;
            }
          }
        });
      },

      reload: function () {
        this.error = '';
        this.load();
      },

      get loadedAttr() {
        return this.loaded ? 'yes' : 'no';
      },

      get isLoading() {
        return !this.loaded && !this.error;
      },

      get hasError() {
        return Boolean(this.error);
      },

      get errorBody() {
        return this.error + '. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get isEmpty() {
        return this.loaded && !this.error && Boolean(this.report && this.report.total && this.report.total.runs === 0);
      },

      get hasData() {
        return this.loaded && !this.error && Boolean(this.report && this.report.total && this.report.total.runs > 0);
      },

      get filterChip() {
        return window.AB.filterText();
      },

      get filterClass() {
        return window.AB.profileClass(this.profile);
      },

      get totalRuns() {
        return (this.report && this.report.total) ? window.AB.fmtNum(this.report.total.runs) : '0';
      },

      get totalIn() {
        return (this.report && this.report.total) ? window.AB.fmtNum(this.report.total.input_tokens) : '0';
      },

      get totalOut() {
        return (this.report && this.report.total) ? window.AB.fmtNum(this.report.total.output_tokens) : '0';
      },

      get totalCache() {
        if (!this.report || !this.report.total || !window.AB.tokensKnown(this.report.total)) {
          return '—';
        }
        return window.AB.fmtNum(this.report.total.cache_read_tokens) + ' / ' + window.AB.fmtNum(this.report.total.cache_write_tokens);
      },

      get totalCost() {
        if (!this.report || !this.report.total || !window.AB.costKnown(this.report.total)) {
          return '—';
        }
        return window.AB.fmtCost(this.report.total.cost_usd);
      },

      get days() {
        return (this.report && this.report.by_day) ? this.report.by_day : [];
      },

      get maxDay() {
        var max = 0;
        var d = this.days;
        for (var i = 0; i < d.length; i++) {
          var val = this.io(d[i]);
          if (val > max) max = val;
        }
        return Math.max(1, max);
      },

      get peakText() {
        var max = 0;
        var d = this.days;
        for (var i = 0; i < d.length; i++) {
          var val = this.io(d[i]);
          if (val > max) max = val;
        }
        return 'Cao nhất: ' + window.AB.fmtNum(max) + ' token / ngày';
      },

      get profiles() {
        var list = (this.report && this.report.by_profile) ? this.report.by_profile.slice() : [];
        var self = this;
        list.sort(function (a, b) { return self.io(b) - self.io(a); });
        return list;
      },

      get roles() {
        var list = (this.report && this.report.by_role) ? this.report.by_role.slice() : [];
        var self = this;
        list.sort(function (a, b) { return self.io(b) - self.io(a); });
        return list;
      },

      get projects() {
        var list = (this.report && this.report.by_project) ? this.report.by_project.slice() : [];
        var self = this;
        list.sort(function (a, b) { return self.io(b) - self.io(a); });
        if (!this.showAllProjects && list.length > 10) {
          return list.slice(0, 10);
        }
        return list;
      },

      get hasMoreProjects() {
        var list = (this.report && this.report.by_project) ? this.report.by_project : [];
        return !this.showAllProjects && list.length > 10;
      },

      get maxProfile() {
        var max = 0;
        var p = this.profiles;
        for (var i = 0; i < p.length; i++) {
          var val = this.io(p[i]);
          if (val > max) max = val;
        }
        return Math.max(1, max);
      },

      get maxProject() {
        var max = 0;
        var list = (this.report && this.report.by_project) ? this.report.by_project : [];
        for (var i = 0; i < list.length; i++) {
          var val = this.io(list[i]);
          if (val > max) max = val;
        }
        return Math.max(1, max);
      },

      get maxRole() {
        var max = 0;
        var r = this.roles;
        for (var i = 0; i < r.length; i++) {
          var val = this.io(r[i]);
          if (val > max) max = val;
        }
        return Math.max(1, max);
      },

      io: function (row) {
        if (!row) return 0;
        return (row.input_tokens || 0) + (row.output_tokens || 0);
      },

      // ---- Theo engine (ENG-04, D-23) and QA rate by Dev engine (ENG-07) ----
      // Engine rows keep the order the server sends (tokens in + out, then name); the figures
      // use the engine columns (`input`, `output`, `cache_read`, `cache_write`).
      get engineList() {
        return (this.report && Array.isArray(this.report.engines)) ? this.report.engines : [];
      },

      isHumanEngine: function (name) {
        return String(name === null || name === undefined ? '' : name).indexOf('người') === 0;
      },

      engineIo: function (e) {
        if (!e) return 0;
        return (e.input || 0) + (e.output || 0);
      },

      // Share of the largest row's tokens; the person's row has no tokens and no bar.
      get maxEngine() {
        var max = 0;
        var list = this.engineList;
        for (var i = 0; i < list.length; i++) {
          var val = this.engineIo(list[i]);
          if (val > max) max = val;
        }
        return Math.max(1, max);
      },

      barStyle: function (pct) {
        var p = Number(pct);
        if (isNaN(p) || p < 0) p = 0;
        if (p > 100) p = 100;
        return { width: p + '%' };
      },

      engineRow: function (e) {
        var AB = window.AB;
        var name = e.engine || '';
        var runs = e.runs || 0;
        if (this.isHumanEngine(name)) {
          return {
            key: name,
            name: name,
            hasBar: false,
            noBar: true,
            style: this.barStyle(0),
            line1: 'không có token',
            line2: runs + ' lượt do người làm',
            title: name + ' · ' + runs + ' lượt do người làm · không có token',
          };
        }
        var cache = (e.cache_read || e.cache_write)
          ? (AB.fmtNum(e.cache_read) + ' / ' + AB.fmtNum(e.cache_write))
          : '—';
        // An engine without a price never reads `≈ $0`: the cost is unknown, not zero.
        // A black-box engine (dsh, `usage: none`) counts no tokens at all: say so instead.
        var line2 = e.usage === 'none'
          ? 'không đếm token · ' + runs + ' run'
          : (e.priced ? AB.fmtCost(e.cost) : 'chưa có giá') + ' · ' + runs + ' run';
        return {
          key: name,
          name: name,
          hasBar: true,
          noBar: false,
          style: this.barStyle((this.engineIo(e) / this.maxEngine) * 100),
          line1: 'vào ' + AB.fmtNum(e.input) + ' · ra ' + AB.fmtNum(e.output),
          line2: line2,
          title: name + ' · cache ' + cache + (e.priced ? ' · ' + AB.fmtCost(e.cost) : ''),
        };
      },

      get engineRows() {
        var list = this.engineList;
        var out = [];
        for (var i = 0; i < list.length; i++) {
          if (list[i]) out.push(this.engineRow(list[i]));
        }
        return out;
      },

      get hasEngineRows() {
        return this.engineList.length > 0;
      },

      // QA rate: `qa_fix` of `qa_total` verdicts (Đạt or Cần sửa) per Dev engine.
      get rateList() {
        return (this.report && Array.isArray(this.report.qa_by_dev_engine)) ? this.report.qa_by_dev_engine : [];
      },

      get rateRows() {
        var list = this.rateList;
        var out = [];
        for (var i = 0; i < list.length; i++) {
          var q = list[i];
          if (!q) continue;
          var k = q.qa_fix || 0;
          var n = q.qa_total || 0;
          var pct = n > 0 ? (100 * k / n) : 0;
          out.push({
            key: q.engine,
            name: q.engine,
            style: this.barStyle(pct),
            line1: window.AB.fmtPercent(k, n),
            line2: k + ' / ' + n + ' lần QA',
            title: q.engine + ' · ' + k + ' / ' + n + ' lần QA kết luận Cần sửa',
          });
        }
        return out;
      },

      get hasRates() {
        return this.rateList.length > 0;
      },

      get noRates() {
        return !this.hasRates;
      },

      colStyle: function (row) {
        var val = this.io(row);
        if (val <= 0) return { height: '0%' };
        var pct = (val / this.maxDay) * 100;
        if (pct < 2) pct = 2;
        return { height: pct + '%' };
      },

      profileBar: function (r) {
        var val = this.io(r);
        if (val <= 0) return { width: '0%' };
        var pct = (val / this.maxProfile) * 100;
        return { width: pct + '%' };
      },

      projectBar: function (r) {
        var val = this.io(r);
        if (val <= 0) return { width: '0%' };
        var pct = (val / this.maxProject) * 100;
        return { width: pct + '%' };
      },

      roleBar: function (r) {
        var val = this.io(r);
        if (val <= 0) return { width: '0%' };
        var pct = (val / this.maxRole) * 100;
        return { width: pct + '%' };
      },

      dayLabel: function (d) {
        if (!d || !d.day) return '';
        var parts = d.day.split('-');
        if (parts.length === 3) {
          return parts[2] + '/' + parts[1];
        }
        return d.day;
      },

      dayLabelClass: function (i) {
        var base = 'block truncate text-center text-sm font-normal leading-normal text-slate-600 dark:text-slate-400';
        if (this.days.length <= 14 || (i % 5 === 0) || (i === this.days.length - 1)) {
          return base;
        }
        return 'invisible ' + base;
      },

      rowTitle: function (d) {
        var date = this.dayLabel(d);
        var tin = window.AB.fmtNum(d.input_tokens);
        var tout = window.AB.fmtNum(d.output_tokens);
        var cache = window.AB.tokensKnown(d)
          ? (window.AB.fmtNum(d.cache_read_tokens) + '/' + window.AB.fmtNum(d.cache_write_tokens))
          : '—';
        var cost = window.AB.costKnown(d) ? window.AB.fmtCost(d.cost_usd) : '—';
        return date + ' · vào ' + tin + ' · ra ' + tout + ' · cache ' + cache + ' · ' + cost;
      },

      barTitle: function (name, r) {
        var cache = window.AB.tokensKnown(r)
          ? (window.AB.fmtNum(r.cache_read_tokens) + '/' + window.AB.fmtNum(r.cache_write_tokens))
          : '—';
        var cost = window.AB.costKnown(r) ? window.AB.fmtCost(r.cost_usd) : '—';
        return name + ' · cache ' + cache + ' · ' + cost;
      },

      rowNumbers: function (r) {
        return 'vào ' + window.AB.fmtNum(r.input_tokens) + ' · ra ' + window.AB.fmtNum(r.output_tokens);
      },

      profileName: function (r) {
        return (r && r.profile) ? r.profile : '';
      },

      projectName: function (r) {
        return (r && r.project) ? r.project : '';
      },

      roleLabel: function (r) {
        return (r && r.role) ? window.AB.roleName(r.role) : '';
      },

      showAll: function () {
        this.showAllProjects = true;
      },
    };
  });
})();

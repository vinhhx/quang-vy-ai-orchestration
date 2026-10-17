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

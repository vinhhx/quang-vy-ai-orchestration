// js/overview.js — Tổng quan screen component
(function () {
  window.AB.onReady('overview', function () {
    return {
      tasks: [],
      busy: [],
      loaded: false,
      error: '',
      stale: false,
      doneLimit: 10,
      profile: window.AB.profile(),

      init: function () {
        var self = this;
        this.load();

        window.AB.every(4000, function () {
          self.load();
        });

        var profileHandler = function (e) {
          self.profile = (e.detail && e.detail.profile) ? e.detail.profile : window.AB.profile();
          self.doneLimit = 10;
          self.load();
        };
        window.addEventListener('ab:profile', profileHandler);
        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
        });
      },

      load: function () {
        var self = this;
        var url = '/api/tasks?limit=100';
        var q = window.AB.profileQuery();
        if (q) {
          url += '&' + q;
        }

        window.AB.api('GET', url).then(function (res) {
          if (res.ok && res.data) {
            self.tasks = res.data.tasks || [];
            self.busy = res.data.busy || [];
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

      get hasTasks() {
        return this.loaded && !this.error && this.tasks && this.tasks.length > 0;
      },

      get isEmptyAll() {
        return this.loaded && !this.error && (!this.tasks || this.tasks.length === 0) && (!this.profile || this.profile === 'all');
      },

      get isEmptyFiltered() {
        return this.loaded && !this.error && (!this.tasks || this.tasks.length === 0) && this.profile && this.profile !== 'all';
      },

      get emptyFilteredTitle() {
        return 'Không có task nào với bộ lọc \'' + this.profile + '\'';
      },

      get filterChip() {
        return window.AB.filterText();
      },

      get filterClass() {
        return window.AB.profileClass(this.profile);
      },

      get busyText() {
        if (!this.busy || this.busy.length === 0) return '';
        var entries = this.busy.map(function (b) {
          return b.project + ' · run ' + b.run_id + ' (' + b.role + ')';
        });
        return 'Đang chạy: ' + entries.join(' · ');
      },

      get groups() {
        var self = this;
        var groupDefs = [
          {
            key: 'running',
            title: 'Đang chạy',
            hint: 'Agent đang làm việc',
            empty: 'Không có run nào đang chạy.',
          },
          {
            key: 'waiting',
            title: 'Chờ bạn',
            hint: 'Duyệt plan hoặc xử lý chỗ autopilot dừng',
            empty: 'Không có task nào chờ bạn.',
          },
          {
            key: 'attention',
            title: 'Lỗi / cần xử lý',
            hint: 'Vướng mắc, cần sửa, bị dừng hoặc lỗi',
            empty: 'Không có task nào cần xử lý.',
          },
          {
            key: 'done',
            title: 'Xong',
            hint: 'QA đạt',
            empty: 'Chưa có task nào xong.',
          },
        ];

        return groupDefs.map(function (g) {
          var all = (self.tasks || []).filter(function (t) {
            return t.group === g.key;
          });
          var items = g.key === 'done' ? all.slice(0, self.doneLimit) : all;
          return {
            key: g.key,
            title: g.title,
            hint: g.hint,
            empty: g.empty,
            items: items,
            total: all.length,
          };
        });
      },

      count: function (g) {
        return String(g.total);
      },

      isEmptyGroup: function (g) {
        return g.total === 0;
      },

      hasMore: function (g) {
        return g.key === 'done' && g.total > this.doneLimit;
      },

      showMore: function () {
        this.doneLimit += 10;
      },

      href: function (t) {
        return '#/task/' + t.id;
      },

      step: function (t, role) {
        if (!t.steps) return null;
        for (var i = 0; i < t.steps.length; i++) {
          if (t.steps[i].role === role) return t.steps[i];
        }
        return null;
      },

      pillClass: function (t, role) {
        var s = this.step(t, role);
        return window.AB.stepClass(s ? s.state : 'cho');
      },

      pillGlyph: function (t, role) {
        var s = this.step(t, role);
        return window.AB.stepGlyph(s ? s.state : 'cho');
      },

      pillGlyphClass: function (t, role) {
        var s = this.step(t, role);
        return window.AB.stepGlyphClass(s ? s.state : 'cho');
      },

      pillLabel: function (t, role) {
        var s = this.step(t, role);
        return window.AB.roleName(role) + ': ' + (s ? window.AB.stepWord(s) : 'chờ');
      },

      // Autopilot stop reason (server stop_detail, shown verbatim) under the banner row.
      hasReason: function (t) {
        return Boolean(t && t.auto_state === 'stopped' && t.stop_detail);
      },

      reasonText: function (t) {
        return (t && t.stop_detail) ? t.stop_detail : '';
      },

      showAutoBadge: function (t) {
        return Boolean(t && (t.auto_state === 'running' || t.auto_state === 'stopping'));
      },

      get autoBadgeClass() {
        return window.AB.stepClass('dang_chay');
      },

      get autoBadgeGlyph() {
        return window.AB.stepGlyph('dang_chay');
      },

      profileClass: function (p) {
        return window.AB.profileClass(p);
      },

      metaText: function (t) {
        return t.project_name + ' · ' + t.slug;
      },

      when: function (t) {
        return 'Cập nhật ' + window.AB.fmtTime(t.updated_at);
      },
    };
  });
})();

// js/task.js — Chi tiết task screen component
(function () {
  window.AB.onReady('task', function () {
    return {
      id: window.AB.route.params.id,
      task: null,
      loaded: false,
      notFound: false,
      error: '',
      stale: false,
      message: '',
      busyAction: '',
      dialog: null,
      lastFocus: null,
      runId: null,

      init: function () {
        var self = this;
        this.load();

        window.AB.every(3000, function () {
          self.load();
        });
      },

      load: function () {
        var self = this;
        if (!this.id) return;
        window.AB.api('GET', '/api/tasks/' + this.id).then(function (res) {
          if (res.ok && res.data) {
            self.task = res.data;
            self.loaded = true;
            self.stale = false;
            self.error = '';
            self.notFound = false;
          } else if (res.status === 404) {
            self.loaded = true;
            self.notFound = true;
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
        return (this.loaded || this.notFound) ? 'yes' : 'no';
      },

      get isLoading() {
        return !this.loaded && !this.error && !this.notFound;
      },

      get hasError() {
        return Boolean(this.error);
      },

      get errorBody() {
        return this.error + '. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get hasTask() {
        return this.loaded && !this.error && !this.notFound && Boolean(this.task);
      },

      get notFoundTitle() {
        return 'Không tìm thấy task ' + this.id;
      },

      get title() {
        return (this.task && this.task.title) ? this.task.title : '';
      },

      get projectLine() {
        if (!this.task || !this.task.project) return '';
        return this.task.project.name_vi + ' (' + this.task.project.id + ')';
      },

      get profile() {
        return (this.task && this.task.project) ? this.task.project.profile : '';
      },

      get profileBadgeClass() {
        return window.AB.profileClass(this.profile);
      },

      get branch() {
        return (this.task && this.task.branch) ? this.task.branch : '';
      },

      get banner() {
        return (this.task && this.task.banner) ? this.task.banner : '';
      },

      get bannerClass() {
        var base = 'inline-flex items-center gap-1 rounded-full px-2 py-1 text-sm font-semibold';
        if (!this.task) return base + ' bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-200';
        var g = this.task.group;
        if (g === 'running' || g === 'waiting') {
          return base + ' bg-indigo-100 text-indigo-800 dark:bg-indigo-900/50 dark:text-indigo-200';
        }
        if (g === 'attention') {
          if (this.task.status === 'blocked' || this.task.status === 'qa_fix') {
            return base + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200';
          }
          return base + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200';
        }
        if (g === 'done') {
          return base + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200';
        }
        return base + ' bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-200';
      },

      get bannerGlyph() {
        if (!this.task) return '○\uFE0E';
        var g = this.task.group;
        if (g === 'running') return '◐\uFE0E';
        if (g === 'waiting') return '○\uFE0E';
        if (g === 'done') return '✓\uFE0E';
        if (g === 'attention') {
          if (this.task.status === 'blocked' || this.task.status === 'qa_fix') {
            return '!\uFE0E';
          }
          return '✕\uFE0E';
        }
        return '○\uFE0E';
      },

      get hasRuns() {
        return Boolean(this.task && this.task.runs && this.task.runs.length > 0);
      },

      get noRuns() {
        return Boolean(this.task && (!this.task.runs || this.task.runs.length === 0));
      },

      get isBlocked() {
        return Boolean(this.task && (this.task.status === 'blocked' || (this.task.plan && this.task.plan.blockers)));
      },

      get isQaFix() {
        return Boolean(this.task && (this.task.status === 'qa_fix' || (this.task.qa && this.task.qa.verdict === 'Cần sửa')));
      },

      get blockedBody() {
        var slug = (this.task && this.task.slug) ? this.task.slug : '';
        return 'Sửa .plan.md trong editor của bạn rồi commit với message \'' + slug + ': plan\', hoặc chạy lại Lead.';
      },

      get qaFixBody() {
        var slug = (this.task && this.task.slug) ? this.task.slug : '';
        return 'Xem báo cáo QA bên dưới. Sửa .plan.md rồi commit \'' + slug + ': plan\', hoặc chạy lại Lead; sau đó chạy lại Dev.';
      },

      get isDone() {
        return Boolean(this.task && this.task.status === 'qa_pass');
      },

      get doneText() {
        var branch = (this.task && this.task.branch) ? this.task.branch : '';
        return 'Pipeline đã xong. Kiểm tra và merge nhánh \'' + branch + '\' bằng git.';
      },

      get hasPlan() {
        return Boolean(this.task && this.task.plan);
      },

      get noPlan() {
        return Boolean(this.task && !this.task.plan);
      },

      get planNote() {
        var slug = (this.task && this.task.slug) ? this.task.slug : '';
        return 'Chỉ xem — sửa trong editor của bạn rồi commit \'' + slug + ': plan\'; dashboard tự nhận commit mới';
      },

      get planPath() {
        if (!this.task || !this.task.project || !this.task.plan) return '';
        return this.task.project.path + '/' + this.task.plan.path;
      },

      get planCommitText() {
        if (!this.task || !this.task.plan || !this.task.plan.commit) return '';
        return 'Commit plan: ' + this.task.plan.commit.slice(0, 7);
      },

      get planSourceText() {
        if (!this.task || !this.task.plan || this.task.plan.source !== 'branch') return '';
        return 'Đang đọc từ nhánh ' + (this.task.branch || '') + ' (dự án đang ở nhánh khác)';
      },

      get planDirtyText() {
        if (!this.task || !this.task.plan || !this.task.plan.dirty) return '';
        var slug = this.task.slug || '';
        return 'Plan có thay đổi chưa commit — commit với message \'' + slug + ': plan\' để Dev dùng bản mới';
      },

      get planErrors() {
        return (this.task && this.task.plan && this.task.plan.errors) ? this.task.plan.errors : [];
      },

      get hasPlanErrors() {
        return this.planErrors.length > 0;
      },

      get planHead() {
        if (!this.task || !this.task.plan || !this.task.plan.sections) return [];
        return this.task.plan.sections.filter(function (s) {
          return s.name === 'Mục tiêu';
        });
      },

      get planTail() {
        if (!this.task || !this.task.plan || !this.task.plan.sections) return [];
        var secs = this.task.plan.sections;
        var names = ['Không được đụng tới', 'Cách kiểm tra', 'Vướng mắc'];
        var result = [];
        names.forEach(function (n) {
          for (var i = 0; i < secs.length; i++) {
            if (secs[i].name === n) {
              result.push(secs[i]);
              break;
            }
          }
        });
        return result;
      },

      get changes() {
        return (this.task && this.task.plan && this.task.plan.changes) ? this.task.plan.changes : [];
      },

      get hasChanges() {
        return this.changes.length > 0;
      },

      get noChanges() {
        return this.changes.length === 0;
      },

      get hasQa() {
        return Boolean(this.task && this.task.qa);
      },

      get qaSections() {
        return (this.task && this.task.qa && this.task.qa.sections) ? this.task.qa.sections : [];
      },

      get qaVerdictText() {
        if (!this.task || !this.task.qa || !this.task.qa.verdict) return '';
        var glyph = (this.task.qa.verdict === 'Đạt') ? '✓\uFE0E ' : '!\uFE0E ';
        return glyph + this.task.qa.verdict;
      },

      get qaVerdictClass() {
        var base = 'inline-flex items-center gap-1 rounded-full px-2 py-1 text-sm font-semibold';
        if (!this.task || !this.task.qa) return base;
        if (this.task.qa.verdict === 'Đạt') {
          return base + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200';
        }
        return base + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200';
      },

      get qaCardRing() {
        return this.isQaFix ? 'ring-2 ring-amber-500 dark:ring-amber-400' : 'ring-1 ring-slate-200 dark:ring-slate-700';
      },

      get runs() {
        return (this.task && this.task.runs) ? this.task.runs : [];
      },

      get messageText() {
        return this.message;
      },

      get dialogOpen() {
        return Boolean(this.dialog);
      },

      get dialogTitle() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') {
          return 'Kill run ' + this.dialog.run_id + ' (' + window.AB.roleName(this.runningStepRole) + ')?';
        }
        if (this.dialog.id === 'relead') {
          var slug = (this.task && this.task.slug) ? this.task.slug : '';
          return 'Chạy lại Lead cho task ' + slug + '?';
        }
        return '';
      },

      get dialogBody() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') {
          return 'Agent và mọi tiến trình con của nó sẽ bị dừng ngay. Thay đổi chưa commit trong thư mục dự án vẫn còn nguyên; task chuyển sang trạng thái killed.';
        }
        if (this.dialog.id === 'relead') {
          var slug = (this.task && this.task.slug) ? this.task.slug : '';
          return 'Lead sẽ lập lại .plan.md trên nhánh task/' + slug + ' và commit \'' + slug + ': plan\'. Bản plan hiện tại vẫn còn trong lịch sử git.';
        }
        return '';
      },

      get dialogOkLabel() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') return 'Kill run';
        if (this.dialog.id === 'relead') return 'Chạy lại Lead';
        return '';
      },

      get dialogCancelLabel() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') return 'Giữ chạy tiếp';
        if (this.dialog.id === 'relead') return 'Giữ plan hiện tại';
        return '';
      },

      get dialogOkClass() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') {
          return 'bg-rose-600 text-white hover:bg-rose-700';
        }
        return 'bg-indigo-600 text-white hover:bg-indigo-700';
      },

      get runningStepRole() {
        if (!this.task || !this.task.steps) return 'Lead';
        for (var i = 0; i < this.task.steps.length; i++) {
          if (this.task.steps[i].state === 'dang_chay') {
            return this.task.steps[i].role;
          }
        }
        return 'Lead';
      },

      step: function (role) {
        if (!this.task || !this.task.steps) return null;
        for (var i = 0; i < this.task.steps.length; i++) {
          if (this.task.steps[i].role === role) return this.task.steps[i];
        }
        return null;
      },

      stepBadgeClass: function (role) {
        var s = this.step(role);
        return window.AB.stepClass(s ? s.state : 'cho');
      },

      stepGlyph: function (role) {
        var s = this.step(role);
        return window.AB.stepGlyph(s ? s.state : 'cho');
      },

      stepGlyphClass: function (role) {
        var s = this.step(role);
        return window.AB.stepGlyphClass(s ? s.state : 'cho');
      },

      stepLabel: function (role) {
        var s = this.step(role);
        return s ? s.label : 'chờ';
      },

      hasRun: function (role) {
        var s = this.step(role);
        return Boolean(s && s.run_id !== null && s.run_id !== undefined);
      },

      noRun: function (role) {
        return !this.hasRun(role);
      },

      isCurrent: function (role) {
        return Boolean(this.task && this.task.current_step === role && this.hasRuns);
      },

      stepRing: function (role) {
        return this.isCurrent(role)
          ? 'ring-2 ring-indigo-600 dark:ring-indigo-400'
          : 'ring-1 ring-slate-200 dark:ring-slate-700';
      },

      stepCurrentAttr: function (role) {
        return this.isCurrent(role) ? 'step' : false;
      },

      tokensText: function (role) {
        var s = this.step(role);
        if (!s || !window.AB.tokensKnown(s)) {
          return 'Token: vào — · ra — · cache — / —';
        }
        return 'Token: vào ' + window.AB.fmtNum(s.input_tokens) +
          ' · ra ' + window.AB.fmtNum(s.output_tokens) +
          ' · cache ' + window.AB.fmtNum(s.cache_read_tokens) +
          ' / ' + window.AB.fmtNum(s.cache_write_tokens);
      },

      durationText: function (role) {
        var s = this.step(role);
        return 'Thời gian: ' + (s && s.duration_ms !== null && s.duration_ms !== undefined ? window.AB.fmtDuration(s.duration_ms) : '—');
      },

      costText: function (role) {
        var s = this.step(role);
        if (!s || !window.AB.costKnown(s)) {
          return 'Chi phí: —';
        }
        return 'Chi phí: ' + window.AB.fmtCost(s.cost_usd);
      },

      actionsFor: function (role) {
        if (!this.task || this.task.current_step !== role || !this.task.actions) return [];
        return this.task.actions;
      },

      hasActions: function (role) {
        return this.actionsFor(role).length > 0;
      },

      actionClass: function (a) {
        if (a.id === 'kill') {
          return 'bg-rose-600 text-white hover:bg-rose-700';
        }
        var actions = (this.task && this.task.actions) ? this.task.actions : [];
        var firstEnabled = null;
        for (var i = 0; i < actions.length; i++) {
          if (actions[i].enabled) {
            firstEnabled = actions[i];
            break;
          }
        }
        if (firstEnabled && firstEnabled.id === a.id) {
          return 'bg-indigo-600 text-white hover:bg-indigo-700';
        }
        return 'text-slate-900 ring-1 ring-slate-300 hover:bg-slate-100 dark:text-slate-100 dark:ring-slate-600 dark:hover:bg-slate-700';
      },

      actionDisabled: function (a) {
        return !a.enabled || Boolean(this.busyAction);
      },

      actionText: function (a) {
        if (this.busyAction === a.id) {
          return a.id === 'kill' ? 'Đang dừng...' : 'Đang giao...';
        }
        return a.label;
      },

      reasonId: function (a) {
        return 'ab-act-reason-' + a.id;
      },

      showReason: function (a) {
        return !a.enabled && Boolean(a.reason);
      },

      actionHelp: function (a) {
        if (!a.enabled) return '';
        var slug = (this.task && this.task.slug) ? this.task.slug : '';
        if (a.id === 'run_dev') {
          return 'Dev sẽ sửa code trên nhánh task/' + slug + '.';
        }
        if (a.id === 'run_qa') {
          return 'QA chỉ đọc diff và ghi .qa-report.md.';
        }
        return '';
      },

      onAction: function (a) {
        if (a.id === 'kill' || a.id === 'relead') {
          this.lastFocus = document.activeElement;
          this.dialog = a;
          window.AB.lockScroll(true);
          window.AB.focusLater('ab-dlg-cancel');
        } else {
          this.runAction(a);
        }
      },

      dialogCancel: function () {
        this.dialog = null;
        window.AB.lockScroll(false);
        if (this.lastFocus && this.lastFocus.focus) {
          this.lastFocus.focus();
        }
      },

      dialogOk: function () {
        var act = this.dialog;
        this.dialog = null;
        window.AB.lockScroll(false);
        if (act) {
          this.runAction(act);
        }
      },

      runAction: function (a) {
        var self = this;
        this.busyAction = a.id;
        if (a.id === 'kill') {
          window.AB.api('POST', '/api/runs/' + a.run_id + '/kill', {}).then(function (res) {
            self.busyAction = '';
            if (res.ok && res.data) {
              self.message = res.data.message || (res.data.ok ? ('Đã dừng run ' + a.run_id) : res.data.error);
            } else {
              self.message = (res.data && res.data.error) ? res.data.error : window.AB.errorText(res);
            }
            self.load();
          });
        } else if (a.id === 'run_dev' || a.id === 'run_qa' || a.id === 'relead') {
          var sub = (a.id === 'run_dev') ? 'dev' : ((a.id === 'run_qa') ? 'qa' : 'relead');
          window.AB.api('POST', '/api/tasks/' + this.id + '/' + sub, {}).then(function (res) {
            self.busyAction = '';
            if (res.status === 202 && res.data) {
              self.message = 'Đã giao (job ' + res.data.job_id + ')';
              self.load();
            } else if (res.status === 409 && res.data && res.data.error) {
              self.message = res.data.error;
            } else {
              self.message = window.AB.errorText(res);
            }
          });
        }
      },

      copyPath: function () {
        var self = this;
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(this.planPath).then(function () {
            self.message = 'Đã sao chép đường dẫn';
          }).catch(function () {
            self.selectPlanPath();
          });
        } else {
          this.selectPlanPath();
        }
      },

      selectPlanPath: function () {
        var el = document.getElementById('ab-plan-path');
        if (el && window.getSelection) {
          var range = document.createRange();
          range.selectNodeContents(el);
          var sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
        }
        this.message = 'Đã sao chép đường dẫn';
      },

      sectionRing: function (name) {
        return (name === 'Vướng mắc' && this.isBlocked)
          ? 'ring-2 ring-amber-500 dark:ring-amber-400'
          : 'ring-1 ring-slate-200 dark:ring-slate-700';
      },

      sectionText: function (s) {
        return (s && s.text && s.text.trim()) ? s.text : '(trống)';
      },

      sectionTextClass: function (name) {
        return name === 'Cách kiểm tra'
          ? 'whitespace-pre-wrap break-words font-mono'
          : 'whitespace-pre-wrap break-words';
      },

      runRole: function (r) {
        return window.AB.roleName(r.role);
      },

      runTokens: function (r) {
        if (!window.AB.tokensKnown(r)) return '—';
        return 'vào ' + window.AB.fmtNum(r.input_tokens) + ' · ra ' + window.AB.fmtNum(r.output_tokens);
      },

      runCost: function (r) {
        if (!window.AB.costKnown(r)) return '—';
        return window.AB.fmtCost(r.cost_usd);
      },

      runStart: function (r) {
        return window.AB.fmtTime(r.started_at);
      },

      runDuration: function (r) {
        if (r.ended_at === null || r.ended_at === undefined) return '—';
        return window.AB.fmtDuration(r.ended_at - r.started_at);
      },

      isViewing: function (r) {
        return r.id === this.runId;
      },

      runRowClass: function (r) {
        return this.isViewing(r)
          ? 'bg-indigo-100 text-indigo-900 dark:bg-indigo-900/40 dark:text-indigo-100'
          : '';
      },

      runButtonText: function (r) {
        return this.isViewing(r) ? 'Đang xem' : 'Xem log';
      },

      selectRun: function (id) {}
    };
  });
})();

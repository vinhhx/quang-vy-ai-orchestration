// js/assign.js — Giao việc screen component
(function () {
  window.AB.onReady('assign', function () {
    return {
      request: '',
      query: '',
      items: [],
      open: false,
      active: -1,
      searching: false,
      suggestError: '',
      suggested: false,
      selected: null,
      slug: '',
      slugEdited: false,
      preflight: null,
      preflightPending: false,
      submitting: false,
      accepted: false,
      jobId: null,
      warning: '',
      error: '',
      candidates: [],
      output: [],
      jobFailed: false,
      profile: window.AB.profile(),
      loaded: false,
      workflows: [],
      workflowErrors: [],
      projectDefaults: {},
      workflowsLoading: true,
      workflowsFailed: false,
      workflowName: 'mac-dinh',
      workflowTouched: false,

      _slugSeq: 0,
      _suggestSeq: 0,
      _preflightSeq: 0,
      _requestTimer: null,
      _queryTimer: null,
      _searchTimer: null,

      init: function () {
        var self = this;
        this.loaded = true;
        this.loadWorkflows();

        var profileHandler = function (e) {
          self.profile = (e.detail && e.detail.profile) ? e.detail.profile : window.AB.profile();
          if (self.open) {
            self.suggest();
          }
        };
        window.addEventListener('ab:profile', profileHandler);

        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
          if (self._requestTimer) clearTimeout(self._requestTimer);
          if (self._queryTimer) clearTimeout(self._queryTimer);
          if (self._searchTimer) clearTimeout(self._searchTimer);
        });
      },

      // Read-only: GET /api/workflows (workflows are edited in workflows.json only, D-23).
      loadWorkflows: function () {
        var self = this;
        this.workflowsLoading = true;
        return window.AB.api('GET', '/api/workflows').then(function (res) {
          self.workflowsLoading = false;
          if (res && res.ok && res.data && Array.isArray(res.data.workflows)) {
            self.workflows = res.data.workflows;
            self.workflowErrors = Array.isArray(res.data.errors) ? res.data.errors : [];
            self.projectDefaults = (res.data.project_defaults && typeof res.data.project_defaults === 'object') ? res.data.project_defaults : {};
            self.workflowsFailed = false;
            if (!self.workflowTouched || !self.hasWorkflow(self.workflowName)) {
              self.workflowName = self.defaultWorkflowFor(self.selected);
            }
          } else {
            self.workflows = [];
            self.workflowErrors = [];
            self.projectDefaults = {};
            self.workflowsFailed = true;
          }
        });
      },

      hasWorkflow: function (name) {
        if (!name || !this.workflows) return false;
        for (var i = 0; i < this.workflows.length; i++) {
          if (this.workflows[i].name === name) return true;
        }
        return false;
      },

      findWorkflow: function (name) {
        if (!name || !this.workflows) return null;
        for (var i = 0; i < this.workflows.length; i++) {
          if (this.workflows[i].name === name) return this.workflows[i];
        }
        return null;
      },

      projectDefault: function (project) {
        if (!project || !project.id || !this.projectDefaults) return null;
        return this.projectDefaults[project.id] || null;
      },

      defaultWorkflowFor: function (project) {
        var d = this.projectDefault(project);
        if (d && d.effective) return d.effective;
        if (project && project.workflow && this.hasWorkflow(project.workflow)) return project.workflow;
        return 'mac-dinh';
      },

      pickWorkflow: function (name) {
        if (!name) return;
        this.workflowName = name;
        this.workflowTouched = true;
      },

      onWorkflowChange: function (event) {
        var target = event && event.target;
        this.pickWorkflow((target && target.value) ? target.value : this.workflowName);
      },

      workflowChecked: function (w) {
        return Boolean(w && w.name === this.workflowName);
      },

      workflowOptionClass: function (w) {
        if (this.workflowChecked(w)) {
          return 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400';
        }
        return 'ring-1 ring-slate-300 hover:bg-slate-50 dark:ring-slate-600 dark:hover:bg-slate-700/50';
      },

      optionChips: function (w) {
        if (!w) return [];
        var chips = [];
        var d = this.projectDefault(this.selected);
        if (d && d.effective === w.name) {
          chips.push('mặc định của dự án');
        }
        chips.push(w.source === 'builtin' ? 'có sẵn' : 'từ workflows.json');
        chips.push(w.auto ? 'autopilot' : 'chạy tay');
        return chips;
      },

      onRequestInput: function () {
        var self = this;
        if (this._requestTimer) clearTimeout(this._requestTimer);
        this._requestTimer = setTimeout(function () {
          if (!self.slugEdited) {
            var seq = ++self._slugSeq;
            window.AB.api('GET', '/api/slug?text=' + encodeURIComponent(self.request || '')).then(function (res) {
              if (seq !== self._slugSeq) return;
              if (res.ok && res.data && typeof res.data.slug === 'string') {
                self.slug = res.data.slug;
              }
              self.refreshPreflight();
            });
          } else {
            self.refreshPreflight();
          }
        }, 300);
      },

      onQueryInput: function () {
        var self = this;
        this.selected = null;
        this.preflight = null;
        this.candidates = [];
        this.active = -1;
        this.open = true;
        if (this._queryTimer) clearTimeout(this._queryTimer);
        this._queryTimer = setTimeout(function () {
          self.suggest();
        }, 200);
      },

      onQueryFocus: function () {
        this.open = true;
        this.suggest();
      },

      onQueryBlur: function () {
        this.open = false;
      },

      suggest: function () {
        var self = this;
        var seq = ++this._suggestSeq;
        var url = '/api/projects/suggest?q=' + encodeURIComponent(this.query || '');
        var pq = window.AB.profileQuery();
        if (pq) {
          url += '&' + pq;
        }
        if (this._searchTimer) clearTimeout(this._searchTimer);
        this._searchTimer = setTimeout(function () {
          if (seq === self._suggestSeq) {
            self.searching = true;
          }
        }, 200);

        window.AB.api('GET', url).then(function (res) {
          if (seq !== self._suggestSeq) return;
          if (self._searchTimer) clearTimeout(self._searchTimer);
          self.searching = false;
          if (res.ok && res.data && Array.isArray(res.data.items)) {
            self.items = res.data.items;
            self.suggested = true;
            self.suggestError = '';
            self.active = -1;
          } else {
            self.items = [];
            self.suggested = true;
            self.suggestError = 'Không tải được danh sách dự án. Thử gõ lại.';
            self.active = -1;
          }
        });
      },

      onQueryKey: function (event) {
        var key = event.key;
        if (key === 'ArrowDown') {
          event.preventDefault();
          if (!this.open) {
            this.open = true;
            this.suggest();
            return;
          }
          if (!this.items || this.items.length === 0) return;
          var next = this.active + 1;
          while (next < this.items.length && this.items[next].missing) {
            next++;
          }
          if (next < this.items.length) {
            this.active = next;
            var el = document.getElementById('ab-opt-' + this.active);
            if (el && typeof el.scrollIntoView === 'function') {
              el.scrollIntoView({ block: 'nearest' });
            }
          }
        } else if (key === 'ArrowUp') {
          event.preventDefault();
          if (!this.open || !this.items || this.items.length === 0) return;
          var prev = this.active - 1;
          while (prev >= 0 && this.items[prev].missing) {
            prev--;
          }
          if (prev >= 0) {
            this.active = prev;
            var el = document.getElementById('ab-opt-' + this.active);
            if (el && typeof el.scrollIntoView === 'function') {
              el.scrollIntoView({ block: 'nearest' });
            }
          }
        } else if (key === 'Enter') {
          if (this.open && this.active >= 0 && this.items && this.items[this.active]) {
            var item = this.items[this.active];
            if (!item.missing) {
              event.preventDefault();
              this.choose(item);
            }
          }
        } else if (key === 'Escape') {
          event.preventDefault();
          this.open = false;
        } else if (key === 'Tab') {
          this.open = false;
        }
      },

      onOptionDown: function (event) {
        event.preventDefault();
        var idx = Number(event.currentTarget.dataset.index);
        if (!isNaN(idx) && this.items && this.items[idx]) {
          this.choose(this.items[idx]);
        }
      },

      choose: function (item) {
        if (!item || item.missing) return;
        this.selected = {
          id: item.id,
          name_vi: item.name_vi,
          profile: item.profile,
          workflow: item.workflow || null,
        };
        this.query = item.name_vi + ' (' + item.id + ')';
        this.open = false;
        this.active = -1;
        this.selectDefaultWorkflow();
        this.refreshPreflight();
      },

      // The project's default, unless the user picked a radio by hand (UI-SPEC B5).
      selectDefaultWorkflow: function () {
        if (this.workflowTouched) return;
        this.workflowName = this.defaultWorkflowFor(this.selected);
      },

      chooseCandidate: function (c) {
        if (!c) return;
        this.selected = {
          id: c.id,
          name_vi: c.name_vi,
          profile: c.profile,
          workflow: c.workflow || null,
        };
        this.query = c.name_vi + ' (' + c.id + ')';
        this.open = false;
        this.active = -1;
        this.candidates = [];
        this.error = '';
        this.selectDefaultWorkflow();
        this.refreshPreflight();
      },

      onSlugInput: function () {
        this.slugEdited = true;
        this.refreshPreflight();
      },

      onSlugBlur: function () {
        var self = this;
        if (!this.slug) {
          this.refreshPreflight();
          return;
        }
        var seq = ++this._slugSeq;
        window.AB.api('GET', '/api/slug?text=' + encodeURIComponent(this.slug)).then(function (res) {
          if (seq !== self._slugSeq) return;
          if (res.ok && res.data && typeof res.data.slug === 'string') {
            self.slug = res.data.slug;
          }
          self.refreshPreflight();
        });
      },

      refreshPreflight: function () {
        var self = this;
        if (!this.selected || !this.selected.id || !this.slug || !this.slug.trim()) {
          this.preflight = null;
          this.preflightPending = false;
          return;
        }
        var seq = ++this._preflightSeq;
        this.preflightPending = true;
        var url = '/api/preflight?project=' + encodeURIComponent(this.selected.id) + '&slug=' + encodeURIComponent(this.slug.trim());
        window.AB.api('GET', url).then(function (res) {
          if (seq !== self._preflightSeq) return;
          self.preflightPending = false;
          if (res.ok && res.data) {
            self.preflight = res.data;
          } else {
            self.preflight = null;
          }
        });
      },

      submit: async function () {
        if (this.startDisabled) return;
        this.submitting = true;
        const res = await window.AB.api('POST', '/api/tasks', this.postBody);
        if (res.status === 202 && res.data) {
          this.accepted = true;
          this.jobId = res.data.job_id;
          this.warning = res.data.warning || '';
          this.error = '';
          this.candidates = [];

          var self = this;
          var pollId = window.AB.every(500, async function () {
            const jobRes = await window.AB.api('GET', '/api/jobs/' + self.jobId);
            if (!jobRes.ok || !jobRes.data) return;
            const job = jobRes.data;
            if (job.task_id) {
              clearInterval(pollId);
              window.location.hash = '#/task/' + job.task_id;
            } else if (job.state === 'exited') {
              clearInterval(pollId);
              self.jobFailed = true;
              self.output = job.output || [];
              self.submitting = false;
              self.accepted = false;
            }
          });
        } else if (res.status === 409 && res.data && res.data.candidates && res.data.candidates.length > 0) {
          this.candidates = res.data.candidates;
          this.error = res.data.error || '';
          this.submitting = false;
        } else {
          this.error = window.AB.errorText(res);
          this.submitting = false;
        }
      },

      get loadedAttr() {
        return this.loaded ? 'yes' : 'no';
      },

      get expandedAttr() {
        return this.open ? 'true' : 'false';
      },

      get activeId() {
        return (this.open && this.active >= 0) ? 'ab-opt-' + this.active : false;
      },

      get hasItems() {
        return Boolean(this.items && this.items.length > 0);
      },

      get listVisible() {
        return Boolean(this.open && this.hasItems);
      },

      get chipText() {
        return this.selected ? this.selected.name_vi + ' (' + this.selected.id + ')' : '';
      },

      get selectedProfile() {
        return this.selected ? this.selected.profile : '';
      },

      get profileNote() {
        return this.selected ? 'Task chạy bằng profile ' + this.selected.profile + ' của dự án — đổi profile dự án trong projects_map.json, không đổi ở đây' : '';
      },

      get showPreflight() {
        return Boolean(this.selected && this.slug && this.preflight);
      },

      get branchName() {
        return (this.preflight && this.preflight.current_branch) ? this.preflight.current_branch : '';
      },

      get planLine() {
        if (!this.preflight) return '';
        if (this.preflight.replan) {
          return 'Lập lại plan trên nhánh hiện tại (' + this.preflight.branch + ')';
        }
        return 'Sẽ tạo nhánh ' + this.preflight.branch;
      },

      get warningText() {
        return (this.preflight && this.preflight.warning) || this.warning || '';
      },

      get reasonHead() {
        if (!this.preflight || !this.preflight.reason) return '';
        var r = this.preflight.reason;
        var idx = r.indexOf('\n');
        return idx !== -1 ? r.slice(0, idx) : r;
      },

      get hasRefusal() {
        return Boolean(this.preflight && !this.preflight.can_start && this.preflight.reason);
      },

      get hasDirty() {
        return Boolean(this.preflight && this.preflight.dirty && this.preflight.dirty.length > 0);
      },

      get dirtyText() {
        return (this.preflight && this.preflight.dirty) ? this.preflight.dirty.join('\n') : '';
      },

      get hasCandidates() {
        return Boolean(this.candidates && this.candidates.length > 0);
      },

      get jobOutputText() {
        return (this.output && this.output.length > 0) ? this.output.join('\n') : '';
      },

      get canStart() {
        return Boolean(
          this.selected &&
          this.request && this.request.trim() &&
          this.slug && this.slug.trim() &&
          this.preflight && this.preflight.can_start &&
          !this.preflightPending &&
          !this.submitting &&
          !this.accepted
        );
      },

      get startDisabled() {
        return !this.canStart;
      },

      get disabledReason() {
        if (this.submitting || this.accepted) return '';
        if (!this.selected) return 'Chọn đúng một dự án trong danh sách';
        if (!this.request || !this.request.trim()) return 'Nhập yêu cầu';
        if (!this.slug || !this.slug.trim()) return 'Nhập tên task';
        if (this.preflightPending || !this.preflight) return 'Đang kiểm tra...';
        if (this.reasonHead) return this.reasonHead;
        return '';
      },

      // `workflow` only when the list loaded; otherwise the server default applies.
      get postBody() {
        var body = {
          project: this.selected ? this.selected.id : '',
          request: this.request,
          name: this.slug,
        };
        if (this.workflowsLoaded && this.workflowName) {
          body.workflow = this.workflowName;
        }
        return body;
      },

      get workflowsLoaded() {
        return Boolean(!this.workflowsLoading && !this.workflowsFailed && this.workflows && this.workflows.length > 0);
      },

      get hasWorkflowErrors() {
        return Boolean(this.workflowErrors && this.workflowErrors.length > 0);
      },

      get errorLines() {
        var errs = this.workflowErrors || [];
        var lines = errs.slice(0, 10);
        if (errs.length > 10) {
          lines.push('… và ' + (errs.length - 10) + ' lỗi khác');
        }
        return lines;
      },

      get selectedHelp() {
        var w = this.findWorkflow(this.workflowName);
        if (!w) return '';
        var d = this.projectDefault(this.selected);
        if (d && d.note && d.effective === w.name) {
          return d.note;
        }
        return w.auto
          ? 'Quy trình này tự chạy autopilot và luôn dừng ở bước bạn duyệt plan.'
          : 'Quy trình này chạy tay: bạn bấm từng bước.';
      },

      get startLabel() {
        if (this.accepted) return 'Đã giao việc — đang mở task...';
        if (this.submitting) return 'Đang giao...';
        return 'Giao việc';
      },

      get suggestStatus() {
        if (this.searching) return 'Đang tìm...';
        if (this.suggestError) return this.suggestError;
        if (this.open && this.suggested && (!this.items || this.items.length === 0) && (this.query && this.query.trim().length > 0)) {
          var msg = 'Không có dự án nào khớp “' + this.query + '”.';
          if (this.profile && this.profile !== 'all') {
            msg += " Bộ lọc profile '" + this.profile + "' đang bật — đổi sang Tất cả để tìm thêm.";
          }
          return msg;
        }
        return '';
      },

      get filterChip() {
        return window.AB.filterText();
      },

      get filterClass() {
        return window.AB.profileClass(this.profile);
      },

      optionText: function (item) {
        return item ? item.name_vi + ' (' + item.id + ')' : '';
      },

      candidateText: function (c) {
        return c ? c.name_vi + ' (' + c.id + ')' : '';
      },

      optionClass: function (item, i) {
        if (i === this.active) {
          return 'bg-indigo-100 text-indigo-900 dark:bg-indigo-900/40 dark:text-indigo-100';
        }
        if (item && item.missing) {
          return 'opacity-60';
        }
        return '';
      },

      optionSelected: function (i) {
        return this.active === i ? 'true' : 'false';
      },

      optionDisabled: function (item) {
        return (item && item.missing) ? 'true' : false;
      },

      profileClass: function (p) {
        return window.AB.profileClass(p);
      },
    };
  });
})();

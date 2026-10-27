// js/assign.js — Giao việc screen component
(function () {
  // Role keys of a team entry (`/api/teams` roles), and the fixed engines in display order.
  var TEAM_ROLES = ['lead', 'dev', 'qa', 'plan_review', 'code_review'];
  var ENGINE_ORDER = ['claude', 'deepseek', 'agy'];
  var BUILTIN_TEAM_NAMES = ['mac-dinh', 'tiet-kiem'];

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
      teams: [],
      teamErrors: [],
      teamBuiltinNames: [],
      projectTeamDefaults: {},
      teamsLoading: true,
      teamsFailed: false,
      teamName: 'mac-dinh',
      teamTouched: false,
      engineReadiness: {},
      staffNames: {},

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
        this.loadTeams();

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

      // Read-only: GET /api/teams, then readiness (/api/engines) and staff names (/api/staff).
      // The two extra calls only feed the warnings; a failure of either just hides them.
      loadTeams: function () {
        var self = this;
        this.teamsLoading = true;
        return window.AB.api('GET', '/api/teams').then(function (res) {
          self.teamsLoading = false;
          if (res && res.ok && res.data && Array.isArray(res.data.teams)) {
            self.teams = res.data.teams;
            self.teamErrors = Array.isArray(res.data.errors) ? res.data.errors : [];
            self.teamBuiltinNames = Array.isArray(res.data.builtin_names) ? res.data.builtin_names : [];
            self.projectTeamDefaults = (res.data.project_defaults && typeof res.data.project_defaults === 'object') ? res.data.project_defaults : {};
            self.teamsFailed = false;
            if (!self.teamTouched || !self.hasTeam(self.teamName)) {
              self.teamName = self.defaultTeamFor(self.selected);
            }
          } else {
            self.teams = [];
            self.teamErrors = [];
            self.teamBuiltinNames = [];
            self.projectTeamDefaults = {};
            self.teamsFailed = true;
          }
          return self.loadTeamExtras();
        });
      },

      loadTeamExtras: function () {
        var self = this;
        var engines = window.AB.api('GET', '/api/engines').then(function (res) {
          var map = {};
          if (res && res.ok && res.data && Array.isArray(res.data.engines)) {
            for (var i = 0; i < res.data.engines.length; i++) {
              var e = res.data.engines[i];
              if (e && e.name && e.readiness && e.readiness !== 'san_sang') {
                map[e.name] = e.word || e.readiness;
              }
            }
          }
          self.engineReadiness = map;
        }).catch(function () {
          self.engineReadiness = {};
        });
        var staff = window.AB.api('GET', '/api/staff').then(function (res) {
          var names = {};
          if (res && res.ok && res.data && Array.isArray(res.data.staff)) {
            for (var i = 0; i < res.data.staff.length; i++) {
              var m = res.data.staff[i];
              if (m && m.id) names[m.id] = m.display_name || m.id;
            }
          }
          self.staffNames = names;
          if (window.AB.setStaffNames) window.AB.setStaffNames(names);
        }).catch(function () {
          self.staffNames = {};
        });
        return Promise.all([engines, staff]);
      },

      hasTeam: function (name) {
        return Boolean(this.findTeam(name));
      },

      findTeam: function (name) {
        if (!name || !this.teams) return null;
        for (var i = 0; i < this.teams.length; i++) {
          if (this.teams[i].name === name) return this.teams[i];
        }
        return null;
      },

      projectTeamDefault: function (project) {
        if (!project || !project.id || !this.projectTeamDefaults) return null;
        return this.projectTeamDefaults[project.id] || null;
      },

      defaultTeamFor: function (project) {
        var d = this.projectTeamDefault(project);
        if (d && d.effective) return d.effective;
        return 'mac-dinh';
      },

      pickTeam: function (name) {
        if (!name) return;
        this.teamName = name;
        this.teamTouched = true;
      },

      onTeamChange: function (event) {
        var target = event && event.target;
        this.pickTeam((target && target.value) ? target.value : this.teamName);
      },

      teamChecked: function (t) {
        return Boolean(t && t.name === this.teamName);
      },

      teamOptionClass: function (t) {
        if (this.teamChecked(t)) {
          return 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400';
        }
        return 'ring-1 ring-slate-300 hover:bg-slate-50 dark:ring-slate-600 dark:hover:bg-slate-700/50';
      },

      teamChips: function (t) {
        if (!t) return [];
        var chips = [];
        var d = this.projectTeamDefault(this.selected);
        if (d && d.effective === t.name) {
          chips.push('mặc định của dự án');
        }
        chips.push(t.source === 'builtin' ? 'có sẵn' : 'từ teams.json');
        return chips;
      },

      // Engines of the team's roles that are not ready, in engine order: [{engine, word}].
      teamNotReady: function (t) {
        var out = [];
        if (!t || !t.roles) return out;
        var used = {};
        for (var i = 0; i < TEAM_ROLES.length; i++) {
          var eng = t.roles[TEAM_ROLES[i]];
          if (eng) used[eng] = true;
        }
        for (var j = 0; j < ENGINE_ORDER.length; j++) {
          var name = ENGINE_ORDER[j];
          if (used[name] && this.engineReadiness && this.engineReadiness[name]) {
            out.push({ engine: name, word: this.engineReadiness[name] });
          }
        }
        return out;
      },

      teamReadinessLines: function (t) {
        var list = this.teamNotReady(t);
        var lines = [];
        for (var i = 0; i < list.length; i++) {
          lines.push('! ' + list[i].engine + ': ' + list[i].word);
        }
        return lines;
      },

      // Display names (without the `người ` prefix) of the humans in the team's roles.
      teamHumanNames: function (t) {
        var names = [];
        if (!t || !t.roles) return names;
        for (var i = 0; i < TEAM_ROLES.length; i++) {
          var eng = t.roles[TEAM_ROLES[i]];
          if (eng && String(eng).indexOf('người:') === 0) {
            var shown = window.AB.engineText(eng, this.staffNames).replace(/^người /, '');
            if (names.indexOf(shown) === -1) names.push(shown);
          }
        }
        return names;
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
        this.selectDefaultTeam();
        this.refreshPreflight();
      },

      // The project's default, unless the user picked a radio by hand (UI-SPEC B5).
      selectDefaultWorkflow: function () {
        if (this.workflowTouched) return;
        this.workflowName = this.defaultWorkflowFor(this.selected);
      },

      selectDefaultTeam: function () {
        if (this.teamTouched) return;
        this.teamName = this.defaultTeamFor(this.selected);
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
        this.selectDefaultTeam();
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
        if (this.teamsLoaded && this.teamName) {
          body.team = this.teamName;
        }
        return body;
      },

      // `team` only when the list loaded; otherwise the server default applies.
      get teamsLoaded() {
        return Boolean(!this.teamsLoading && !this.teamsFailed && this.teams && this.teams.length > 0);
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

      get hasTeamErrors() {
        return Boolean(this.teamErrors && this.teamErrors.length > 0);
      },

      get teamErrorLines() {
        var errs = this.teamErrors || [];
        var lines = errs.slice(0, 10);
        if (errs.length > 10) {
          lines.push('… và ' + (errs.length - 10) + ' lỗi khác');
        }
        return lines;
      },

      get teamErrorClosing() {
        var names = this.teamBuiltinNames;
        if (!names || names.length === 0) {
          names = [];
          for (var i = 0; i < (this.teams || []).length; i++) {
            if (this.teams[i].source === 'builtin') names.push(this.teams[i].name);
          }
        }
        if (names.length === 0) names = BUILTIN_TEAM_NAMES;
        return 'Đang dùng các đội hình có sẵn (' + names.join(', ') + ') cho đến khi bạn sửa xong file.';
      },

      get selectedTeam() {
        return this.findTeam(this.teamName);
      },

      // Amber callout for the selected team; a warning only, never a reason to disable Giao việc.
      get teamWarningText() {
        var t = this.selectedTeam;
        var list = this.teamNotReady(t);
        if (!t || list.length === 0) return '';
        var parts = [];
        for (var i = 0; i < list.length; i++) {
          parts.push(list[i].engine + ' (' + list[i].word + ')');
        }
        return 'Đội hình ' + t.name + ' dùng engine chưa sẵn sàng: ' + parts.join(', ') +
          '. Task vẫn giao được, nhưng bước dùng engine này sẽ dừng với lỗi.';
      },

      get hasTeamWarning() {
        return this.teamWarningText !== '';
      },

      // Helper lines under the list: the server note of the project default (verbatim) and,
      // per human in the team, how their step waits.
      get teamHelpLines() {
        var lines = [];
        var t = this.selectedTeam;
        if (!t) return lines;
        var d = this.projectTeamDefault(this.selected);
        if (d && d.note && d.effective === t.name) {
          lines.push(d.note);
        }
        var humans = this.teamHumanNames(t);
        for (var i = 0; i < humans.length; i++) {
          lines.push('Bước do ' + humans[i] + ' làm sẽ chờ ở trạng thái "Chờ ' + humans[i] +
            ' làm" cho tới khi bạn bấm Xong ở màn hình task.');
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

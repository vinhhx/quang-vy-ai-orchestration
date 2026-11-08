// js/projects.js — Dự án screen component
(function () {
  window.AB.onReady('projects', function () {
    return {
      etag: '',
      projects: [],
      duplicates: [],
      warnings: [],
      editingId: '',
      aliasText: '',
      saving: false,
      telegramSaving: false,
      roots: [],
      rootInput: '',
      rootsDirty: false,
      savingRoots: false,
      addPath: '',
      addType: 'code',
      newProfile: '',
      profileOptions: [],
      profileSaving: false,
      adding: false,
      browseOpen: false,
      browsePath: '',
      browseParent: null,
      browseDirs: [],
      browseTruncated: false,
      browseError: '',
      browseLoading: false,
      telegramCount: 0,
      telegramMax: 30,
      scanning: false,
      scanOut: null,
      statusFor: '',
      status: null,
      statusError: '',
      message: '',
      error: '',
      loaded: false,
      loadError: '',
      stale: false,
      profile: window.AB.profile(),
      workflowList: [],
      workflowErrors: [],
      projectDefaults: {},
      workflowsLoading: true,
      workflowLoadError: '',
      reloadingWorkflows: false,
      builtinStacks: [],
      stackSaving: {},
      stackErrors: {},
      readSourcesSaving: {},
      readSourcesErrors: {},
      timeoutDrafts: {},
      timeoutSaving: {},
      timeoutErrors: {},
      promptModalOpen: false,
      promptProject: null,
      promptRoles: ['lead', 'dev', 'qa', 'plan-review', 'code-review'],
      promptRole: 'lead',
      promptText: '',
      promptDraft: '',
      promptVersion: 'absent',
      promptSource: 'none',
      promptLoading: false,
      promptSaving: false,
      promptError: '',
      promptSuccess: '',
      previewLoading: false,
      previewText: '',
      previewWarnings: [],
      previewWorkflow: '',
      previewTeam: '',
      teamsList: [],

      init: function () {
        var self = this;
        this.load();
        this.loadWorkflows();
        this.loadPromptsMeta();
        this.loadTeams();

        var profileHandler = function (e) {
          self.profile = (e.detail && e.detail.profile) ? e.detail.profile : window.AB.profile();
          self.load();
        };
        window.addEventListener('ab:profile', profileHandler);
        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
        });
      },

      load: function () {
        var self = this;
        var q = window.AB.profileQuery();
        var url = '/api/projects' + (q ? '?' + q : '');
        return window.AB.api('GET', url).then(function (res) {
          if (res.ok && res.data) {
            self.etag = res.data.etag || '';
            self.projects = res.data.projects || [];
            self.duplicates = res.data.duplicates || [];
            self.telegramCount = res.data.telegram_count || 0;
            self.telegramMax = res.data.telegram_max || 30;
            if (Array.isArray(res.data.profiles)) self.profileOptions = res.data.profiles;
            if (!self.rootsDirty) self.roots = (res.data.scan_roots || []).slice();
            self.loaded = true;
            self.stale = false;
            self.loadError = '';
          } else {
            if (!self.loaded) {
              self.loadError = window.AB.errorText(res);
            } else {
              self.stale = true;
            }
          }
        });
      },

      // Read-only: GET /api/workflows (workflows are edited in workflows.json only, D-23).
      loadWorkflows: function () {
        var self = this;
        return window.AB.api('GET', '/api/workflows').then(function (res) {
          self.workflowsLoading = false;
          if (res && res.ok && res.data && Array.isArray(res.data.workflows)) {
            self.workflowList = res.data.workflows;
            self.workflowErrors = Array.isArray(res.data.errors) ? res.data.errors : [];
            self.projectDefaults = (res.data.project_defaults && typeof res.data.project_defaults === 'object') ? res.data.project_defaults : {};
            self.workflowLoadError = '';
          } else {
            self.workflowLoadError = window.AB.errorText(res);
          }
        });
      },

      reloadWorkflows: function () {
        var self = this;
        if (this.reloadingWorkflows) return Promise.resolve();
        this.reloadingWorkflows = true;
        return this.loadWorkflows().then(function () {
          self.reloadingWorkflows = false;
        });
      },

      itemChips: function (w) {
        if (!w) return [];
        var chips = [];
        if (w.source === 'builtin') {
          chips.push('có sẵn');
        } else if (w.source === 'override') {
          chips.push('ghi đè quy trình có sẵn');
        } else {
          chips.push('từ workflows.json');
        }
        chips.push(w.auto ? 'autopilot' : 'chạy tay');
        return chips;
      },

      defaultForLine: function (w) {
        if (!w || !w.default_for || w.default_for.length === 0) return '';
        return 'Mặc định của: ' + w.default_for.join(', ');
      },

      projectWorkflowLine: function (p) {
        var name = '';
        if (p && this.workflowList.length > 0 && this.projectDefaults) {
          var d = this.projectDefaults[p.id];
          if (d && d.effective) name = d.effective;
        }
        if (!name) {
          name = (p && p.workflow) ? p.workflow : 'mac-dinh';
        }
        return 'Quy trình mặc định: ' + name;
      },

      reload: function () {
        this.loadError = '';
        this.load();
      },

      startEdit: function (p) {
        this.editingId = p.id;
        this.aliasText = (p.aliases || []).join(', ');
        this.message = '';
        this.error = '';
      },

      cancelEdit: function () {
        this.editingId = '';
        this.aliasText = '';
      },

      save: function () {
        var self = this;
        this.saving = true;
        this.message = '';
        this.error = '';
        var aliases = (this.aliasText || '')
          .split(',')
          .map(function (s) { return s.trim(); })
          .filter(Boolean);

        window.AB.api('PUT', '/api/projects/' + encodeURIComponent(this.editingId), { aliases: aliases, etag: this.etag }).then(function (res) {
          self.saving = false;
          if (res.ok && res.status === 200) {
            self.message = 'Đã lưu alias';
            self.warnings = (res.data && res.data.warnings) ? res.data.warnings : [];
            self.editingId = '';
            self.aliasText = '';
            self.load();
          } else if (res.status === 409) {
            self.error = (res.data && res.data.error) ? res.data.error : window.AB.errorText(res);
            self.load().then(function () {
              var fresh = null;
              for (var i = 0; i < self.projects.length; i++) {
                if (self.projects[i].id === self.editingId) {
                  fresh = self.projects[i];
                  break;
                }
              }
              if (fresh) {
                self.aliasText = (fresh.aliases || []).join(', ');
              }
            });
          } else {
            self.error = window.AB.errorText(res);
          }
        });
      },

      toggleTelegram: function (p) {
        var self = this;
        if (this.telegramSaving || !p.telegram_offerable) return Promise.resolve();
        this.telegramSaving = true;
        this.message = '';
        this.error = '';
        var enable = !p.telegram;
        return window.AB.api('PUT', '/api/projects/' + encodeURIComponent(p.id) + '/telegram', { enabled: enable, etag: this.etag }).then(function (res) {
          self.telegramSaving = false;
          if (res.ok && res.status === 200 && res.data) {
            self.etag = res.data.etag || self.etag;
            self.projects = res.data.projects || self.projects;
            self.telegramCount = res.data.telegram_count || 0;
            self.telegramMax = res.data.telegram_max || self.telegramMax;
            self.message = enable ? 'Đã bật giao việc qua Telegram' : 'Đã tắt giao việc qua Telegram';
          } else if (res.status === 409) {
            self.error = (res.data && res.data.error) ? res.data.error : 'projects_map.json vừa thay đổi ở nơi khác — đã tải lại danh sách, hãy sửa lại';
            return self.load();
          } else {
            self.error = window.AB.errorText(res);
          }
        });
      },

      // PUT /api/projects/{id}/profile: sets work|personal on one project.
      setProfile: function (p, value) {
        var self = this;
        if (this.profileSaving || !value || value === p.profile) return Promise.resolve();
        this.profileSaving = true;
        this.message = '';
        this.error = '';
        return window.AB.api('PUT', '/api/projects/' + encodeURIComponent(p.id) + '/profile', { profile: value, etag: this.etag }).then(function (res) {
          self.profileSaving = false;
          if (res.ok && res.status === 200 && res.data) {
            self.etag = res.data.etag || self.etag;
            self.projects = res.data.projects || self.projects;
            if (Array.isArray(res.data.profiles)) self.profileOptions = res.data.profiles;
            self.message = 'Đã đặt profile ' + value + ' cho ' + (p.name_vi || p.id);
            return self.load();
          }
          return self.conflictOrError(res);
        });
      },

      issueText: function (p) {
        return (p && p.runnable_issue) ? p.runnable_issue : '';
      },

      hasIssue: function (p) {
        return Boolean(p && p.runnable_issue);
      },

      profileSelectId: function (p) {
        return 'ab-profile-' + p.id;
      },

      // 409 from a write: show the API message and reload the list.
      conflictOrError: function (res) {
        if (res.status === 409) {
          this.error = (res.data && res.data.error) ? res.data.error : 'projects_map.json vừa thay đổi ở nơi khác — đã tải lại danh sách, hãy sửa lại';
          this.rootsDirty = false;
          return this.load();
        }
        this.error = window.AB.errorText(res);
        return Promise.resolve();
      },

      addRoot: function () {
        var v = (this.rootInput || '').trim();
        if (!v) return;
        if (this.roots.indexOf(v) === -1) this.roots.push(v);
        this.rootInput = '';
        this.rootsDirty = true;
      },

      removeRoot: function (i) {
        this.roots.splice(i, 1);
        this.rootsDirty = true;
      },

      saveRoots: function () {
        var self = this;
        if (this.savingRoots) return Promise.resolve();
        this.addRoot();
        this.savingRoots = true;
        this.message = '';
        this.error = '';
        return window.AB.api('PUT', '/api/projects/scan-roots', { roots: this.roots, etag: this.etag }).then(function (res) {
          self.savingRoots = false;
          if (res.ok && res.status === 200 && res.data) {
            self.rootsDirty = false;
            self.etag = res.data.etag || self.etag;
            self.roots = (res.data.scan_roots || []).slice();
            self.message = 'Đã lưu thư mục gốc quét';
          } else {
            return self.conflictOrError(res);
          }
        });
      },

      browse: function (path) {
        var self = this;
        this.browseOpen = true;
        this.browseLoading = true;
        this.browseError = '';
        var url = '/api/fs/dirs' + (path ? '?path=' + encodeURIComponent(path) : '');
        return window.AB.api('GET', url).then(function (res) {
          self.browseLoading = false;
          if (res.ok && res.data) {
            self.browsePath = res.data.path || '';
            self.browseParent = res.data.parent || null;
            self.browseDirs = res.data.dirs || [];
            self.browseTruncated = Boolean(res.data.truncated);
          } else {
            self.browseError = window.AB.errorText(res);
          }
        });
      },

      openBrowse: function () {
        var start = (this.addPath || '').trim();
        return this.browse(start && (start.charAt(0) === '/' || start.indexOf('~/') === 0) ? start : '');
      },

      browseUp: function () {
        if (this.browseParent) return this.browse(this.browseParent);
        return Promise.resolve();
      },

      browseInto: function (d) {
        var base = this.browsePath === '/' ? '' : this.browsePath;
        return this.browse(base + '/' + d.name);
      },

      // Choose a child directory: fills the path and suggests idea when it has no .git.
      pick: function (d) {
        var base = this.browsePath === '/' ? '' : this.browsePath;
        this.addPath = base + '/' + d.name;
        this.addType = d.git ? 'code' : 'idea';
        this.browseOpen = false;
      },

      // Choose the folder being browsed (git status unknown: keep the chosen type).
      pickHere: function () {
        this.addPath = this.browsePath;
        this.browseOpen = false;
      },

      closeBrowse: function () {
        this.browseOpen = false;
      },

      addProject: function () {
        var self = this;
        if (this.adding) return Promise.resolve();
        this.adding = true;
        this.message = '';
        this.error = '';
        var payload = { path: (this.addPath || '').trim(), type: this.addType, etag: this.etag };
        if (this.newProfile) payload.profile = this.newProfile;
        return window.AB.api('POST', '/api/projects', payload).then(function (res) {
          self.adding = false;
          if (res.ok && (res.status === 201 || res.status === 200) && res.data) {
            self.etag = res.data.etag || self.etag;
            self.projects = res.data.projects || self.projects;
            self.telegramCount = res.data.telegram_count || 0;
            self.message = res.status === 201 ? 'Đã thêm dự án' : 'Dự án này đã có trong danh sách';
            self.warnings = (res.data.warnings || []).slice();
            self.addPath = '';
            self.addType = 'code';
            self.newProfile = '';
            return self.load();
          }
          return self.conflictOrError(res);
        });
      },

      typeHint: function () {
        if (this.addType === 'idea') return 'idea: thư mục ý tưởng, không cần git.';
        if (this.addType === 'media') return 'media: thư mục tư liệu, không cần git, không giao việc qua Telegram.';
        return 'code: repo git, cần có thư mục .git.';
      },

      hasRoots: function () {
        return this.roots.length > 0;
      },

      telegramLabel: function (p) {
        return p.telegram ? 'Đang bật — bấm để tắt' : 'Đang tắt — bấm để bật';
      },

      telegramDisabled: function (p) {
        return !p.telegram_offerable || this.telegramSaving;
      },

      telegramLocked: function (p) {
        return !p.telegram_offerable;
      },

      telegramCountText: function () {
        return 'Telegram: ' + this.telegramCount + '/' + this.telegramMax + ' dự án';
      },

      rescan: function () {
        var self = this;
        this.scanning = true;
        this.error = '';
        window.AB.api('POST', '/api/projects/scan', {}).then(function (res) {
          self.scanning = false;
          if (res.ok && res.data) {
            self.scanOut = res.data;
            self.load();
          } else {
            self.error = window.AB.errorText(res);
          }
        });
      },

      showStatus: function (p) {
        var self = this;
        if (this.statusFor === p.id) {
          this.statusFor = '';
          this.status = null;
          this.statusError = '';
          return;
        }
        this.statusFor = p.id;
        this.status = null;
        this.statusError = '';
        window.AB.api('GET', '/api/projects/' + encodeURIComponent(p.id) + '/status').then(function (res) {
          if (self.statusFor !== p.id) return;
          if (res.ok && res.data) {
            self.status = res.data;
          } else {
            self.statusError = window.AB.errorText(res);
          }
        });
      },

      isEditing: function (p) {
        return this.editingId === p.id;
      },

      notEditing: function (p) {
        return this.editingId !== p.id;
      },

      isStatusOpen: function (p) {
        return this.statusFor === p.id;
      },

      hasAliases: function (p) {
        return Boolean(p.aliases && p.aliases.length > 0);
      },

      noAliases: function (p) {
        return !p.aliases || p.aliases.length === 0;
      },

      aliasId: function (p) {
        return 'ab-alias-' + p.id;
      },

      statusTitle: function (p) {
        return p.missing ? 'Thư mục dự án không còn tồn tại' : false;
      },

      profileClass: function (p) {
        var prof = (p && typeof p === 'object') ? p.profile : p;
        return window.AB.profileClass(prof);
      },

      get loadedAttr() {
        return this.loaded ? 'yes' : 'no';
      },

      get isLoading() {
        return !this.loaded && !this.loadError;
      },

      get hasError() {
        return Boolean(this.loadError);
      },

      get errorBody() {
        return this.loadError + '. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get isEmpty() {
        return this.loaded && !this.loadError && (!this.projects || this.projects.length === 0);
      },

      get hasProjects() {
        return this.loaded && !this.loadError && this.projects && this.projects.length > 0;
      },

      get filterChip() {
        return window.AB.filterText();
      },

      get filterClass() {
        return window.AB.profileClass(this.profile);
      },

      get duplicateLines() {
        if (!this.duplicates) return [];
        return this.duplicates.map(function (d) {
          return 'Tên/alias trùng "' + d.key + '": ' + (d.ids || []).join(', ');
        });
      },

      get hasDuplicates() {
        return Boolean(this.duplicates && this.duplicates.length > 0);
      },

      get hasWarnings() {
        return Boolean(this.warnings && this.warnings.length > 0);
      },

      get warningText() {
        return (this.warnings || []).join('\n');
      },

      get scanTitle() {
        var code = (this.scanOut && this.scanOut.exit_code !== undefined && this.scanOut.exit_code !== null) ? this.scanOut.exit_code : 0;
        return 'Kết quả quét (mã thoát ' + code + ')';
      },

      get scanText() {
        if (!this.scanOut) return '';
        var lines = [];
        if (this.scanOut.stdout && this.scanOut.stdout.length > 0) {
          lines = lines.concat(this.scanOut.stdout);
        }
        if (this.scanOut.stderr && this.scanOut.stderr.length > 0) {
          lines = lines.concat(this.scanOut.stderr);
        }
        return lines.join('\n');
      },

      get statusText() {
        if (!this.status) return '';
        var s = this.status;
        var lines = [
          'Dự án: ' + s.project + ' (' + s.path + ')',
          'Nhánh: ' + s.branch,
          '== git status --short ==',
        ];
        if (s.status && s.status.length > 0) {
          lines = lines.concat(s.status);
        } else {
          lines.push('(sạch)');
        }
        lines.push('== git log --oneline -5 ==');
        if (s.log && s.log.length > 0) {
          lines = lines.concat(s.log);
        } else {
          lines.push('(chưa có commit)');
        }
        lines.push('== git diff --stat ==');
        if (s.diff_stat && s.diff_stat.length > 0) {
          lines = lines.concat(s.diff_stat);
        } else {
          lines.push('(không có thay đổi)');
        }
        if (s.task) {
          lines.push('Task ' + s.task.id + ' · ' + s.task.branch + ' · trạng thái ' + s.task.status);
        }
        return lines.join('\n');
      },

      get hasWorkflowList() {
        return Boolean(this.workflowList && this.workflowList.length > 0);
      },

      get hasWorkflowErrors() {
        return Boolean(this.workflowErrors && this.workflowErrors.length > 0);
      },

      get workflowErrorLines() {
        var errs = this.workflowErrors || [];
        var lines = errs.slice(0, 10);
        if (errs.length > 10) {
          lines.push('… và ' + (errs.length - 10) + ' lỗi khác');
        }
        return lines;
      },

      get hasWorkflowLoadError() {
        return Boolean(this.workflowLoadError);
      },

      get workflowErrorBody() {
        return this.workflowLoadError ? this.workflowLoadError + '. Bấm Đọc lại để thử lại.' : '';
      },

      get workflowsPending() {
        return this.workflowsLoading && !this.workflowLoadError;
      },

      get reloadLabel() {
        return this.reloadingWorkflows ? 'Đang đọc...' : 'Đọc lại';
      },

      get saveRootsLabel() {
        return this.savingRoots ? 'Đang lưu...' : 'Lưu thư mục gốc';
      },

      get addLabel() {
        return this.adding ? 'Đang thêm...' : 'Thêm dự án';
      },

      get browseUpDisabled() {
        return !this.browseParent || this.browseLoading;
      },

      get saveLabel() {
        return this.saving ? 'Đang lưu...' : 'Lưu alias';
      },

      get rescanLabel() {
        return this.scanning ? 'Đang quét...' : 'Quét lại';
      },

      loadPromptsMeta: function () {
        var self = this;
        return window.AB.api('GET', '/api/prompts').then(function (res) {
          if (res && res.ok && res.data) {
            self.promptsMeta = res.data;
            self.builtinStacks = Array.isArray(res.data.builtin_stacks) ? res.data.builtin_stacks : [];
          }
        });
      },

      loadTeams: function () {
        var self = this;
        return window.AB.api('GET', '/api/teams').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.teams)) {
            self.teamsList = res.data.teams;
          }
        });
      },

      projectStacks: function (p) {
        return (p && Array.isArray(p.stack)) ? p.stack : [];
      },

      hasNoStack: function (p) {
        return !p || !Array.isArray(p.stack) || p.stack.length === 0;
      },

      allKnownStacks: function () {
        var map = {};
        (this.builtinStacks || []).forEach(function (s) {
          if (s) map[s] = true;
        });
        if (this.promptsMeta && Array.isArray(this.promptsMeta.builtin_stacks)) {
          this.promptsMeta.builtin_stacks.forEach(function (s) {
            if (s) map[s] = true;
          });
        }
        if (this.promptsMeta && this.promptsMeta.layers && Array.isArray(this.promptsMeta.layers.stacks)) {
          this.promptsMeta.layers.stacks.forEach(function (s) {
            if (s) map[s] = true;
          });
        }
        (this.projects || []).forEach(function (p) {
          if (Array.isArray(p.stack)) {
            p.stack.forEach(function (s) {
              if (s) map[s] = true;
            });
          }
        });
        return Object.keys(map).sort();
      },

      availableStacksFor: function (p) {
        var cur = this.projectStacks(p);
        return this.allKnownStacks().filter(function (s) {
          return cur.indexOf(s) === -1;
        });
      },

      isStackSaving: function (id) {
        return Boolean(this.stackSaving && this.stackSaving[id]);
      },

      stackError: function (id) {
        return (this.stackErrors && this.stackErrors[id]) ? this.stackErrors[id] : '';
      },

      addProjectStackFromSelect: function (p, val) {
        if (!val) return;
        this.addProjectStack(p, val);
      },

      promptCustomStack: function (p) {
        var name = window.prompt('Nhập tên stack (chữ thường, số, gạch ngang, ví dụ: vue, python):');
        if (name && name.trim()) {
          this.addProjectStack(p, name.trim());
        }
      },

      addProjectStack: function (p, name) {
        if (!p) return;
        name = (name || '').trim();
        var re = /^[a-z0-9][a-z0-9-_]*$/;
        if (!re.test(name)) {
          this.stackErrors[p.id] = 'Tên stack không hợp lệ (dùng chữ thường, số, gạch ngang, gạch dưới)';
          return;
        }
        var cur = this.projectStacks(p).slice();
        if (cur.indexOf(name) >= 0) return;
        if (cur.length >= 6) {
          this.stackErrors[p.id] = 'Tối đa 6 stack';
          return;
        }
        cur.push(name);
        this.saveStack(p, cur);
      },

      removeProjectStack: function (p, name) {
        if (!p) return;
        var cur = this.projectStacks(p).filter(function (s) {
          return s !== name;
        });
        this.saveStack(p, cur);
      },

      saveStack: function (p, list) {
        var self = this;
        this.stackSaving[p.id] = true;
        this.stackErrors[p.id] = '';
        return window.AB.api('PUT', '/api/projects/' + encodeURIComponent(p.id) + '/stack', {
          stack: list,
          etag: this.etag
        }).then(function (res) {
          self.stackSaving[p.id] = false;
          if (res && res.ok) {
            p.stack = list;
          } else if (res && res.status === 409) {
            self.stackErrors[p.id] = 'projects_map.json vừa đổi ở nơi khác — đã tải lại, vui lòng thử lại.';
            self.load();
          } else {
            self.stackErrors[p.id] = window.AB.errorText(res);
          }
        });
      },

      projectReadSources: function (p) {
        return (p && Array.isArray(p.read_sources)) ? p.read_sources : [];
      },

      hasNoReadSources: function (p) {
        return !p || !Array.isArray(p.read_sources) || p.read_sources.length === 0;
      },

      availableProjectsForReadSources: function (p) {
        if (!p) return [];
        var cur = this.projectReadSources(p);
        return (this.projects || []).filter(function (other) {
          return other.id !== p.id && cur.indexOf(other.id) === -1;
        });
      },

      projectSourceLabel: function (srcId) {
        var found = (this.projects || []).find(function (p) { return p.id === srcId; });
        if (found && found.name_vi) {
          return found.name_vi + ' (' + srcId + ')';
        }
        return srcId;
      },

      isReadSourcesSaving: function (id) {
        return Boolean(this.readSourcesSaving && this.readSourcesSaving[id]);
      },

      hasReadSourcesError: function (id) {
        return Boolean(this.readSourcesErrors && this.readSourcesErrors[id]);
      },

      readSourcesError: function (id) {
        return (this.readSourcesErrors && this.readSourcesErrors[id]) ? this.readSourcesErrors[id] : '';
      },

      addProjectReadSourceFromSelect: function (p, val) {
        if (!val) return;
        this.addProjectReadSource(p, val);
      },

      addProjectReadSource: function (p, srcId) {
        if (!p || !srcId) return;
        var cur = this.projectReadSources(p).slice();
        if (cur.indexOf(srcId) >= 0) return;
        if (cur.length >= 5) {
          this.readSourcesErrors[p.id] = 'Tối đa 5 nguồn';
          return;
        }
        cur.push(srcId);
        this.saveReadSources(p, cur);
      },

      removeProjectReadSource: function (p, srcId) {
        if (!p) return;
        var cur = this.projectReadSources(p).filter(function (s) {
          return s !== srcId;
        });
        this.saveReadSources(p, cur);
      },

      // C1: the project's `timeouts: {role: seconds}`, edited as "dev=3600, qa=1200".
      timeoutDraft: function (p) {
        if (Object.prototype.hasOwnProperty.call(this.timeoutDrafts, p.id)) return this.timeoutDrafts[p.id];
        return window.AB.timeoutsText(p.timeouts);
      },

      setTimeoutDraft: function (p, value) {
        this.timeoutDrafts[p.id] = value;
        this.timeoutErrors[p.id] = '';
      },

      isTimeoutSaving: function (p) {
        return Boolean(this.timeoutSaving[p.id]);
      },

      timeoutError: function (p) {
        return this.timeoutErrors[p.id] || '';
      },

      saveTimeouts: function (p) {
        var self = this;
        var parsed = window.AB.parseTimeouts(this.timeoutDraft(p));
        if (!parsed.ok) {
          this.timeoutErrors[p.id] = parsed.message;
          return Promise.resolve();
        }
        this.timeoutSaving[p.id] = true;
        this.timeoutErrors[p.id] = '';
        return window.AB.api('PUT', '/api/projects/' + encodeURIComponent(p.id) + '/timeouts', {
          timeouts: parsed.value,
          etag: this.etag
        }).then(function (res) {
          self.timeoutSaving[p.id] = false;
          if (res && res.ok) {
            p.timeouts = parsed.value;
            delete self.timeoutDrafts[p.id];
            if (res.data && res.data.etag) self.etag = res.data.etag;
          } else if (res && res.status === 409) {
            self.timeoutErrors[p.id] = 'projects_map.json vừa đổi ở nơi khác — đã tải lại, vui lòng thử lại.';
            self.load();
          } else {
            self.timeoutErrors[p.id] = window.AB.errorText(res) || 'Không lưu được thời gian chạy';
          }
        });
      },

      saveReadSources: function (p, list) {
        var self = this;
        this.readSourcesSaving[p.id] = true;
        this.readSourcesErrors[p.id] = '';
        return window.AB.api('POST', '/api/projects/' + encodeURIComponent(p.id) + '/read-sources', {
          read_sources: list,
          etag: this.etag
        }).then(function (res) {
          self.readSourcesSaving[p.id] = false;
          if (res && res.ok) {
            p.read_sources = list;
            if (res.data && res.data.etag) self.etag = res.data.etag;
          } else if (res && res.status === 409) {
            self.readSourcesErrors[p.id] = 'projects_map.json vừa đổi ở nơi khác — đã tải lại, vui lòng thử lại.';
            self.load();
          } else {
            var msg = window.AB.errorText(res);
            if (res && res.data && res.data.errors && Array.isArray(res.data.errors) && res.data.errors.length) {
              msg = res.data.errors[0].message || msg;
            }
            self.readSourcesErrors[p.id] = msg || 'Không lưu được dự án nguồn';
          }
        });
      },

      openPromptPanel: function (p) {
        this.promptProject = p;
        this.promptRole = 'lead';
        this.promptDraft = '';
        this.promptText = '';
        this.promptError = '';
        this.promptSuccess = '';
        this.previewText = '';
        this.previewWarnings = [];
        this.previewWorkflow = (p && p.workflow) ? p.workflow : 'mac-dinh';
        this.previewTeam = '';
        this.promptModalOpen = true;
        this.loadProjectPromptNote();
      },

      closePromptPanel: function () {
        this.promptModalOpen = false;
        this.promptProject = null;
        this.promptError = '';
        this.promptSuccess = '';
      },

      selectPromptRole: function (role) {
        this.promptRole = role;
        this.previewText = '';
        this.previewWarnings = [];
        this.loadProjectPromptNote();
      },

      loadProjectPromptNote: function () {
        if (!this.promptProject) return Promise.resolve();
        var self = this;
        this.promptLoading = true;
        this.promptError = '';
        this.promptSuccess = '';
        return window.AB.api('GET', '/api/prompts/projects/' + encodeURIComponent(this.promptProject.id) + '/' + this.promptRole).then(function (res) {
          self.promptLoading = false;
          if (res && res.ok && res.data) {
            self.promptText = res.data.text || '';
            self.promptDraft = self.promptText;
            self.promptVersion = res.data.version || 'absent';
            self.promptSource = res.data.source || 'none';
          } else {
            self.promptError = window.AB.errorText(res);
          }
        });
      },

      saveProjectPromptNote: function () {
        if (!this.promptProject || this.isPromptSaveDisabled()) return Promise.resolve();
        var self = this;
        this.promptSaving = true;
        this.promptError = '';
        this.promptSuccess = '';
        return window.AB.api('PUT', '/api/prompts/projects/' + encodeURIComponent(this.promptProject.id) + '/' + this.promptRole, {
          text: this.promptDraft,
          version: this.promptVersion
        }).then(function (res) {
          self.promptSaving = false;
          if (res && res.ok && res.data) {
            self.promptVersion = res.data.version;
            self.promptText = self.promptDraft;
            self.promptSource = 'user';
            self.promptSuccess = 'Đã lưu ghi chú dự án.';
          } else if (res && res.status === 409) {
            var msg = 'Ghi chú vừa bị sửa ở nơi khác — đã tải lại, vui lòng kiểm tra và lưu lại.';
            return self.loadProjectPromptNote().then(function () {
              self.promptError = msg;
            });
          } else {
            self.promptError = window.AB.errorText(res);
          }
        });
      },

      deleteProjectPromptNote: function () {
        if (!this.promptProject) return Promise.resolve();
        var self = this;
        this.promptSaving = true;
        this.promptError = '';
        this.promptSuccess = '';
        return window.AB.api('DELETE', '/api/prompts/projects/' + encodeURIComponent(this.promptProject.id) + '/' + this.promptRole).then(function (res) {
          self.promptSaving = false;
          if (res && res.ok) {
            self.promptDraft = '';
            self.promptText = '';
            self.promptVersion = 'absent';
            self.promptSource = 'none';
            self.promptSuccess = 'Đã xóa ghi chú dự án.';
          } else {
            self.promptError = window.AB.errorText(res);
          }
        });
      },

      loadPromptPreview: function () {
        if (!this.promptProject) return Promise.resolve();
        var self = this;
        this.previewLoading = true;
        var url = '/api/prompts/preview?project=' + encodeURIComponent(this.promptProject.id) + '&role=' + encodeURIComponent(this.promptRole);
        if (this.previewWorkflow) url += '&workflow=' + encodeURIComponent(this.previewWorkflow);
        if (this.previewTeam) url += '&team=' + encodeURIComponent(this.previewTeam);
        return window.AB.api('GET', url).then(function (res) {
          self.previewLoading = false;
          if (res && res.ok && res.data) {
            self.previewText = res.data.prompt || '';
            self.previewWarnings = Array.isArray(res.data.warnings) ? res.data.warnings : [];
          } else {
            self.previewText = 'Lỗi xem trước: ' + window.AB.errorText(res);
            self.previewWarnings = [];
          }
        });
      },

      isPromptPanelOpen: function () {
        return Boolean(this.promptModalOpen);
      },

      promptPanelTitle: function () {
        return this.promptProject ? 'Ghi chú prompt: ' + (this.promptProject.name_vi || this.promptProject.id) : '';
      },

      isRoleActive: function (r) {
        return this.promptRole === r;
      },

      roleTabClass: function (r) {
        return (this.promptRole === r)
          ? 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-200'
          : 'text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700';
      },

      promptByteCount: function () {
        var str = this.promptDraft || '';
        if (typeof TextEncoder !== 'undefined') {
          return new TextEncoder().encode(str).length;
        }
        return encodeURIComponent(str).replace(/%[A-F\d]{2}/g, 'U').length;
      },

      promptByteCountText: function () {
        return this.promptByteCount() + ' / 8192 byte';
      },

      isPromptOverLimit: function () {
        return this.promptByteCount() > 8192;
      },

      promptByteClass: function () {
        return this.isPromptOverLimit() ? 'text-rose-600 dark:text-rose-400 font-bold' : 'text-slate-500 dark:text-slate-400';
      },

      isPromptSaveDisabled: function () {
        return Boolean(this.promptLoading || this.promptSaving || this.isPromptOverLimit());
      },

      hasUserPromptNote: function () {
        return this.promptSource === 'user';
      },

      hasPreviewWarnings: function () {
        return Array.isArray(this.previewWarnings) && this.previewWarnings.length > 0;
      },

      hasPreviewText: function () {
        return Boolean(this.previewText);
      },
    };
  });
})();

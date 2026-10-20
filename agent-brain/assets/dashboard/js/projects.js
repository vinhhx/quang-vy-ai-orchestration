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

      init: function () {
        var self = this;
        this.load();
        this.loadWorkflows();

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

      get saveLabel() {
        return this.saving ? 'Đang lưu...' : 'Lưu alias';
      },

      get rescanLabel() {
        return this.scanning ? 'Đang quét...' : 'Quét lại';
      },
    };
  });
})();

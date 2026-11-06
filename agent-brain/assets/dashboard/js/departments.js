// js/departments.js — Màn hình Phòng ban và quản lý thành viên
(function () {
  window.AB.onReady('departments', function () {
    return {
      loading: false,
      loaded: false,
      error: '',
      version: 'absent',
      fileExists: true,
      errors: [],
      departments: [],
      engines: [],
      modelSuggestions: [],

      // Modal form state
      modalOpen: false,
      isEdit: false,
      formDept: 'phan-tich',
      oldDept: null,
      formId: '',
      formDisplayName: '',
      formEngine: 'claude',
      formModel: '',
      formLevelDe: true,
      formLevelVua: true,
      formLevelKho: false,
      formNotes: '',
      formRules: '',
      fieldErrors: {},
      modalError: '',
      saving: false,

      // Delete confirmation modal state
      delModalOpen: false,
      delId: '',
      delDisplayName: '',
      delDeptId: '',
      delError: '',
      deleting: false,

      // Prompt preview modal state
      promptPreviewModalOpen: false,
      previewMember: null,
      previewDept: null,
      previewRole: 'dev',
      previewProject: '',
      previewPrompt: '',
      previewWarnings: [],
      previewLoading: false,
      previewError: '',
      previewCopied: false,

      // Department rules modal state
      rulesModalOpen: false,
      rulesDeptId: '',
      rulesRole: '',
      rulesText: '',
      rulesDraft: '',
      rulesVersion: 'absent',
      rulesSource: 'builtin',
      rulesLoading: false,
      rulesSaving: false,
      rulesDeleting: false,
      rulesError: '',
      rulesSuccess: '',

      // Init example state
      initing: false,

      init: function () {
        this.loadData();
        this.fetchEngines();
      },

      loadData: function () {
        var self = this;
        self.loading = true;
        self.error = '';
        fetch('/api/departments', { credentials: 'same-origin' })
          .then(function (res) {
            if (!res.ok) throw new Error('HTTP ' + res.status);
            return res.json();
          })
          .then(function (data) {
            self.departments = data.departments || [];
            self.errors = data.errors || [];
            self.version = data.version || 'absent';
            self.fileExists = Boolean(data.file_exists);
            self.loaded = true;
            self.loading = false;
          })
          .catch(function (err) {
            self.error = err.message || 'Không thể tải danh sách phòng ban';
            self.loading = false;
          });
      },

      fetchEngines: function () {
        var self = this;
        fetch('/api/engines', { credentials: 'same-origin' })
          .then(function (res) {
            if (!res.ok) return { engines: [] };
            return res.json();
          })
          .then(function (data) {
            var list = (data.engines || []).filter(function (e) {
              return e.name !== 'người' && !e.name.startsWith('người:');
            });
            // Ensure claude is always in the list
            var hasClaude = list.some(function (e) { return e.name === 'claude'; });
            if (!hasClaude) {
              list.unshift({ name: 'claude', kind: 'claude' });
            }
            self.engines = list;
          })
          .catch(function () {
            self.engines = [{ name: 'claude', kind: 'claude' }];
          });
      },

      fetchModels: function (engine) {
        var self = this;
        if (!engine) {
          self.modelSuggestions = [];
          return;
        }
        fetch('/api/departments/models?engine=' + encodeURIComponent(engine), { credentials: 'same-origin' })
          .then(function (res) {
            if (!res.ok) return { models: [] };
            return res.json();
          })
          .then(function (data) {
            self.modelSuggestions = data.models || [];
          })
          .catch(function () {
            self.modelSuggestions = [];
          });
      },

      reload: function () {
        this.loadData();
      },

      hasErrors: function () {
        return Array.isArray(this.errors) && this.errors.length > 0;
      },

      hasError: function () {
        return Boolean(this.error);
      },

      isLoaded: function () {
        return Boolean(this.loaded);
      },

      loadedAttr: function () {
        return this.loaded ? 'true' : null;
      },

      hasMembers: function (dept) {
        return Boolean(dept && dept.members && dept.members.length > 0);
      },

      levelChipClass: function (lvl) {
        if (lvl === 'de') return 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-300';
        if (lvl === 'vua') return 'bg-sky-100 text-sky-800 dark:bg-sky-900/30 dark:text-sky-300';
        if (lvl === 'kho') return 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300';
        return 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300';
      },

      levelLabel: function (lvl) {
        if (lvl === 'de') return 'Dễ';
        if (lvl === 'vua') return 'Vừa';
        if (lvl === 'kho') return 'Khó';
        return lvl;
      },

      scoreText: function (stats) {
        if (!stats) return '—';
        if (stats.avg_score === null || stats.avg_score === undefined || stats.scores === 0) {
          return '— (0 lượt)';
        }
        return Number(stats.avg_score).toFixed(1) + ' ★ (' + stats.scores + ' lượt)';
      },

      runsText: function (stats) {
        if (!stats) return '0/0';
        return stats.succeeded + '/' + stats.runs;
      },

      runningCount: function (stats) {
        if (!stats) return 0;
        return stats.running || 0;
      },

      readyBadgeClass: function (ready) {
        return ready
          ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-300 ring-1 ring-emerald-300 dark:ring-emerald-700'
          : 'bg-rose-100 text-rose-800 dark:bg-rose-900/30 dark:text-rose-300 ring-1 ring-rose-300 dark:ring-rose-700';
      },

      readyText: function (m) {
        if (!m) return '';
        if (m.ready) return 'Sẵn sàng';
        return m.ready_reason || 'Chưa sẵn sàng';
      },

      hasFieldError: function (field) {
        return Boolean(this.fieldErrors && this.fieldErrors[field]);
      },

      fieldError: function (field) {
        return (this.fieldErrors && this.fieldErrors[field]) || '';
      },

      fieldClass: function (field) {
        if (this.hasFieldError(field)) {
          return 'ring-2 ring-rose-500 focus-visible:ring-rose-500';
        }
        return 'ring-1 ring-slate-300 dark:ring-slate-700';
      },

      openAddModal: function (deptId) {
        this.isEdit = false;
        this.formDept = deptId || 'phan-tich';
        this.oldDept = null;
        this.formId = '';
        this.formDisplayName = '';
        this.formEngine = 'claude';
        this.formModel = '';
        this.formLevelDe = true;
        this.formLevelVua = true;
        this.formLevelKho = false;
        this.formNotes = '';
        this.formRules = '';
        this.fieldErrors = {};
        this.modalError = '';
        this.modalOpen = true;
        this.fetchModels('claude');
      },

      openEditModal: function (deptId, m) {
        this.isEdit = true;
        this.formDept = deptId;
        this.oldDept = deptId;
        this.formId = m.id;
        this.formDisplayName = m.display_name;
        this.formEngine = m.engine;
        this.formModel = m.model || '';
        var lvls = Array.isArray(m.levels) ? m.levels : [];
        this.formLevelDe = lvls.includes('de');
        this.formLevelVua = lvls.includes('vua');
        this.formLevelKho = lvls.includes('kho');
        this.formNotes = m.notes || '';
        this.formRules = m.rules || '';
        this.fieldErrors = {};
        this.modalError = '';
        this.modalOpen = true;
        this.fetchModels(m.engine);
      },

      rulesCharCount: function () {
        return (this.formRules || '').length;
      },

      closeModal: function () {
        this.modalOpen = false;
        this.fieldErrors = {};
        this.modalError = '';
      },

      onEngineChange: function () {
        this.fetchModels(this.formEngine);
      },

      saveMember: function () {
        var self = this;
        self.saving = true;
        self.fieldErrors = {};
        self.modalError = '';

        var levels = [];
        if (self.formLevelDe) levels.push('de');
        if (self.formLevelVua) levels.push('vua');
        if (self.formLevelKho) levels.push('kho');

        var payload = {
          version: self.version,
          department: self.formDept,
          id: self.formId.trim(),
          member: {
            display_name: self.formDisplayName.trim(),
            engine: self.formEngine,
            model: self.formModel.trim() || null,
            levels: levels,
            notes: self.formNotes.trim() || null,
            rules: self.formRules.trim() || null,
          },
          old_department: self.isEdit ? self.oldDept : null,
        };

        fetch('/api/departments/member/save', {
          method: 'POST',
          credentials: 'same-origin',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        })
          .then(function (res) {
            return res.json().then(function (data) {
              return { status: res.status, ok: res.ok, data: data };
            });
          })
          .then(function (result) {
            self.saving = false;
            if (result.ok && result.data && result.data.ok) {
              self.departments = result.data.departments || [];
              self.errors = result.data.errors || [];
              self.version = result.data.version || self.version;
              self.fileExists = true;
              self.closeModal();
            } else if (result.status === 422) {
              var errMap = {};
              var errList = result.data.errors || [];
              errList.forEach(function (e) {
                if (e.field) errMap[e.field] = e.message;
              });
              self.fieldErrors = errMap;
              self.modalError = result.data.error || 'Vui lòng kiểm tra lại thông tin';
            } else if (result.status === 409) {
              self.modalError = result.data.error || 'Dữ liệu bị xung đột';
              self.loadData();
            } else {
              self.modalError = (result.data && result.data.error) || 'Lỗi khi lưu thành viên';
            }
          })
          .catch(function (err) {
            self.saving = false;
            self.modalError = err.message || 'Lỗi mạng khi lưu thành viên';
          });
      },

      openDeleteModal: function (m) {
        this.delId = m.id;
        this.delDisplayName = m.display_name;
        this.delDeptId = m.department || '';
        this.delError = '';
        this.delModalOpen = true;
      },

      closeDeleteModal: function () {
        this.delModalOpen = false;
        this.delError = '';
      },

      confirmDelete: function () {
        var self = this;
        self.deleting = true;
        self.delError = '';

        var payload = {
          version: self.version,
          id: self.delId,
        };

        fetch('/api/departments/member/delete', {
          method: 'POST',
          credentials: 'same-origin',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        })
          .then(function (res) {
            return res.json().then(function (data) {
              return { status: res.status, ok: res.ok, data: data };
            });
          })
          .then(function (result) {
            self.deleting = false;
            if (result.ok && result.data && result.data.ok) {
              self.departments = result.data.departments || [];
              self.errors = result.data.errors || [];
              self.version = result.data.version || self.version;
              self.closeDeleteModal();
            } else {
              self.delError = (result.data && result.data.error) || 'Không thể xóa thành viên';
              if (result.status === 409) {
                self.loadData();
              }
            }
          })
          .catch(function (err) {
            self.deleting = false;
            self.delError = err.message || 'Lỗi mạng khi xóa thành viên';
          });
      },

      initExample: function () {
        var self = this;
        self.initing = true;
        fetch('/api/departments/init-example', {
          method: 'POST',
          credentials: 'same-origin',
        })
          .then(function (res) {
            return res.json().then(function (data) {
              return { status: res.status, ok: res.ok, data: data };
            });
          })
          .then(function (result) {
            self.initing = false;
            if (result.ok && result.data && result.data.ok) {
              self.departments = result.data.departments || [];
              self.errors = result.data.errors || [];
              self.version = result.data.version || self.version;
              self.fileExists = true;
            } else {
              self.error = (result.data && result.data.error) || 'Không thể khởi tạo từ mẫu';
            }
          })
          .catch(function (err) {
            self.initing = false;
            self.error = err.message || 'Lỗi mạng khi khởi tạo từ mẫu';
          });
      },

      // ---- Prompt Preview ----

      openPromptPreview: function (dept, m) {
        this.previewMember = m;
        this.previewDept = dept;
        var roles = (dept && Array.isArray(dept.roles) && dept.roles.length > 0) ? dept.roles : ['dev'];
        this.previewRole = roles[0];
        this.previewProject = '';
        this.previewCopied = false;
        this.promptPreviewModalOpen = true;
        this.fetchPromptPreview();
      },

      closePromptPreview: function () {
        this.promptPreviewModalOpen = false;
        this.previewMember = null;
        this.previewDept = null;
        this.previewPrompt = '';
        this.previewWarnings = [];
        this.previewError = '';
      },

      previewRolesList: function () {
        if (this.previewDept && Array.isArray(this.previewDept.roles) && this.previewDept.roles.length > 0) {
          return this.previewDept.roles;
        }
        return ['lead', 'dev', 'qa', 'plan-review', 'code-review', 'analyst', 'analysis-review'];
      },

      fetchPromptPreview: function () {
        if (!this.previewMember) return;
        var self = this;
        self.previewLoading = true;
        self.previewError = '';
        var url = '/api/prompts/preview?role=' + encodeURIComponent(self.previewRole) + '&member=' + encodeURIComponent(self.previewMember.id);
        if (self.previewProject) {
          url += '&project=' + encodeURIComponent(self.previewProject);
        }
        fetch(url, { credentials: 'same-origin' })
          .then(function (res) {
            return res.json().then(function (data) {
              return { ok: res.ok, status: res.status, data: data };
            });
          })
          .then(function (result) {
            self.previewLoading = false;
            if (result.ok && result.data) {
              self.previewPrompt = result.data.prompt || '';
              self.previewWarnings = Array.isArray(result.data.warnings) ? result.data.warnings : [];
            } else {
              self.previewError = (result.data && result.data.error) || 'Không thể xem trước prompt';
            }
          })
          .catch(function (err) {
            self.previewLoading = false;
            self.previewError = err.message || 'Lỗi mạng khi xem trước prompt';
          });
      },

      copyPromptPreview: function () {
        var self = this;
        if (!self.previewPrompt) return;
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(self.previewPrompt).then(function () {
            self.previewCopied = true;
            setTimeout(function () {
              self.previewCopied = false;
            }, 2000);
          });
        }
      },

      // ---- Department Rules Modal ----

      openDeptRulesModal: function (deptId, role) {
        this.rulesDeptId = deptId;
        this.rulesRole = role;
        this.rulesText = '';
        this.rulesDraft = '';
        this.rulesVersion = 'absent';
        this.rulesSource = 'builtin';
        this.rulesError = '';
        this.rulesSuccess = '';
        this.rulesModalOpen = true;
        this.loadDeptRule();
      },

      closeDeptRulesModal: function () {
        this.rulesModalOpen = false;
        this.rulesError = '';
        this.rulesSuccess = '';
      },

      loadDeptRule: function () {
        var self = this;
        self.rulesLoading = true;
        self.rulesError = '';
        var url = '/api/prompts/departments/' + encodeURIComponent(self.rulesDeptId) + '/' + encodeURIComponent(self.rulesRole);
        fetch(url, { credentials: 'same-origin' })
          .then(function (res) {
            return res.json().then(function (data) {
              return { ok: res.ok, status: res.status, data: data };
            });
          })
          .then(function (result) {
            self.rulesLoading = false;
            if (result.ok && result.data) {
              self.rulesText = result.data.text || '';
              self.rulesDraft = self.rulesText;
              self.rulesVersion = result.data.version || 'absent';
              self.rulesSource = result.data.source || 'builtin';
            } else {
              self.rulesError = (result.data && result.data.error) || 'Không thể tải rules của phòng';
            }
          })
          .catch(function (err) {
            self.rulesLoading = false;
            self.rulesError = err.message || 'Lỗi mạng khi tải rules';
          });
      },

      saveDeptRule: function () {
        var self = this;
        self.rulesSaving = true;
        self.rulesError = '';
        self.rulesSuccess = '';
        var url = '/api/prompts/departments/' + encodeURIComponent(self.rulesDeptId) + '/' + encodeURIComponent(self.rulesRole);
        fetch(url, {
          method: 'PUT',
          credentials: 'same-origin',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            text: self.rulesDraft,
            version: self.rulesVersion,
          }),
        })
          .then(function (res) {
            return res.json().then(function (data) {
              return { ok: res.ok, status: res.status, data: data };
            });
          })
          .then(function (result) {
            self.rulesSaving = false;
            if (result.ok && result.data) {
              self.rulesVersion = result.data.version || self.rulesVersion;
              self.rulesText = self.rulesDraft;
              self.rulesSource = 'user';
              self.rulesSuccess = 'Đã lưu rules phòng ban.';
              self.updateRoleMetaSource(self.rulesDeptId, self.rulesRole, 'user');
            } else if (result.status === 409) {
              self.rulesError = (result.data && result.data.error) || 'Ghi chú vừa bị sửa ở nơi khác — vui lòng tải lại.';
              self.loadDeptRule();
            } else {
              self.rulesError = (result.data && result.data.error) || 'Không thể lưu ghi chú';
            }
          })
          .catch(function (err) {
            self.rulesSaving = false;
            self.rulesError = err.message || 'Lỗi mạng khi lưu ghi chú';
          });
      },

      deleteDeptRule: function () {
        var self = this;
        self.rulesDeleting = true;
        self.rulesError = '';
        self.rulesSuccess = '';
        var url = '/api/prompts/departments/' + encodeURIComponent(self.rulesDeptId) + '/' + encodeURIComponent(self.rulesRole);
        fetch(url, {
          method: 'DELETE',
          credentials: 'same-origin',
        })
          .then(function (res) {
            return res.json().then(function (data) {
              return { ok: res.ok, status: res.status, data: data };
            });
          })
          .then(function (result) {
            self.rulesDeleting = false;
            if (result.ok && result.data) {
              self.rulesText = result.data.text || '';
              self.rulesDraft = self.rulesText;
              self.rulesVersion = result.data.version || 'absent';
              self.rulesSource = result.data.source || 'builtin';
              self.rulesSuccess = 'Đã khôi phục về bản có sẵn.';
              self.updateRoleMetaSource(self.rulesDeptId, self.rulesRole, self.rulesSource);
            } else {
              self.rulesError = (result.data && result.data.error) || 'Không thể khôi phục ghi chú';
            }
          })
          .catch(function (err) {
            self.rulesDeleting = false;
            self.rulesError = err.message || 'Lỗi mạng khi xóa ghi chú';
          });
      },

      updateRoleMetaSource: function (deptId, role, source) {
        if (!Array.isArray(this.departments)) return;
        var dept = this.departments.find(function (d) { return d.id === deptId; });
        if (dept && Array.isArray(dept.roles_meta)) {
          var rm = dept.roles_meta.find(function (r) { return r.role === role; });
          if (rm) {
            rm.source = source;
          }
        }
      },

      deptRoleBadgeText: function (rm) {
        if (!rm) return '';
        if (rm.source === 'builtin') return 'đang dùng bản có sẵn';
        if (rm.source === 'user') return 'đã tùy chỉnh';
        return 'chưa có';
      },

      deptRoleBadgeClass: function (rm) {
        if (!rm) return '';
        if (rm.source === 'builtin') {
          return 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300';
        }
        if (rm.source === 'user') {
          return 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-300';
        }
        return 'bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-300';
      },

      rulesByteCountText: function () {
        var str = this.rulesDraft || '';
        var bytes = new Blob([str]).size;
        return bytes + ' / 32768 bytes';
      },

      // ---- Helper booleans for Alpine bindings ----

      hasMemberRules: function (m) {
        return Boolean(m && m.rules);
      },

      hasMemberNotes: function (m) {
        return Boolean(m && m.notes);
      },

      hasDeptSkills: function (dept) {
        return Boolean(dept && Array.isArray(dept.skills) && dept.skills.length > 0);
      },

      hasDeptRoles: function (dept) {
        return Boolean(dept && Array.isArray(dept.roles_meta) && dept.roles_meta.length > 0);
      },

      isSavingMember: function () {
        return Boolean(this.saving);
      },

      isDeletingMember: function () {
        return Boolean(this.deleting);
      },

      isInitingExample: function () {
        return Boolean(this.initing);
      },

      isLoadingData: function () {
        return Boolean(this.loading);
      },

      isRulesSaving: function () {
        return Boolean(this.rulesSaving || this.rulesLoading);
      },

      isRulesDeleting: function () {
        return Boolean(this.rulesDeleting || this.rulesLoading);
      },

      isRulesUser: function () {
        return this.rulesSource === 'user';
      },

      isRulesBuiltin: function () {
        return this.rulesSource === 'builtin';
      },

      hasRulesError: function () {
        return Boolean(this.rulesError);
      },

      hasRulesSuccess: function () {
        return Boolean(this.rulesSuccess);
      },

      hasPromptPreviewWarnings: function () {
        return Array.isArray(this.previewWarnings) && this.previewWarnings.length > 0;
      },

      hasPromptPreviewError: function () {
        return Boolean(this.previewError);
      },

      hasPromptPreviewText: function () {
        return Boolean(this.previewPrompt);
      },

      isPromptPreviewLoading: function () {
        return Boolean(this.previewLoading);
      },
    };
  });
})();

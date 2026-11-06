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
        this.fieldErrors = {};
        this.modalError = '';
        this.modalOpen = true;
        this.fetchModels(m.engine);
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
    };
  });
})();

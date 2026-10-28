// js/settings.js — Cài đặt screen component (teams, workflows, staff, engines)
(function () {
  var SECTION_KEY = 'ab.settings.section';
  var SECTIONS = [
    { id: 'teams', label: 'Đội hình' },
    { id: 'workflows', label: 'Quy trình' },
    { id: 'staff', label: 'Nhân sự' },
    { id: 'engines', label: 'Engine' },
  ];

  var NAV_NORMAL = 'text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700';
  var NAV_ACTIVE = 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-200';
  var RING_OK = 'ring-1 ring-slate-500 dark:ring-slate-400';
  var RING_BAD = 'ring-2 ring-rose-600 dark:ring-rose-400';

  // Fixed stage order of the workflow editor (UI-SPEC § Quy trình).
  var STAGES = [
    { key: 'plan', label: 'Lên plan' },
    { key: 'plan-review', label: 'Review plan' },
    { key: 'gate:you', label: 'Bạn duyệt plan' },
    { key: 'implement', label: 'Code' },
    { key: 'code-review', label: 'Review code' },
    { key: 'qa', label: 'QA' },
    { key: 'fix', label: 'Sửa' },
  ];
  var LOOPS = [
    { key: 'plan', a: 'plan', b: 'plan-review', nameA: 'Lên plan', nameB: 'Review plan', label: 'Lên plan ⇄ Review plan' },
    { key: 'implement', a: 'implement', b: 'code-review', nameA: 'Code', nameB: 'Review code', label: 'Code ⇄ Review code' },
    { key: 'qa', a: 'qa', b: 'fix', nameA: 'QA', nameB: 'Sửa', label: 'QA ⇄ Sửa' },
  ];

  // Per-kind copy of the Engine section (UI-SPEC Delta 2026-10-28, D-44). The list of engines
  // itself comes from GET /api/engines; nothing here names a configured engine.
  var KIND_CHIP = {
    'claude': 'có sẵn',
    'người': 'có sẵn',
    'anthropic': 'anthropic',
    'agy': 'agy',
    'dsh': 'dsh',
  };
  var KIND_DESC = {
    'claude': 'Dùng profile của dự án (work hoặc personal), không có tài khoản riêng.',
    'người': 'Nhân sự làm trên máy của họ và đẩy nhánh task lên remote.',
    'agy': 'Antigravity chạy không giao diện (headless).',
    'dsh': 'DeepSeek Harness trong container, chạy như hộp đen: chỉ văn bản, không đếm token.',
  };
  var DESC_ANTHROPIC_ENDPOINT = 'claude chạy trên máy với một endpoint định dạng Anthropic.';
  var DESC_ANTHROPIC_NATIVE = 'claude chạy trên máy bằng tài khoản Claude của profile dự án.';
  var HUMAN_ENGINE = 'người';
  var NO_TOKEN_LINE = 'Không đếm token (chỉ văn bản).';
  var READINESS_WORDS = {
    san_sang: 'sẵn sàng',
    thieu_key: 'thiếu key',
    thieu_lenh: 'thiếu lệnh',
    chua_kiem: 'chưa kiểm',
  };

  // Per-kind constants. `pfx` prefixes the editor state fields (teamMode, wMode, sMode ...).
  var KIND = {
    team: {
      pfx: 'team',
      file: 'teams.json',
      noun: 'đội hình',
      heading: 'Không lưu được đội hình',
      conflict: 'teams.json vừa bị sửa ở nơi khác',
      savePath: '/api/teams/save',
      delPath: '/api/teams/delete',
      saveLabel: 'Lưu đội hình',
      delLabel: 'Xóa đội hình',
      keepLabel: 'Giữ đội hình',
      maxName: 40,
    },
    workflow: {
      pfx: 'w',
      file: 'workflows.json',
      noun: 'quy trình',
      heading: 'Không lưu được quy trình',
      conflict: 'workflows.json vừa bị sửa ở nơi khác',
      savePath: '/api/workflows/save',
      delPath: '/api/workflows/delete',
      saveLabel: 'Lưu quy trình',
      delLabel: 'Xóa quy trình',
      keepLabel: 'Giữ quy trình',
      maxName: 40,
    },
    staff: {
      pfx: 's',
      file: 'staff.json',
      noun: 'nhân sự',
      heading: 'Không lưu được nhân sự',
      conflict: 'file nhân sự vừa bị sửa ở nơi khác',
      savePath: '/api/staff/save',
      delPath: '/api/staff/delete',
      saveLabel: 'Lưu nhân sự',
      delLabel: 'Xóa nhân sự',
      keepLabel: 'Giữ nhân sự',
      maxName: 32,
    },
  };

  var FIELD_IDS = {
    team: { name: 'ab-set-team-name', description: 'ab-set-team-desc', qa: 'ab-set-team-qa' },
    workflow: {
      name: 'ab-set-wf-name',
      stages: 'ab-set-wf-stage-0',
      max_rounds: 'ab-set-wf-rounds',
      budget_minutes: 'ab-set-wf-minutes',
      budget_usd: 'ab-set-wf-usd',
    },
    staff: { id: 'ab-set-staff-id', display_name: 'ab-set-staff-name' },
  };

  var NAME_FORMAT = 'Tên chỉ gồm chữ thường không dấu, số và dấu gạch ngang (ví dụ tiet-kiem).';
  var STAFF_FORMAT = 'Mã chỉ gồm chữ thường không dấu, số và dấu gạch ngang.';
  var CONFLICT_BODY = 'Đã tải lại danh sách. Bản nháp của bạn vẫn còn trong form — kiểm tra rồi lưu lại.';

  function validSection(name) {
    for (var i = 0; i < SECTIONS.length; i++) {
      if (SECTIONS[i].id === name) return true;
    }
    return false;
  }

  function slugOk(name, max) {
    return typeof name === 'string' && name.length > 0 && name.length <= max && /^[a-z0-9][a-z0-9-]*$/.test(name);
  }

  function isInt(v) {
    return /^[0-9]+$/.test(String(v === null || v === undefined ? '' : v).trim());
  }

  function cap10(lines) {
    var out = (lines || []).slice(0, 10);
    if ((lines || []).length > 10) {
      out.push('… và ' + (lines.length - 10) + ' lỗi khác');
    }
    return out;
  }

  function errorLines(data, res) {
    var lines = [];
    if (data && Array.isArray(data.errors)) {
      for (var i = 0; i < data.errors.length; i++) {
        var e = data.errors[i];
        if (e && e.message) lines.push(e.message);
      }
    }
    if (lines.length === 0) {
      lines.push(window.AB.errorText(res));
    }
    return lines;
  }

  function engineWord(value, staffList) {
    var v = String(value || '');
    if (v.indexOf('người:') === 0) {
      var id = v.slice('người:'.length);
      for (var i = 0; i < (staffList || []).length; i++) {
        if (staffList[i].id === id) return 'người ' + staffList[i].display_name;
      }
      return 'người ' + id;
    }
    return v;
  }

  function selectLater(id) {
    setTimeout(function () {
      var el = document.getElementById(id);
      if (el && typeof el.select === 'function') el.select();
    }, 120);
  }

  window.AB.onReady('settings', function () {
    return {
      section: 'teams',
      sections: SECTIONS,
      loaded: false,
      loadError: '',
      loadFail: '',
      stale: false,
      reloading: false,
      saving: false,
      statusText: '',

      teams: [],
      teamErrors: [],
      teamsVersion: '',
      builtinTeams: [],
      workflows: [],
      workflowErrors: [],
      workflowsVersion: '',
      staff: [],
      staffErrors: [],
      staffVersion: '',
      engines: [],
      enginesChecking: false,
      enginesFailed: false,

      stages: STAGES,
      loops: LOOPS,

      // Team editor (flat fields: Alpine CSP, no x-model on deep paths).
      teamMode: '',
      teamEditing: '',
      tName: '',
      tDesc: '',
      tLead: 'claude',
      tDev: 'claude',
      tQa: 'claude',
      tPlanReview: 'claude',
      tCodeReview: 'claude',
      teamFieldErrors: {},
      teamRefusal: [],
      teamConflict: false,
      teamTried: false,
      teamTouched: {},

      // Workflow editor.
      wMode: '',
      wEditing: '',
      wName: '',
      wStageOn: {},
      wLoopOn: {},
      wAuto: false,
      wMaxRounds: '3',
      wBudgetMinutes: '60',
      wBudgetUsd: '20',
      wFieldErrors: {},
      wRefusal: [],
      wConflict: false,
      wTried: false,
      wTouched: {},

      // Staff editor.
      sMode: '',
      sEditing: '',
      sId: '',
      sName: '',
      sNotes: '',
      sFieldErrors: {},
      sRefusal: [],
      sConflict: false,
      sTried: false,
      sTouched: {},

      // Delete dialogs (one open at most).
      delKind: '',
      delName: '',
      delLabel: '',
      delBusy: false,
      delError: '',

      init: function () {
        var stored = '';
        try {
          stored = sessionStorage.getItem(SECTION_KEY) || '';
        } catch (e) {}
        if (validSection(stored)) {
          this.section = stored;
        }
        window.AB.onLeave(function () {
          window.AB.lockScroll(false);
        });
        this.resetWorkflowDraft();
        this.loadEngines();
        this.load();
      },

      // ---- loading ----

      loadTeams: function () {
        var self = this;
        return window.AB.api('GET', '/api/teams').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.teams)) {
            self.applyTeams(res.data);
            return true;
          }
          self.loadFail = window.AB.errorText(res);
          return false;
        });
      },

      loadWorkflows: function () {
        var self = this;
        return window.AB.api('GET', '/api/workflows').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.workflows)) {
            self.applyWorkflows(res.data);
            return true;
          }
          self.loadFail = window.AB.errorText(res);
          return false;
        });
      },

      loadStaff: function () {
        var self = this;
        return window.AB.api('GET', '/api/staff').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.staff)) {
            self.applyStaff(res.data);
            return true;
          }
          self.loadFail = window.AB.errorText(res);
          return false;
        });
      },

      // Engines never block the screen: a failure keeps the rows in `chưa kiểm`.
      loadEngines: function () {
        var self = this;
        return window.AB.api('GET', '/api/engines').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.engines)) {
            self.engines = res.data.engines;
            self.enginesFailed = false;
            return true;
          }
          self.enginesFailed = true;
          return false;
        });
      },

      // Teams, workflows and staff. Before the first full answer a failure shows the
      // load-error card; afterwards the previous data stays and a muted line shows.
      load: function () {
        var self = this;
        return Promise.all([this.loadTeams(), this.loadWorkflows(), this.loadStaff()]).then(function (r) {
          if (r[0] && r[1] && r[2]) {
            self.loaded = true;
            self.stale = false;
            self.loadError = '';
          } else if (!self.loaded) {
            self.loadError = self.loadFail;
          } else {
            self.stale = true;
          }
        });
      },

      applyTeams: function (data) {
        this.teams = data.teams || [];
        this.teamErrors = data.errors || [];
        this.teamsVersion = data.version || '';
        this.builtinTeams = data.builtin_names || [];
      },

      applyWorkflows: function (data) {
        this.workflows = data.workflows || [];
        this.workflowErrors = data.errors || [];
        this.workflowsVersion = data.version || '';
      },

      applyStaff: function (data) {
        this.staff = data.staff || [];
        this.staffErrors = data.errors || [];
        this.staffVersion = data.version || '';
        if (window.AB.setStaffNames) {
          var names = {};
          for (var i = 0; i < this.staff.length; i++) {
            names[this.staff[i].id] = this.staff[i].display_name;
          }
          window.AB.setStaffNames(names);
        }
      },

      reload: function () {
        var self = this;
        if (this.reloading) return Promise.resolve();
        this.reloading = true;
        this.statusText = '';
        return this.load().then(function () {
          self.reloading = false;
        });
      },

      retry: function () {
        this.loadError = '';
        return this.load();
      },

      checkEngines: function () {
        var self = this;
        if (this.enginesChecking) return Promise.resolve();
        this.enginesChecking = true;
        return this.loadEngines().then(function () {
          self.enginesChecking = false;
        });
      },

      // ---- sub-nav ----

      showSection: function (name) {
        if (!validSection(name)) return;
        this.section = name;
        try {
          sessionStorage.setItem(SECTION_KEY, name);
        } catch (e) {}
      },

      pickSection: function (s) {
        this.showSection(s.id);
      },

      navClass: function (s) {
        return this.section === s.id ? NAV_ACTIVE : NAV_NORMAL;
      },

      navCurrent: function (s) {
        return this.section === s.id ? 'true' : false;
      },

      get showTeams() {
        return this.loaded && this.section === 'teams';
      },

      get showWorkflows() {
        return this.loaded && this.section === 'workflows';
      },

      get showStaff() {
        return this.loaded && this.section === 'staff';
      },

      get showEngines() {
        return this.loaded && this.section === 'engines';
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
        return this.loadError + '. Bấm Thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get reloadLabel() {
        return this.reloading ? 'Đang đọc...' : 'Đọc lại';
      },

      get checkLabel() {
        return this.enginesChecking ? 'Đang kiểm tra...' : 'Kiểm tra lại';
      },

      // ---- generic helpers over the three kinds ----

      modeOf: function (kind) {
        return this[KIND[kind].pfx + 'Mode'];
      },

      editingOf: function (kind) {
        return this[KIND[kind].pfx + 'Editing'];
      },

      listOf: function (kind) {
        if (kind === 'team') return this.teams;
        if (kind === 'workflow') return this.workflows;
        return this.staff;
      },

      keyOf: function (kind, entry) {
        return kind === 'staff' ? entry.id : entry.name;
      },

      findEntry: function (kind, key) {
        var list = this.listOf(kind) || [];
        for (var i = 0; i < list.length; i++) {
          if (this.keyOf(kind, list[i]) === key) return list[i];
        }
        return null;
      },

      versionOf: function (kind) {
        if (kind === 'team') return this.teamsVersion;
        if (kind === 'workflow') return this.workflowsVersion;
        return this.staffVersion;
      },

      errorsOf: function (kind) {
        if (kind === 'team') return this.teamErrors;
        if (kind === 'workflow') return this.workflowErrors;
        return this.staffErrors;
      },

      applyList: function (kind, data) {
        if (kind === 'team') this.applyTeams(data);
        else if (kind === 'workflow') this.applyWorkflows(data);
        else this.applyStaff(data);
      },

      reloadKind: function (kind) {
        if (kind === 'team') return this.loadTeams();
        if (kind === 'workflow') return this.loadWorkflows();
        return this.loadStaff();
      },

      isEditorOpen: function (kind) {
        return this.modeOf(kind) !== '';
      },

      isEditMode: function (kind) {
        return this.modeOf(kind) === 'edit';
      },

      get teamEditorOpen() {
        return this.teamMode !== '';
      },

      get workflowEditorOpen() {
        return this.wMode !== '';
      },

      get staffEditorOpen() {
        return this.sMode !== '';
      },

      get writeLocked() {
        return this.saving || this.delBusy;
      },

      invalidFile: function (kind) {
        return (this.errorsOf(kind) || []).length > 0;
      },

      fileReason: function (kind) {
        return KIND[kind].file + ' đang có lỗi — sửa file trong editor rồi bấm Đọc lại; dashboard không ghi đè file lỗi.';
      },

      fileErrorLines: function (kind) {
        return cap10(this.errorsOf(kind));
      },

      // Creation button and row actions of a section.
      actionsLocked: function (kind) {
        return this.writeLocked || this.isEditorOpen(kind) || this.invalidFile(kind);
      },

      lockLine: function (kind) {
        if (!this.isEditorOpen(kind)) return '';
        var name = this.draftName(kind);
        return 'Đang sửa ' + (name || 'mục mới') + ' — lưu hoặc hủy trước khi sửa mục khác.';
      },

      draftName: function (kind) {
        if (kind === 'team') return this.tName;
        if (kind === 'workflow') return this.wName;
        return this.sId;
      },

      newId: function (kind) {
        return 'ab-set-' + kind + '-new';
      },

      editId: function (kind, key) {
        return 'ab-set-' + kind + '-edit-' + key;
      },

      copyId: function (kind, key) {
        return 'ab-set-' + kind + '-copy-' + key;
      },

      delId: function (kind, key) {
        return 'ab-set-' + kind + '-del-' + key;
      },

      // First free `{name}-ban-sao`, `{name}-ban-sao-2`, ...
      copyName: function (kind, name) {
        var base = name + '-ban-sao';
        var candidate = base;
        var n = 2;
        while (this.findEntry(kind, candidate)) {
          candidate = base + '-' + n;
          n += 1;
        }
        return candidate;
      },

      clearDraftMeta: function (kind) {
        var p = KIND[kind].pfx;
        this[p + 'FieldErrors'] = {};
        this[p + 'Refusal'] = [];
        this[p + 'Conflict'] = false;
        this[p + 'Tried'] = false;
        this[p + 'Touched'] = {};
      },

      closeEditor: function (kind) {
        var p = KIND[kind].pfx;
        this[p + 'Mode'] = '';
        this[p + 'Editing'] = '';
        this.clearDraftMeta(kind);
      },

      focusField: function (kind, field) {
        var ids = FIELD_IDS[kind] || {};
        var id = ids[field];
        if (id) window.AB.focusLater(id);
      },

      // ---- inline validation (client mirror; the server answer wins) ----

      // Errors in the order of the disabled reasons: [{field, message}].
      clientErrors: function (kind) {
        var out = [];
        var n = 0;
        var name = '';
        if (kind === 'team') {
          name = this.tName;
          if (!this.isEditMode('team')) {
            if (name.length === 0) {
              out.push({ field: 'name', message: 'Nhập tên đội hình.' });
            } else if (!slugOk(name, 40)) {
              out.push({ field: 'name', message: NAME_FORMAT });
            } else if (this.isBuiltinTeam(name)) {
              out.push({ field: 'name', message: "'" + name + "' là đội hình có sẵn, không ghi đè được. Đặt tên khác." });
            } else if (this.findEntry('team', name)) {
              out.push({ field: 'name', message: "Đã có đội hình tên '" + name + "'." });
            }
          }
          if (this.tDesc.trim().length === 0) {
            out.push({ field: 'description', message: 'Nhập mô tả ngắn để chọn đội hình cho đúng.' });
          }
          if (this.tQa !== 'claude') {
            out.push({ field: 'qa', message: 'QA phải dùng engine claude.' });
          }
        } else if (kind === 'workflow') {
          name = this.wName;
          if (!this.isEditMode('workflow')) {
            if (name.length === 0) {
              out.push({ field: 'name', message: 'Nhập tên quy trình.' });
            } else if (!slugOk(name, 40)) {
              out.push({ field: 'name', message: NAME_FORMAT });
            } else if (this.isBuiltinWorkflow(name)) {
              out.push({ field: 'name', message: "'" + name + "' là quy trình có sẵn, không ghi đè được. Đặt tên khác." });
            } else if (this.findEntry('workflow', name)) {
              out.push({ field: 'name', message: "Đã có quy trình tên '" + name + "'." });
            }
          }
          for (var i = 0; i < STAGES.length; i++) {
            if (this.wStageOn[STAGES[i].key]) n += 1;
          }
          if (n === 0) {
            out.push({ field: 'stages', message: 'Chọn ít nhất một bước.' });
          }
          if (this.anyLoopOn) {
            var r = String(this.wMaxRounds).trim();
            if (!isInt(r) || Number(r) < 1 || Number(r) > 10) {
              out.push({ field: 'max_rounds', message: 'Số vòng phải là số nguyên từ 1 đến 10.' });
            }
          }
          var m = String(this.wBudgetMinutes).trim();
          if (!isInt(m) || Number(m) < 1) {
            out.push({ field: 'budget_minutes', message: 'Ngân sách thời gian phải là số nguyên dương.' });
          }
          var u = String(this.wBudgetUsd).trim();
          if (u.length === 0 || !isFinite(Number(u)) || Number(u) <= 0) {
            out.push({ field: 'budget_usd', message: 'Ngân sách chi phí phải là số dương.' });
          }
        } else {
          name = this.sId;
          if (!this.isEditMode('staff')) {
            if (name.length === 0) {
              out.push({ field: 'id', message: 'Nhập mã nhân sự.' });
            } else if (!slugOk(name, 32)) {
              out.push({ field: 'id', message: STAFF_FORMAT });
            } else if (this.findEntry('staff', name)) {
              out.push({ field: 'id', message: "Đã có nhân sự mã '" + name + "'." });
            }
          }
          if (this.sName.trim().length === 0) {
            out.push({ field: 'display_name', message: 'Nhập tên hiển thị.' });
          }
        }
        return out;
      },

      isBuiltinTeam: function (name) {
        return (this.builtinTeams || []).indexOf(name) >= 0;
      },

      isBuiltinWorkflow: function (name) {
        var w = this.findEntry('workflow', name);
        return Boolean(w && w.source === 'builtin');
      },

      // Field message: the server's first, then the client's once the field was
      // touched (or a submit failed); the QA rule shows at once (D-06).
      fieldMsg: function (kind, field) {
        var p = KIND[kind].pfx;
        var server = this[p + 'FieldErrors'][field];
        if (server) return server;
        var shown = this[p + 'Tried'] || this[p + 'Touched'][field] || field === 'qa';
        if (!shown) return '';
        var errs = this.clientErrors(kind);
        for (var i = 0; i < errs.length; i++) {
          if (errs[i].field === field) return errs[i].message;
        }
        return '';
      },

      fieldInvalid: function (kind, field) {
        return this.fieldMsg(kind, field) !== '';
      },

      invalidAttr: function (kind, field) {
        return this.fieldInvalid(kind, field) ? 'true' : false;
      },

      fieldClass: function (kind, field) {
        return this.fieldInvalid(kind, field) ? RING_BAD : RING_OK;
      },

      describedBy: function (kind, field, base) {
        return this.fieldInvalid(kind, field) ? base + '-help ' + base + '-err' : base + '-help';
      },

      onField: function (kind, field) {
        var p = KIND[kind].pfx;
        var touched = Object.assign({}, this[p + 'Touched']);
        touched[field] = true;
        this[p + 'Touched'] = touched;
        if (this[p + 'FieldErrors'][field]) {
          var errs = Object.assign({}, this[p + 'FieldErrors']);
          delete errs[field];
          this[p + 'FieldErrors'] = errs;
        }
      },

      // The reason shown under the save button: the first failing rule.
      saveReason: function (kind) {
        if (this.invalidFile(kind)) return this.fileReason(kind);
        var errs = this.clientErrors(kind);
        return errs.length > 0 ? errs[0].message : '';
      },

      saveDisabled: function (kind) {
        return this.writeLocked || this.saveReason(kind) !== '';
      },

      saveText: function (kind) {
        return this.saving ? 'Đang lưu...' : KIND[kind].saveLabel;
      },

      editorTitle: function (kind) {
        if (this.isEditMode(kind)) {
          return 'Sửa ' + KIND[kind].noun + ' ' + this.draftName(kind);
        }
        return 'Tạo ' + KIND[kind].noun + ' mới';
      },

      readonlyAttr: function (kind) {
        return this.isEditMode(kind) ? 'true' : false;
      },

      // Helper under the name field: the slug rule, or why a saved name cannot change.
      nameHelp: function (kind) {
        if (this.isEditMode(kind)) {
          return 'Không đổi tên được vì dự án và task tham chiếu theo tên. Muốn tên khác: bấm Nhân bản.';
        }
        return 'Chữ thường không dấu, số và dấu gạch ngang, ví dụ tiet-kiem';
      },

      cancelText: function (kind) {
        return this.isEditMode(kind) ? 'Hủy sửa' : 'Hủy tạo';
      },

      // A non-blocking note when the same name exists in the other list (D-11).
      collisionNote: function (kind) {
        if (this.isEditMode(kind)) return '';
        if (kind === 'team') {
          if (slugOk(this.tName, 40) && this.findEntry('workflow', this.tName)) {
            return 'Tên này cũng là một quy trình. Hai danh sách riêng nên vẫn lưu được; nhớ phân biệt đội hình và quy trình.';
          }
        } else if (kind === 'workflow') {
          if (slugOk(this.wName, 40) && this.findEntry('team', this.wName)) {
            return 'Tên này cũng là một đội hình. Hai danh sách riêng nên vẫn lưu được; nhớ phân biệt đội hình và quy trình.';
          }
        }
        return '';
      },

      // ---- refusal and conflict callouts ----

      refusalHeading: function (kind) {
        return KIND[kind].heading;
      },

      refusalLines: function (kind) {
        return cap10(this[KIND[kind].pfx + 'Refusal']);
      },

      hasRefusal: function (kind) {
        return this[KIND[kind].pfx + 'Refusal'].length > 0;
      },

      conflictHeading: function (kind) {
        return KIND[kind].conflict;
      },

      hasConflict: function (kind) {
        return this[KIND[kind].pfx + 'Conflict'];
      },

      conflictBody: function () {
        return CONFLICT_BODY;
      },

      refusalId: function (kind) {
        return 'ab-set-' + kind + '-refusal';
      },

      conflictId: function (kind) {
        return 'ab-set-' + kind + '-conflict';
      },

      // ---- team editor ----

      newTeam: function () {
        if (this.actionsLocked('team')) return;
        this.statusText = '';
        this.clearDraftMeta('team');
        this.teamMode = 'create';
        this.teamEditing = '';
        this.tName = '';
        this.tDesc = '';
        this.tLead = 'claude';
        this.tDev = 'claude';
        this.tQa = 'claude';
        this.tPlanReview = 'claude';
        this.tCodeReview = 'claude';
        window.AB.focusLater(FIELD_IDS.team.name);
      },

      fillTeam: function (t) {
        var r = t.roles || {};
        this.tDesc = t.description || '';
        this.tLead = r.lead || 'claude';
        this.tDev = r.dev || 'claude';
        this.tQa = r.qa || 'claude';
        this.tPlanReview = r.plan_review || 'claude';
        this.tCodeReview = r.code_review || 'claude';
      },

      editTeam: function (name) {
        if (this.actionsLocked('team')) return;
        var t = this.findEntry('team', name);
        if (!t || t.source === 'builtin') return;
        this.statusText = '';
        this.clearDraftMeta('team');
        this.teamMode = 'edit';
        this.teamEditing = name;
        this.tName = name;
        this.fillTeam(t);
        window.AB.focusLater(FIELD_IDS.team.description);
      },

      copyTeam: function (name) {
        if (this.actionsLocked('team')) return;
        var t = this.findEntry('team', name);
        if (!t) return;
        this.statusText = '';
        this.clearDraftMeta('team');
        this.teamMode = 'create';
        this.teamEditing = '';
        this.tName = this.copyName('team', name);
        this.fillTeam(t);
        window.AB.focusLater(FIELD_IDS.team.name);
        selectLater(FIELD_IDS.team.name);
      },

      cancelTeam: function () {
        var edited = this.teamEditing;
        this.closeEditor('team');
        window.AB.focusLater(edited ? this.editId('team', edited) : this.newId('team'), this.newId('team'));
      },

      saveTeam: function () {
        return this.saveEntry('team');
      },

      roleValue: function (role) {
        if (role === 'lead') return this.tLead;
        if (role === 'dev') return this.tDev;
        if (role === 'qa') return this.tQa;
        if (role === 'plan_review') return this.tPlanReview;
        return this.tCodeReview;
      },

      engineStatus: function (name) {
        var list = this.engines || [];
        for (var i = 0; i < list.length; i++) {
          if (list[i] && list[i].name === name) return list[i];
        }
        return null;
      },

      engineOptionLabel: function (name) {
        var st = this.engineStatus(name);
        if (st && st.readiness !== 'san_sang') {
          return name + ' — ' + (st.word || READINESS_WORDS[st.readiness] || '');
        }
        return name;
      },

      // Options of one role select. Human options only in the Dev select (D-24); a value
      // already on the draft that is not offered stays visible so it is never changed silently.
      roleOptions: function (role) {
        var opts = [];
        var seen = {};
        var i;
        // `claude` is always first (and the only QA engine); the configured ids follow in API order.
        opts.push({ value: 'claude', label: this.engineOptionLabel('claude') });
        seen['claude'] = true;
        var list = this.engines || [];
        for (i = 0; i < list.length; i++) {
          var e = list[i];
          if (!e || !e.name || e.name === HUMAN_ENGINE || seen[e.name]) continue;
          // D-37: a dsh engine is offered only in the Dev select.
          if (e.kind === 'dsh' && role !== 'dev') continue;
          opts.push({ value: e.name, label: this.engineOptionLabel(e.name) });
          seen[e.name] = true;
        }
        if (role === 'dev') {
          for (i = 0; i < this.staff.length; i++) {
            var v = 'người:' + this.staff[i].id;
            opts.push({ value: v, label: 'người ' + this.staff[i].display_name });
            seen[v] = true;
          }
        }
        var cur = this.roleValue(role);
        if (cur && !seen[cur]) {
          opts.push({ value: cur, label: engineWord(cur, this.staff) });
        }
        return opts;
      },

      optionSelected: function (role, value) {
        return this.roleValue(role) === value;
      },

      get noStaffYet() {
        return this.staff.length === 0;
      },

      // ---- team rows ----

      teamWarnings: function (t) {
        var lines = [];
        var seen = {};
        var roles = t.roles || {};
        var keys = ['lead', 'dev', 'qa', 'plan_review', 'code_review'];
        for (var i = 0; i < keys.length; i++) {
          var eng = roles[keys[i]];
          if (!eng || eng.indexOf('người:') === 0 || seen[eng]) continue;
          seen[eng] = true;
          var st = this.engineStatus(eng);
          if (st && st.readiness !== 'san_sang') {
            lines.push(eng + ': ' + (st.word || READINESS_WORDS[st.readiness] || ''));
          }
        }
        var w = t.warnings || [];
        for (var j = 0; j < w.length; j++) {
          lines.push(w[j]);
        }
        return lines;
      },

      teamChips: function (t) {
        if (!t) return [];
        if (t.source === 'builtin') return ['có sẵn'];
        if (t.source === 'override') return ['ghi đè đội hình có sẵn'];
        return ['từ teams.json'];
      },

      get teamRows() {
        var self = this;
        return (this.teams || []).map(function (t) {
          var builtin = t.source === 'builtin';
          return {
            name: t.name,
            description: t.description || '',
            summary: t.summary || '',
            chips: self.teamChips(t),
            warnings: self.teamWarnings(t),
            canEdit: !builtin,
            visible: !(self.teamMode === 'edit' && self.teamEditing === t.name),
            editId: self.editId('team', t.name),
            copyId: self.copyId('team', t.name),
            delId: self.delId('team', t.name),
            editLabel: 'Sửa đội hình ' + t.name,
            copyLabel: 'Nhân bản đội hình ' + t.name,
            delLabel: 'Xóa đội hình ' + t.name,
          };
        });
      },

      get hasCustomTeam() {
        for (var i = 0; i < this.teams.length; i++) {
          if (this.teams[i].source !== 'builtin') return true;
        }
        return false;
      },

      get noCustomTeam() {
        return this.teams.length > 0 && !this.hasCustomTeam;
      },

      get teamFileClosing() {
        return 'Đang dùng các đội hình có sẵn (' + (this.builtinTeams || []).join(', ') + ') cho đến khi bạn sửa xong file.';
      },

      // ---- workflow editor ----

      resetWorkflowDraft: function () {
        var on = {};
        on['plan'] = true;
        on['gate:you'] = true;
        on['implement'] = true;
        on['qa'] = true;
        this.wStageOn = on;
        this.wLoopOn = {};
        this.wAuto = false;
        this.wMaxRounds = '3';
        this.wBudgetMinutes = '60';
        this.wBudgetUsd = '20';
      },

      fillWorkflow: function (w) {
        var edit = w.edit || {};
        var on = {};
        var stages = edit.stages || w.stages || [];
        for (var i = 0; i < stages.length; i++) {
          on[stages[i]] = true;
        }
        var loopOn = {};
        var loops = edit.loops || {};
        for (var j = 0; j < LOOPS.length; j++) {
          if (loops[LOOPS[j].key]) loopOn[LOOPS[j].key] = true;
        }
        this.wStageOn = on;
        this.wLoopOn = loopOn;
        this.wAuto = Boolean(edit.auto !== undefined ? edit.auto : w.auto);
        this.wMaxRounds = String(edit.max_rounds !== undefined ? edit.max_rounds : (w.max_rounds || 3));
        this.wBudgetMinutes = String(edit.budget_minutes !== undefined ? edit.budget_minutes : (w.budget_minutes || 60));
        this.wBudgetUsd = String(edit.budget_usd !== undefined ? edit.budget_usd : (w.budget_usd || 20));
      },

      newWorkflow: function () {
        if (this.actionsLocked('workflow')) return;
        this.statusText = '';
        this.clearDraftMeta('workflow');
        this.wMode = 'create';
        this.wEditing = '';
        this.wName = '';
        this.resetWorkflowDraft();
        window.AB.focusLater(FIELD_IDS.workflow.name);
      },

      editWorkflow: function (name) {
        if (this.actionsLocked('workflow')) return;
        var w = this.findEntry('workflow', name);
        if (!w || w.source === 'builtin') return;
        this.statusText = '';
        this.clearDraftMeta('workflow');
        this.wMode = 'edit';
        this.wEditing = name;
        this.wName = name;
        this.fillWorkflow(w);
        window.AB.focusLater(FIELD_IDS.workflow.stages);
      },

      copyWorkflow: function (name) {
        if (this.actionsLocked('workflow')) return;
        var w = this.findEntry('workflow', name);
        if (!w) return;
        this.statusText = '';
        this.clearDraftMeta('workflow');
        this.wMode = 'create';
        this.wEditing = '';
        this.wName = this.copyName('workflow', name);
        this.fillWorkflow(w);
        window.AB.focusLater(FIELD_IDS.workflow.name);
        selectLater(FIELD_IDS.workflow.name);
      },

      cancelWorkflow: function () {
        var edited = this.wEditing;
        this.closeEditor('workflow');
        window.AB.focusLater(edited ? this.editId('workflow', edited) : this.newId('workflow'), this.newId('workflow'));
      },

      saveWorkflow: function () {
        return this.saveEntry('workflow');
      },

      stageInputId: function (index) {
        return 'ab-set-wf-stage-' + index;
      },

      stageChecked: function (key) {
        return Boolean(this.wStageOn[key]);
      },

      toggleStage: function (key) {
        var on = Object.assign({}, this.wStageOn);
        if (on[key]) {
          delete on[key];
          // Unchecking a stage unchecks the loops that need it.
          var loopOn = Object.assign({}, this.wLoopOn);
          for (var i = 0; i < LOOPS.length; i++) {
            if (LOOPS[i].a === key || LOOPS[i].b === key) delete loopOn[LOOPS[i].key];
          }
          this.wLoopOn = loopOn;
        } else {
          on[key] = true;
        }
        this.wStageOn = on;
        this.onField('workflow', 'stages');
      },

      loopEnabled: function (loop) {
        return Boolean(this.wStageOn[loop.a] && this.wStageOn[loop.b]);
      },

      loopDisabled: function (loop) {
        return !this.loopEnabled(loop);
      },

      loopChecked: function (loop) {
        return Boolean(this.wLoopOn[loop.key]) && this.loopEnabled(loop);
      },

      loopReason: function (loop) {
        if (this.loopEnabled(loop)) return '';
        return 'Cần chọn cả ' + loop.nameA + ' và ' + loop.nameB + ' ở mục Các bước.';
      },

      loopRowClass: function (loop) {
        return this.loopEnabled(loop) ? 'cursor-pointer' : 'cursor-not-allowed';
      },

      toggleLoop: function (loop) {
        if (!this.loopEnabled(loop)) return;
        var loopOn = Object.assign({}, this.wLoopOn);
        if (loopOn[loop.key]) delete loopOn[loop.key];
        else loopOn[loop.key] = true;
        this.wLoopOn = loopOn;
      },

      loopInputId: function (loop) {
        return 'ab-set-wf-loop-' + loop.key;
      },

      loopHelpId: function (loop) {
        return 'ab-set-wf-loop-' + loop.key + '-help';
      },

      toggleAuto: function () {
        this.wAuto = !this.wAuto;
      },

      get anyLoopOn() {
        for (var i = 0; i < LOOPS.length; i++) {
          if (this.loopChecked(LOOPS[i])) return true;
        }
        return false;
      },

      get roundsDisabled() {
        return !this.anyLoopOn;
      },

      get roundsReason() {
        return this.anyLoopOn ? '' : 'Chưa bật vòng lặp nào.';
      },

      get roundsDescribedBy() {
        if (this.fieldInvalid('workflow', 'max_rounds')) return 'ab-set-wf-rounds-help ab-set-wf-rounds-err';
        return 'ab-set-wf-rounds-help';
      },

      // ---- workflow rows ----

      workflowChips: function (w) {
        if (!w) return [];
        var chips = [];
        if (w.source === 'builtin') chips.push('có sẵn');
        else if (w.source === 'override') chips.push('ghi đè quy trình có sẵn');
        else chips.push('từ workflows.json');
        chips.push(w.auto ? 'autopilot' : 'chạy tay');
        return chips;
      },

      get workflowRows() {
        var self = this;
        return (this.workflows || []).map(function (w) {
          var builtin = w.source === 'builtin';
          var defaults = (w.default_for && w.default_for.length > 0) ? 'Mặc định của: ' + w.default_for.join(', ') : '';
          return {
            name: w.name,
            chips: self.workflowChips(w),
            summary: w.summary || '',
            limits: w.limits || '',
            defaultFor: defaults,
            canEdit: !builtin,
            visible: !(self.wMode === 'edit' && self.wEditing === w.name),
            editId: self.editId('workflow', w.name),
            copyId: self.copyId('workflow', w.name),
            delId: self.delId('workflow', w.name),
            editLabel: 'Sửa quy trình ' + w.name,
            copyLabel: 'Nhân bản quy trình ' + w.name,
            delLabel: 'Xóa quy trình ' + w.name,
          };
        });
      },

      get noCustomWorkflow() {
        if (this.workflows.length === 0) return false;
        for (var i = 0; i < this.workflows.length; i++) {
          if (this.workflows[i].source !== 'builtin') return false;
        }
        return true;
      },

      get workflowFileClosing() {
        return 'Đang dùng 2 quy trình có sẵn (mac-dinh, nhanh) cho đến khi bạn sửa xong file.';
      },

      // ---- staff editor ----

      newStaff: function () {
        if (this.actionsLocked('staff')) return;
        this.statusText = '';
        this.clearDraftMeta('staff');
        this.sMode = 'create';
        this.sEditing = '';
        this.sId = '';
        this.sName = '';
        this.sNotes = '';
        window.AB.focusLater(FIELD_IDS.staff.id);
      },

      editStaff: function (id) {
        if (this.actionsLocked('staff')) return;
        var s = this.findEntry('staff', id);
        if (!s) return;
        this.statusText = '';
        this.clearDraftMeta('staff');
        this.sMode = 'edit';
        this.sEditing = id;
        this.sId = id;
        this.sName = s.display_name || '';
        this.sNotes = s.notes || '';
        window.AB.focusLater(FIELD_IDS.staff.display_name);
      },

      cancelStaff: function () {
        var edited = this.sEditing;
        this.closeEditor('staff');
        window.AB.focusLater(edited ? this.editId('staff', edited) : this.newId('staff'), this.newId('staff'));
      },

      saveStaff: function () {
        return this.saveEntry('staff');
      },

      get staffRows() {
        var self = this;
        return (this.staff || []).map(function (s) {
          var used = s.used_in || [];
          return {
            id: s.id,
            displayName: s.display_name || '',
            notes: s.notes || '',
            hasNotes: Boolean(s.notes),
            usage: used.length > 0 ? 'Dùng trong đội hình: ' + used.join(', ') : 'Chưa dùng trong đội hình nào',
            visible: !(self.sMode === 'edit' && self.sEditing === s.id),
            editId: self.editId('staff', s.id),
            delId: self.delId('staff', s.id),
            editLabel: 'Sửa nhân sự ' + s.display_name,
            delLabel: 'Xóa nhân sự ' + s.display_name,
          };
        });
      },

      get staffEmpty() {
        return this.loaded && this.staff.length === 0 && this.sMode === '';
      },

      get staffHasRows() {
        return this.staff.length > 0;
      },

      // ---- save (shared by the three editors) ----

      buildEntry: function (kind) {
        var i;
        if (kind === 'team') {
          return {
            name: this.tName,
            description: this.tDesc.trim(),
            roles: {
              lead: this.tLead,
              dev: this.tDev,
              qa: this.tQa,
              plan_review: this.tPlanReview,
              code_review: this.tCodeReview,
            },
          };
        }
        if (kind === 'workflow') {
          var stages = [];
          for (i = 0; i < STAGES.length; i++) {
            if (this.wStageOn[STAGES[i].key]) stages.push(STAGES[i].key);
          }
          var loops = {};
          for (i = 0; i < LOOPS.length; i++) {
            if (this.loopChecked(LOOPS[i])) loops[LOOPS[i].key] = LOOPS[i].b;
          }
          var entry = {
            name: this.wName,
            stages: stages,
            loops: loops,
            auto: Boolean(this.wAuto),
            budget_minutes: Number(String(this.wBudgetMinutes).trim()),
            budget_usd: Number(String(this.wBudgetUsd).trim()),
          };
          var rounds = String(this.wMaxRounds).trim();
          if (isInt(rounds) && Number(rounds) >= 1 && Number(rounds) <= 10) {
            entry.max_rounds = Number(rounds);
          }
          return entry;
        }
        return { id: this.sId, display_name: this.sName.trim(), notes: this.sNotes.trim() };
      },

      savedLabel: function (kind, entry) {
        if (kind === 'staff') return entry.display_name;
        return entry.name;
      },

      saveEntry: function (kind) {
        var self = this;
        var k = KIND[kind];
        var p = k.pfx;
        if (this.writeLocked || this.invalidFile(kind)) return Promise.resolve();
        var errs = this.clientErrors(kind);
        if (errs.length > 0) {
          this[p + 'Tried'] = true;
          this.focusField(kind, errs[0].field);
          return Promise.resolve();
        }
        this.statusText = '';
        this[p + 'FieldErrors'] = {};
        this[p + 'Refusal'] = [];
        this[p + 'Conflict'] = false;
        this.saving = true;
        var entry = this.buildEntry(kind);
        var body = {
          version: this.versionOf(kind),
          create: this.modeOf(kind) !== 'edit',
          entry: entry,
        };
        var edited = this.editingOf(kind);
        return window.AB.api('POST', k.savePath, body).then(function (res) {
          self.saving = false;
          return self.afterSave(kind, res, entry, edited);
        });
      },

      afterSave: function (kind, res, entry, edited) {
        var self = this;
        var k = KIND[kind];
        var p = k.pfx;
        var data = res ? res.data : null;
        if (res && res.ok && res.status === 200 && data) {
          this.applyList(kind, data);
          this.closeEditor(kind);
          this.statusText = 'Đã lưu ' + k.noun + ' ' + this.savedLabel(kind, entry) + '.';
          window.AB.focusLater(edited ? this.editId(kind, edited) : this.newId(kind), this.newId(kind));
          return Promise.resolve();
        }
        if (res && res.status === 409 && data && data.code === 'conflict') {
          // The list is reloaded, the draft stays in the form.
          return this.reloadKind(kind).then(function () {
            self[p + 'Conflict'] = true;
            window.AB.focusLater(self.conflictId(kind));
          });
        }
        if (res && res.status === 409 && data && data.code === 'invalid_file') {
          // The file now has errors: reload shows the file-error state and blocks writes.
          return this.reloadKind(kind).then(function () {
            self[p + 'Refusal'] = [data.error || self.fileReason(kind)];
            window.AB.focusLater(self.refusalId(kind));
          });
        }
        var fieldErrors = {};
        if (data && Array.isArray(data.errors)) {
          for (var i = 0; i < data.errors.length; i++) {
            var e = data.errors[i];
            if (e && e.field && !fieldErrors[e.field]) fieldErrors[e.field] = e.message;
          }
        }
        this[p + 'FieldErrors'] = fieldErrors;
        this[p + 'Refusal'] = errorLines(data, res);
        window.AB.focusLater(this.refusalId(kind));
        return Promise.resolve();
      },

      // ---- delete dialogs ----

      askDelete: function (kind, key) {
        if (this.actionsLocked(kind)) return;
        var e = this.findEntry(kind, key);
        if (!e) return;
        if (kind !== 'staff' && e.source === 'builtin') return;
        this.statusText = '';
        this.delKind = kind;
        this.delName = key;
        this.delLabel = kind === 'staff' ? (e.display_name || key) : key;
        this.delError = '';
        this.delBusy = false;
        window.AB.lockScroll(true);
        window.AB.focusLater('ab-set-keep-' + kind);
      },

      isDeleting: function (kind) {
        return this.delKind === kind;
      },

      delEntry: function () {
        if (!this.delKind) return null;
        return this.findEntry(this.delKind, this.delName);
      },

      delTitle: function (kind) {
        return 'Xóa ' + KIND[kind].noun + ' ' + this.delLabel + '?';
      },

      delBody: function (kind) {
        if (kind === 'team') {
          return 'Đội hình này bị xóa khỏi teams.json và không khôi phục được từ dashboard. Bấm Nhân bản trước nếu muốn giữ một bản.';
        }
        if (kind === 'workflow') {
          return 'Quy trình bị xóa khỏi workflows.json. Task đã giao giữ bản sao nên không bị ảnh hưởng. Dự án đang dùng quy trình này làm mặc định sẽ dùng mac-dinh.';
        }
        return 'Nhân sự bị xóa khỏi danh sách và không khôi phục được từ dashboard.';
      },

      // Server-derived lines below the body: usage, then the override note.
      delExtra: function (kind) {
        var lines = [];
        var e = this.delEntry();
        if (!e || this.delKind !== kind) return lines;
        if (kind === 'team') {
          var used = e.used_by || {};
          if (used.projects && used.projects.length > 0) {
            lines.push('Đang là đội hình mặc định của: ' + used.projects.join(', ') + '.');
          }
          if (used.open_tasks > 0) {
            lines.push(used.open_tasks + ' task chưa xong đang dùng đội hình này; sau khi xóa, các task đó cần Đổi đội hình trước khi chạy bước kế tiếp.');
          }
          if (e.source === 'override') {
            lines.push('Đội hình có sẵn ' + e.name + ' sẽ được dùng lại.');
          }
        } else if (kind === 'workflow') {
          if (e.source === 'override') {
            lines.push('Quy trình có sẵn ' + e.name + ' sẽ được dùng lại.');
          }
        }
        return lines;
      },

      delConfirmReason: function (kind) {
        if (kind !== 'staff') return '';
        var e = this.delEntry();
        if (!e || this.delKind !== kind) return '';
        var used = e.used_in || [];
        if (used.length > 0) {
          return 'Đang dùng trong đội hình: ' + used.join(', ') + '. Đổi engine của vai trò đó trước rồi xóa.';
        }
        return '';
      },

      delConfirmDisabled: function (kind) {
        return this.delBusy || this.delConfirmReason(kind) !== '';
      },

      delConfirmText: function (kind) {
        return this.delBusy ? 'Đang xóa...' : KIND[kind].delLabel;
      },

      delKeepText: function (kind) {
        return KIND[kind].keepLabel;
      },

      closeDelete: function () {
        if (this.delBusy) return;
        var kind = this.delKind;
        var key = this.delName;
        this.delKind = '';
        this.delName = '';
        this.delLabel = '';
        this.delError = '';
        window.AB.lockScroll(false);
        if (kind) {
          window.AB.focusLater(this.delId(kind, key), this.newId(kind));
        }
      },

      confirmDelete: function () {
        var self = this;
        var kind = this.delKind;
        if (!kind || this.delBusy || this.delConfirmReason(kind) !== '') return Promise.resolve();
        var k = KIND[kind];
        var key = this.delName;
        var label = this.delLabel;
        var body = { version: this.versionOf(kind) };
        if (kind === 'staff') body.id = key;
        else body.name = key;
        this.delBusy = true;
        this.delError = '';
        return window.AB.api('POST', k.delPath, body).then(function (res) {
          self.delBusy = false;
          var data = res ? res.data : null;
          if (res && res.ok && res.status === 200 && data) {
            self.applyList(kind, data);
            self.delKind = '';
            self.delName = '';
            self.delLabel = '';
            window.AB.lockScroll(false);
            self.statusText = 'Đã xóa ' + k.noun + ' ' + label + '.';
            window.AB.focusLater(self.newId(kind), self.newId(kind));
            return;
          }
          if (res && res.status === 409 && data && data.code === 'conflict') {
            self.delError = (data.error || k.conflict) + '. Đã tải lại danh sách; kiểm tra rồi xóa lại nếu vẫn cần.';
            self.reloadKind(kind);
            return;
          }
          if (res && res.status === 409 && data && data.code === 'invalid_file') {
            self.delError = data.error || self.fileReason(kind);
            self.reloadKind(kind);
            return;
          }
          self.delError = errorLines(data, res).join(' ');
        });
      },

      // ---- engines (read-only, D-08) ----

      // Rows come from a whitelist of fields of GET /api/engines, in API order: id, kind, label,
      // readiness, reason, command, key variable NAME, model, pin / allowed profiles, evidence,
      // container, usage and the staff count. Nothing else of the payload is read (D-03).
      get engineRows() {
        var self = this;
        var list = (this.engines || []).filter(function (e) { return e && e.name; });
        // The section is never empty: `claude` and `người` always have a row.
        if (list.length === 0) list = [{ name: 'claude' }, { name: HUMAN_ENGINE }];
        return list.map(function (e) {
          var name = String(e.name);
          var kind = e.kind;
          if (!kind) kind = (name === 'claude' || name === HUMAN_ENGINE) ? name : '';
          var key = READINESS_WORDS[e.readiness] ? e.readiness : 'chua_kiem';
          var details = [];
          if (kind === HUMAN_ENGINE) {
            details.push({ label: '', value: e.detail ? String(e.detail) : (self.staff.length + ' nhân sự'), cls: '' });
          } else {
            if (e.command) details.push({ label: 'Lệnh: ', value: String(e.command), cls: 'font-mono' });
            if (e.key_env) details.push({ label: 'Biến key: ', value: String(e.key_env), cls: 'font-mono' });
            if (e.model && kind !== 'dsh') details.push({ label: 'Model: ', value: String(e.model), cls: 'font-mono' });
            if (e.pin_profile) details.push({ label: 'Profile cố định: ', value: String(e.pin_profile), cls: 'font-mono' });
            var allowed = Array.isArray(e.allowed_profiles) ? e.allowed_profiles : [];
            var onlyPin = allowed.length === 1 && String(allowed[0]) === String(e.pin_profile || '');
            if (allowed.length > 0 && !onlyPin) {
              details.push({ label: 'Chỉ cho dự án profile: ', value: allowed.join(', '), cls: 'font-mono' });
            }
            if (e.evidence) details.push({ label: 'Bằng chứng E-09: ', value: String(e.evidence), cls: '' });
            if (e.container) details.push({ label: 'Container: ', value: String(e.container), cls: 'font-mono' });
            if (kind === 'dsh' || e.usage === 'none') details.push({ label: '', value: NO_TOKEN_LINE, cls: '' });
          }
          var description = KIND_DESC[kind];
          if (kind === 'anthropic') {
            description = e.key_env ? DESC_ANTHROPIC_ENDPOINT : DESC_ANTHROPIC_NATIVE;
          }
          if (!description) description = e.description ? String(e.description) : '';
          var reason = '';
          if (key !== 'san_sang' && e.reason) reason = String(e.reason);
          var label = e.label ? String(e.label) : '';
          return {
            name: name,
            kind: kind,
            chip: KIND_CHIP[kind] || kind,
            key: key,
            word: READINESS_WORDS[key],
            label: label,
            hasLabel: label !== '',
            description: description,
            details: details,
            reason: reason,
            hasReason: reason !== '',
          };
        });
      },

      engineBadgeClass: function (row) {
        return window.AB.stepClass(row.key);
      },

      engineGlyph: function (row) {
        return window.AB.stepGlyph(row.key);
      },
    };
  });
})();

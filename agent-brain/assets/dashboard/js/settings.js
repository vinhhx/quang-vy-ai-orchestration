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

  function validSection(name) {
    for (var i = 0; i < SECTIONS.length; i++) {
      if (SECTIONS[i].id === name) return true;
    }
    return false;
  }

  window.AB.onReady('settings', function () {
    return {
      section: 'teams',
      sections: SECTIONS,
      loaded: false,
      loadError: '',
      stale: false,
      reloading: false,
      statusText: '',

      teams: [],
      teamErrors: [],
      teamsVersion: '',
      builtinTeams: [],

      init: function () {
        var stored = '';
        try {
          stored = sessionStorage.getItem(SECTION_KEY) || '';
        } catch (e) {}
        if (validSection(stored)) {
          this.section = stored;
        }
        this.load();
      },

      // Reads teams (Task 2 adds workflows, staff and engines). Before the first
      // answer a failure shows the load-error card; afterwards the data is kept.
      load: function () {
        var self = this;
        return window.AB.api('GET', '/api/teams').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.teams)) {
            self.applyTeams(res.data);
            self.loaded = true;
            self.stale = false;
            self.loadError = '';
          } else if (!self.loaded) {
            self.loadError = window.AB.errorText(res);
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

      showsSection: function (name) {
        return this.section === name;
      },

      get isTeamsSection() {
        return this.section === 'teams';
      },

      get isWorkflowsSection() {
        return this.section === 'workflows';
      },

      get isStaffSection() {
        return this.section === 'staff';
      },

      get isEnginesSection() {
        return this.section === 'engines';
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

      teamChips: function (t) {
        if (!t) return [];
        if (t.source === 'builtin') return ['có sẵn'];
        if (t.source === 'override') return ['ghi đè đội hình có sẵn'];
        return ['từ teams.json'];
      },

      get teamRows() {
        var self = this;
        return (this.teams || []).map(function (t) {
          return {
            name: t.name,
            description: t.description || '',
            summary: t.summary || '',
            chips: self.teamChips(t),
          };
        });
      },
    };
  });
})();

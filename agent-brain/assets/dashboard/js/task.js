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
      lastFocusId: '',
      returnNote: '',
      runId: null,
      _taskSeq: 0,
      _taskApplied: 0,

      pipe: null,
      pipeError: '',
      pipeStale: false,
      ledgerVisible: 20,
      _pipeSeq: 0,
      _pipeBusy: false,
      _pipeAgain: false,
      _pipeStarted: 0,
      _left: false,

      // 06-17: change-team / quota re-run dialog (one panel, two modes: '' closed,
      // 'change' or 'rerun'). The team lists are fetched when a dialog opens, never
      // on the 3 s poll.
      teamDlgMode: '',
      teamDlgTeams: [],
      teamDlgPick: '',
      teamDlgBusy: false,
      teamDlgError: '',
      teamDlgLoading: false,
      teamDlgFailed: false,
      teamDlgNotReady: {},
      teamRefusal: '',
      quotaRefusal: '',
      quotaPostponedRun: null,
      staffVer: 0,
      _staffLoaded: false,

      stageTitles: {
        start: 'Bắt đầu',
        plan: 'Lên plan',
        plan_review: 'Review plan',
        ban: 'Bạn',
        code: 'Code',
        code_review: 'Review code',
        qa: 'QA',
        fix: 'Sửa',
      },

      // Live log state
      pinned: false,
      rows: [],
      dropped: false,
      streamState: '',
      streamRun: null,
      follow: true,
      showThinking: false,
      raw: null,
      rawFocus: null,
      _es: null,

      init: function () {
        var self = this;
        this.load();

        window.AB.every(3000, function () {
          self.load();
        });

        window.AB.onLeave(function () {
          self._left = true;
          self._pipeSeq += 1;
          self._pipeAgain = false;
          if (self._es) {
            self._es.close();
            self._es = null;
          }
        });
      },

      load: function () {
        var self = this;
        if (!this.id) return;
        this.loadPipeline();
        // IN-07: polls can land out of order on a slow server; a response older
        // than the last applied one is dropped, so it cannot bring back state
        // (and buttons) from before a successful control.
        var seq = ++this._taskSeq;
        window.AB.api('GET', '/api/tasks/' + this.id).then(function (res) {
          if (seq <= self._taskApplied) return;
          self._taskApplied = seq;
          if (res.ok && res.data) {
            self.task = res.data;
            self.loaded = true;
            self.stale = false;
            self.error = '';
            self.notFound = false;

            if (!self.pinned) {
              var targetId = null;
              if (self.task && self.task.steps) {
                for (var i = 0; i < self.task.steps.length; i++) {
                  if (self.task.steps[i].state === 'dang_chay' && self.task.steps[i].run_id) {
                    targetId = self.task.steps[i].run_id;
                    break;
                  }
                }
              }
              if (!targetId && self.task && self.task.runs && self.task.runs.length > 0) {
                targetId = self.task.runs[self.task.runs.length - 1].id;
              }
              if (targetId && targetId !== self.runId) {
                self.openStream(targetId);
              }
            }
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

      loadPipeline: function () {
        var self = this;
        if (!this.id || this._left || this.notFound) return;
        if (this._pipeBusy) {
          // 05.1 WR-04: a request busy for 10 s or more is superseded; the
          // seq guard below discards its late response.
          if (Date.now() - this._pipeStarted < 10000) {
            this._pipeAgain = true;
            return;
          }
          this.pipeStale = true;
          this._pipeAgain = false;
        }
        this._pipeBusy = true;
        this._pipeStarted = Date.now();
        var seq = ++this._pipeSeq;
        function finish() {
          if (seq !== self._pipeSeq) return;
          self._pipeBusy = false;
          if (self._pipeAgain) {
            self._pipeAgain = false;
            self.loadPipeline();
          }
        }
        window.AB.api('GET', '/api/tasks/' + this.id + '/pipeline').then(function (res) {
          if (seq === self._pipeSeq) {
            if (res && res.ok && res.data) {
              self.pipe = res.data;
              self.pipeError = '';
              self.pipeStale = false;
              self.ensureStaffNames();
            } else if (res && res.status === 404) {
              // nothing (load() shows the not-found state)
            } else {
              if (self.pipe) {
                self.pipeStale = true;
              } else {
                self.pipeError = window.AB.errorText(res);
              }
            }
          }
          finish();
        }).catch(function (err) {
          if (seq === self._pipeSeq) {
            if (self.pipe) {
              self.pipeStale = true;
            } else {
              self.pipeError = (err && err.message) ? err.message : 'Lỗi kết nối';
            }
          }
          finish();
        });
      },

      reload: function () {
        this.error = '';
        this.load();
      },

      openStream: function (id) {
        var self = this;
        if (this._es) {
          this._es.close();
          this._es = null;
        }
        this.runId = id;
        this.rows = [];
        this.dropped = false;
        this.streamState = 'đang kết nối';

        if (this.task && this.task.runs) {
          for (var i = 0; i < this.task.runs.length; i++) {
            if (this.task.runs[i].id === id) {
              this.streamRun = {
                role: this.task.runs[i].role,
                status: this.task.runs[i].status,
                outcome: this.task.runs[i].outcome || null
              };
              break;
            }
          }
        }

        var es = new EventSource('/api/runs/' + id + '/stream');
        this._es = es;

        es.onopen = function () {
          self.streamState = 'live';
        };

        es.onmessage = function (e) {
          try {
            var row = JSON.parse(e.data);
            self.rows.push(row);
            if (self.rows.length > 5000) {
              self.rows.shift();
              self.dropped = true;
            }
            if (self.follow) {
              setTimeout(function () {
                var el = document.getElementById('ab-log');
                if (el) el.scrollTop = el.scrollHeight;
              }, 0);
            }
          } catch (err) {}
        };

        es.addEventListener('status', function (e) {
          try {
            var s = JSON.parse(e.data);
            var prev = self.streamRun;
            self.streamRun = {
              role: s.role,
              status: s.status,
              outcome: s.outcome || null
            };
            if (prev && (prev.status !== s.status || prev.outcome !== (s.outcome || null))) {
              self.loadPipeline();
            }
          } catch (err) {}
        });

        es.addEventListener('end', function () {
          self.streamState = 'đã kết thúc';
          es.close();
          self._es = null;
          self.load();
        });

        es.onerror = function () {
          if (es.readyState === EventSource.CLOSED) {
            self.streamState = 'lỗi';
          } else {
            self.streamState = 'mất kết nối — đang thử lại...';
          }
        };
      },

      selectRun: function (id) {
        this.pinned = true;
        this.openStream(id);
      },

      onScroll: function () {
        var el = document.getElementById('ab-log');
        if (!el) return;
        var dist = el.scrollHeight - el.scrollTop - el.clientHeight;
        if (dist > 40) {
          this.follow = false;
        } else {
          this.follow = true;
        }
      },

      jumpLive: function () {
        this.follow = true;
        var el = document.getElementById('ab-log');
        if (el) el.scrollTop = el.scrollHeight;
      },

      toggleThinking: function () {
        this.showThinking = !this.showThinking;
      },

      openRaw: function (r) {
        var self = this;
        this.rawFocus = document.activeElement;
        this.raw = { seq: r.seq, text: '', error: '' };
        window.AB.lockScroll(true);
        window.AB.api('GET', '/api/runs/' + this.runId + '/events/' + r.seq + '/raw').then(function (res) {
          if (res.ok && res.data && res.data.raw_json !== undefined) {
            try {
              self.raw.text = JSON.stringify(JSON.parse(res.data.raw_json), null, 2);
            } catch (e) {
              self.raw.text = res.data.raw_json;
            }
          } else {
            self.raw.error = 'Không tải được dữ liệu thô của dòng này.';
          }
          window.AB.focusLater('ab-raw-close');
        });
      },

      closeRaw: function () {
        this.raw = null;
        window.AB.lockScroll(false);
        if (this.rawFocus && this.rawFocus.focus) {
          this.rawFocus.focus();
        }
      },

      onRawOverlay: function (event) {
        if (event.target === event.currentTarget) {
          this.closeRaw();
        }
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

      get bannerState() {
        if (!this.task) return 'cho';
        var g = this.task.group;
        if (g === 'running') return 'dang_chay';
        if (g === 'waiting') return 'cho_ban';
        if (g === 'attention') {
          if (this.task.status === 'blocked' || this.task.status === 'qa_fix') {
            return 'vuong_mac';
          }
          return 'loi';
        }
        if (g === 'done') return 'xong';
        return 'cho';
      },

      get bannerClass() {
        return window.AB.stepClass(this.bannerState);
      },

      get bannerGlyph() {
        return window.AB.stepGlyph(this.bannerState);
      },

      // ---- Staff names for human engine texts (read once, only when a human run shows) ----
      // `người` or `người:{id}` (a person did / will do the step).
      isHumanEngine: function (engine) {
        return String(engine === null || engine === undefined ? '' : engine).indexOf('người') === 0;
      },

      pipeHasHumanEngine: function () {
        var p = this.pipe;
        if (!p) return false;
        var gs = p.groups || [];
        for (var i = 0; i < gs.length; i++) {
          var cs = (gs[i] && gs[i].cards) ? gs[i].cards : [];
          for (var j = 0; j < cs.length; j++) {
            if (cs[j] && this.isHumanEngine(cs[j].engine)) return true;
          }
        }
        var rows = this.ledgerAllRows;
        for (var k = 0; k < rows.length; k++) {
          if (rows[k] && this.isHumanEngine(rows[k].engine)) return true;
        }
        return false;
      },

      // `AB.setStaffNames` feeds `AB.engineText`; the version bump re-renders the engine texts.
      ensureStaffNames: function () {
        var self = this;
        if (this._staffLoaded || !this.pipeHasHumanEngine()) return;
        this._staffLoaded = true;
        return this.fetchStaffMap().then(function (names) {
          if (names) {
            window.AB.setStaffNames(names);
            self.staffVer += 1;
          } else {
            self._staffLoaded = false;
          }
        });
      },

      // {id: display_name} from GET /api/staff, or null when it could not be read.
      fetchStaffMap: function () {
        return window.AB.api('GET', '/api/staff').then(function (res) {
          if (!(res && res.ok && res.data && Array.isArray(res.data.staff))) return null;
          var names = {};
          for (var i = 0; i < res.data.staff.length; i++) {
            var m = res.data.staff[i];
            if (m && m.id) names[m.id] = m.display_name || m.id;
          }
          return names;
        });
      },

      // ---- Team line, history (D-13) — only the task-detail payload ----
      get teamView() {
        return (this.task && this.task.team) ? this.task.team : null;
      },

      get teamName() {
        return this.teamView ? (this.teamView.name || '') : '';
      },

      get teamLineText() {
        var t = this.teamView;
        if (!t) return '';
        if (t.legacy) {
          return 'Đội hình: ' + t.name + ' (task tạo trước Phase 6 — mọi vai trò chạy bằng claude)';
        }
        if (t.missing) {
          return 'Đội hình: ' + t.name + ' — không còn trong teams.json';
        }
        return 'Đội hình: ' + t.name + (t.summary ? ' · ' + t.summary : '');
      },

      get hasTeamLine() {
        return this.teamLineText !== '';
      },

      get teamMissing() {
        var t = this.teamView;
        return Boolean(t && t.missing && !t.legacy);
      },

      get teamMissingText() {
        return 'Chọn đội hình khác bằng Đổi đội hình trước khi chạy bước tiếp.';
      },

      histTime: function (ms) {
        if (ms === null || ms === undefined || isNaN(ms)) return '—';
        var d = new Date(ms);
        if (isNaN(d.getTime())) return '—';
        function pad(v) { return v < 10 ? '0' + v : String(v); }
        return pad(d.getDate()) + '/' + pad(d.getMonth() + 1) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
      },

      histBy: function (by) {
        if (by === 'telegram') return 'bạn qua Telegram';
        if (by === 'cli') return 'bạn qua CLI';
        return 'bạn qua dashboard';
      },

      // Oldest first, no cap.
      get teamHistoryLines() {
        var h = (this.task && Array.isArray(this.task.team_history)) ? this.task.team_history : [];
        var lines = [];
        for (var i = 0; i < h.length; i++) {
          var e = h[i];
          if (!e) continue;
          lines.push(this.histTime(e.at) + ' · ' + this.histBy(e.by) + ': ' + e.from + ' → ' + e.to);
        }
        return lines;
      },

      get hasTeamHistory() {
        return this.teamHistoryLines.length > 0;
      },

      get changeTeamAction() {
        var acts = this.actionList;
        for (var i = 0; i < acts.length; i++) {
          if (acts[i] && acts[i].id === 'change_team') return acts[i];
        }
        return null;
      },

      // The button renders only when the payload carries the action.
      get teamControls() {
        var a = this.changeTeamAction;
        return a ? [a] : [];
      },

      get changeTeamReason() {
        var a = this.changeTeamAction;
        return a ? this.reasonText(a) : '';
      },

      // ---- Quota stop (D-15 / D-16 / D-18) ----
      get quotaView() {
        return (this.task && this.task.quota) ? this.task.quota : null;
      },

      get hasQuota() {
        return Boolean(this.quotaView);
      },

      get quotaRerunAction() {
        var acts = this.actionList;
        for (var i = 0; i < acts.length; i++) {
          if (acts[i] && acts[i].id === 'quota_rerun') return acts[i];
        }
        return null;
      },

      // `Thử lại sau` is client-only: it remembers the run it was pressed for, so a reload
      // (or a newer quota stop) shows the full banner again.
      get quotaPostponed() {
        var q = this.quotaView;
        return Boolean(q && this.quotaPostponedRun !== null && this.quotaPostponedRun === q.run_id);
      },

      get quotaBannerFull() {
        return this.hasQuota && !this.quotaPostponed;
      },

      get quotaHint() {
        var q = this.quotaView;
        if (!q) return '';
        var label = q.retry_label ? q.retry_label : 'nút chạy lại của bước';
        return 'Đã hoãn. Khi quota hồi phục, bấm ' + label + ' ở dưới để chạy lại bằng engine ' + q.engine + '.';
      },

      get quotaBody2() {
        return this.quotaBannerFull
          ? 'Chạy lại bằng đội hình khác sẽ tiếp tục trên thư mục làm việc hiện tại: giữ nguyên diff đang có, đọc lại .plan.md (cùng .handover.md nếu có).'
          : '';
      },

      postponeQuota: function () {
        var q = this.quotaView;
        if (!q) return;
        this.quotaPostponedRun = q.run_id;
        this.quotaRefusal = '';
        // Writes nothing and starts nothing (D-16): the step's own button stays in the strip.
        window.AB.focusLater('ab-stop-heading', 'screen');
      },

      // Shown reason of an action: the server's when it is disabled, else the last refusal.
      reasonText: function (a) {
        if (!a) return '';
        if (!a.enabled && a.reason) return a.reason;
        if (a.id === 'change_team') return this.teamRefusal;
        if (a.id === 'quota_rerun') return this.quotaRefusal;
        return '';
      },

      // ---- Team dialogs (change team / quota re-run) ----
      get teamDlgOpen() {
        return this.teamDlgMode !== '';
      },

      get rerunOpen() {
        return this.teamDlgMode === 'rerun';
      },

      get teamDlgTitle() {
        if (this.teamDlgMode === 'change') {
          return 'Đổi đội hình cho task ' + ((this.task && this.task.slug) ? this.task.slug : '');
        }
        if (this.teamDlgMode === 'rerun') {
          var q = this.quotaView;
          return 'Chạy lại bước ' + (q ? q.step_name : '') + ' bằng đội hình nào?';
        }
        return '';
      },

      get teamDlgBody() {
        if (this.teamDlgMode === 'change') {
          return 'Đội hình mới áp dụng từ bước kế tiếp; các run đã chạy giữ nguyên engine của chúng. Việc đổi được ghi lại trong lịch sử.';
        }
        if (this.teamDlgMode === 'rerun') {
          return 'Đội hình bạn chọn thành đội hình của task từ bước này trở đi. Engine mới tiếp tục từ thư mục làm việc hiện tại, không làm lại từ đầu.';
        }
        return '';
      },

      get teamDlgOkLabel() {
        if (this.teamDlgMode === 'rerun') return this.teamDlgBusy ? 'Đang giao...' : 'Chạy lại bước này';
        return this.teamDlgBusy ? 'Đang đổi...' : 'Đổi đội hình';
      },

      get teamDlgCancelLabel() {
        return this.teamDlgMode === 'rerun' ? 'Không chạy lại' : 'Giữ đội hình hiện tại';
      },

      // First failing reason, shown under the primary button while it is disabled.
      get teamDlgReason() {
        if (this.teamDlgLoading || this.teamDlgBusy) return '';
        if (this.teamDlgMode === 'change') {
          if (!this.teamDlgPick || this.teamDlgPick === this.teamName) return 'Chọn đội hình khác đội hình hiện tại.';
          return '';
        }
        if (this.teamDlgMode === 'rerun') {
          return this.teamDlgPick ? '' : 'Chọn một đội hình.';
        }
        return '';
      },

      get teamDlgOkDisabled() {
        if (this.teamDlgBusy || this.teamDlgLoading || Boolean(this.busyAction)) return true;
        return this.teamDlgReason !== '';
      },

      get hasTeamDlgReason() {
        return this.teamDlgReason !== '';
      },

      get hasTeamDlgError() {
        return this.teamDlgError !== '';
      },

      get teamDlgLoadFailed() {
        return this.teamDlgFailed && !this.teamDlgLoading;
      },

      teamDlgRadioId: function (t) {
        return 'ab-tdlg-radio-' + (t ? t.name : '');
      },

      teamDlgChecked: function (t) {
        return Boolean(t && this.teamDlgPick && t.name === this.teamDlgPick);
      },

      teamDlgOptionClass: function (t) {
        if (this.teamDlgChecked(t)) {
          return 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400';
        }
        return 'ring-1 ring-slate-300 hover:bg-slate-50 dark:ring-slate-600 dark:hover:bg-slate-700/50';
      },

      // Engine of the stopped step's role under team `t`: `lead`, `plan_review` ... keys.
      teamDlgRoleEngine: function (t) {
        var q = this.quotaView;
        if (!q || !t || !t.roles) return '';
        var key = String(q.role || '').replace(/-/g, '_');
        var eng = t.roles[key];
        return eng ? String(eng) : '';
      },

      teamDlgChips: function (t) {
        if (!t) return [];
        var chips = [];
        if (this.teamDlgMode === 'change' && t.name === this.teamName) chips.push('đang dùng');
        chips.push(t.source === 'builtin' ? 'có sẵn' : 'từ teams.json');
        if (this.teamDlgMode === 'rerun') {
          var q = this.quotaView;
          var eng = this.teamDlgRoleEngine(t);
          if (q && eng && eng === q.engine) chips.push('vẫn là ' + q.engine);
        }
        return chips;
      },

      // `Bước {step} sẽ chạy bằng: {engine}` (re-run dialog only).
      teamDlgStepLine: function (t) {
        if (this.teamDlgMode !== 'rerun') return '';
        var q = this.quotaView;
        var eng = this.teamDlgRoleEngine(t);
        if (!q || !eng) return '';
        return 'Bước ' + q.step_name + ' sẽ chạy bằng: ' + window.AB.engineText(eng);
      },

      // Engines of the team's roles that are not ready, as Giao việc shows them.
      teamDlgReadinessLines: function (t) {
        var lines = [];
        if (!t || !t.roles) return lines;
        var order = ['claude', 'deepseek', 'agy'];
        var roles = ['lead', 'dev', 'qa', 'plan_review', 'code_review'];
        var used = {};
        for (var i = 0; i < roles.length; i++) {
          if (t.roles[roles[i]]) used[t.roles[roles[i]]] = true;
        }
        for (var j = 0; j < order.length; j++) {
          var word = this.teamDlgNotReady ? this.teamDlgNotReady[order[j]] : '';
          if (used[order[j]] && word) lines.push('! ' + order[j] + ': ' + word);
        }
        return lines;
      },

      onTeamDlgChange: function (event) {
        var target = event && event.target;
        this.pickDlgTeam((target && target.value) ? target.value : this.teamDlgPick);
      },

      pickDlgTeam: function (name) {
        if (!name || this.teamDlgBusy) return;
        this.teamDlgPick = name;
        this.teamDlgError = '';
      },

      pickRerunTeam: function (name) {
        this.pickDlgTeam(name);
      },

      // GET /api/teams + /api/engines + /api/staff when a dialog opens.
      loadTeamDlg: function () {
        var self = this;
        this.teamDlgLoading = true;
        this.teamDlgFailed = false;
        var teams = window.AB.api('GET', '/api/teams').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.teams)) {
            self.teamDlgTeams = res.data.teams;
            self.teamDlgFailed = false;
          } else {
            self.teamDlgTeams = [];
            self.teamDlgFailed = true;
          }
        });
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
          self.teamDlgNotReady = map;
        }).catch(function () {
          self.teamDlgNotReady = {};
        });
        var staff = this.fetchStaffMap().then(function (names) {
          if (names) {
            window.AB.setStaffNames(names);
            self.staffVer += 1;
          }
        }).catch(function () {});
        return Promise.all([teams, engines, staff]).then(function () {
          self.teamDlgLoading = false;
        });
      },

      // Initial focus: the selected radio, else the first one, else Giữ / Không chạy lại.
      focusTeamDlg: function () {
        var pick = this.teamDlgPick;
        var list = this.teamDlgTeams || [];
        var target = null;
        for (var i = 0; i < list.length; i++) {
          if (list[i] && list[i].name === pick) target = list[i];
        }
        if (!target && list.length > 0) target = list[0];
        window.AB.focusLater(target ? this.teamDlgRadioId(target) : 'ab-tdlg-cancel', 'ab-tdlg-cancel');
      },

      openTeamDlg: function (mode, pick, focusBack) {
        var self = this;
        this.teamDlgMode = mode;
        this.teamDlgPick = pick;
        this.teamDlgBusy = false;
        this.teamDlgError = '';
        this.lastFocusId = focusBack;
        window.AB.lockScroll(true);
        return this.loadTeamDlg().then(function () {
          if (self.teamDlgMode === mode) self.focusTeamDlg();
        });
      },

      closeTeamDlg: function () {
        var back = this.lastFocusId;
        this.teamDlgMode = '';
        this.teamDlgPick = '';
        this.teamDlgBusy = false;
        this.teamDlgError = '';
        this.lastFocusId = '';
        window.AB.lockScroll(false);
        window.AB.focusLater(back || 'screen', 'screen');
      },

      teamDlgCancel: function () {
        if (this.teamDlgBusy) return;
        this.closeTeamDlg();
      },

      openChangeTeam: function () {
        if (this.busyAction || !this.task) return;
        this.teamRefusal = '';
        return this.openTeamDlg('change', this.teamName, 'ab-act-change_team');
      },

      closeChangeTeam: function () {
        this.teamDlgCancel();
      },

      openQuotaRerun: function () {
        if (this.busyAction || !this.quotaView) return;
        this.quotaRefusal = '';
        return this.openTeamDlg('rerun', '', 'ab-act-quota_rerun');
      },

      closeQuotaRerun: function () {
        this.teamDlgCancel();
      },

      teamDlgOk: function () {
        if (this.teamDlgMode === 'rerun') return this.confirmQuotaRerun();
        return this.confirmChangeTeam();
      },

      // POST /api/tasks/{id}/team {team, rev} → {ok, from, to}; a refusal shows `data.error`
      // inside the dialog and on the Đổi đội hình reason line.
      confirmChangeTeam: function () {
        var self = this;
        if (this.teamDlgMode !== 'change' || this.teamDlgOkDisabled) return;
        var pick = this.teamDlgPick;
        var rev = (this.task && this.task.rev !== undefined) ? this.task.rev : null;
        this.teamDlgBusy = true;
        this.busyAction = 'change_team';
        this.teamDlgError = '';
        return window.AB.api('POST', '/api/tasks/' + this.id + '/team', { team: pick, rev: rev }).then(function (res) {
          self.teamDlgBusy = false;
          self.busyAction = '';
          if (res && res.status === 200 && res.data && res.data.ok) {
            self.teamRefusal = '';
            self.closeTeamDlg();
            self.message = 'Đã đổi đội hình: ' + res.data.from + ' → ' + res.data.to + '.';
            self.load();
          } else {
            var text = (res && res.data && res.data.error) ? res.data.error : window.AB.errorText(res);
            self.teamDlgError = text;
            self.teamRefusal = text;
            if (res && res.data && res.data.kind === 'stale') self.load();
          }
        });
      },

      // POST /api/tasks/{id}/quota-rerun {team, rev} → {job_id}; the chosen team becomes the
      // task's team (history written by the server).
      confirmQuotaRerun: function () {
        var self = this;
        if (this.teamDlgMode !== 'rerun' || this.teamDlgOkDisabled) return;
        var pick = this.teamDlgPick;
        var q = this.quotaView;
        var step = q ? q.step_name : '';
        var rev = (this.task && this.task.rev !== undefined) ? this.task.rev : null;
        this.teamDlgBusy = true;
        this.busyAction = 'quota_rerun';
        this.teamDlgError = '';
        return window.AB.api('POST', '/api/tasks/' + this.id + '/quota-rerun', { team: pick, rev: rev }).then(function (res) {
          self.teamDlgBusy = false;
          self.busyAction = '';
          if (res && (res.status === 202 || res.status === 200) && res.data && res.data.job_id !== undefined) {
            self.quotaRefusal = '';
            self.closeTeamDlg();
            self.message = 'Đã đổi sang đội hình ' + pick + ' và giao lại bước ' + step + ' (job ' + res.data.job_id + ').';
            self.load();
          } else {
            var text = (res && res.data && res.data.error) ? res.data.error : window.AB.errorText(res);
            self.teamDlgError = text;
            self.quotaRefusal = text;
            if (res && res.data && res.data.kind === 'stale') self.load();
          }
        });
      },

      get ledgerAllRows() {
        if (!this.pipe || !this.pipe.ledger || !this.pipe.ledger.groups) return [];
        var all = [];
        var groups = this.pipe.ledger.groups;
        for (var i = 0; i < groups.length; i++) {
          if (groups[i].rows) {
            for (var j = 0; j < groups[i].rows.length; j++) {
              all.push(groups[i].rows[j]);
            }
          }
        }
        return all;
      },

      get ledgerGroups() {
        if (!this.pipe || !this.pipe.ledger || !this.pipe.ledger.groups) return [];
        var allRows = this.ledgerAllRows;
        var cut = Math.max(0, allRows.length - this.ledgerVisible);
        var groups = this.pipe.ledger.groups;
        var flatIdx = 0;
        var out = [];
        for (var i = 0; i < groups.length; i++) {
          var g = groups[i];
          var visibleRows = [];
          if (g.rows) {
            for (var j = 0; j < g.rows.length; j++) {
              if (flatIdx >= cut) {
                visibleRows.push(g.rows[j]);
              }
              flatIdx++;
            }
          }
          if (visibleRows.length > 0) {
            out.push({
              key: g.key,
              kind: g.kind,
              cycle: g.cycle,
              header: g.header,
              right: g.right,
              rows: visibleRows,
            });
          }
        }
        return out;
      },

      get ledgerHasRows() {
        return this.ledgerAllRows.length > 0;
      },

      get ledgerHasMore() {
        return this.ledgerAllRows.length > this.ledgerVisible;
      },

      get ledgerEmpty() {
        return Boolean(this.pipe && this.ledgerAllRows.length === 0);
      },

      get showLedgerCard() {
        return !this.pipeFailed;
      },

      get showTotal() {
        return this.ledgerHasRows;
      },

      get totalText() {
        var total = (this.pipe && this.pipe.ledger && this.pipe.ledger.total) ? this.pipe.ledger.total : null;
        if (!total) return '';
        return total.runs + ' lượt chạy · ' + window.AB.fmtMinutes(total.duration_ms) + ' · ' + window.AB.fmtUsd(total.cost_usd);
      },

      // C16: minutes count agent runs only; the helper says how many human runs were left out.
      get humanRunsNote() {
        var total = (this.pipe && this.pipe.ledger && this.pipe.ledger.total) ? this.pipe.ledger.total : null;
        var k = total ? total.human_runs : 0;
        if (k && k >= 1) {
          return 'Không tính thời gian chờ của ' + k + ' lượt do người làm.';
        }
        return '';
      },

      get hasHumanRunsNote() {
        return this.humanRunsNote !== '';
      },

      get costNote() {
        var total = (this.pipe && this.pipe.ledger && this.pipe.ledger.total) ? this.pipe.ledger.total : null;
        var k = total ? total.cost_unknown : 0;
        if (k && k >= 1) {
          return 'Chưa tính chi phí của ' + k + ' run bị dừng hoặc chưa có kết quả.';
        }
        return '';
      },

      get hasCostNote() {
        return Boolean(this.costNote);
      },

      // D-03 causes (also drive the amber markers on the plan section and the
      // QA card); the callouts themselves hide while the stop banner shows the
      // same cause, because the banner body carries the guidance.
      get blockedCause() {
        return Boolean(this.task && (this.task.status === 'blocked' || (this.task.plan && this.task.plan.blockers)));
      },

      get qaFixCause() {
        return Boolean(this.task && (this.task.status === 'qa_fix' || (this.task.qa && this.task.qa.verdict === 'Cần sửa')));
      },

      get isBlocked() {
        if (this.hasQuota) return false;
        if (this.showAutoStop && this.autoView.stop_reason === 'vuong_mac') return false;
        return this.blockedCause;
      },

      get isQaFix() {
        if (this.hasQuota) return false;
        if (this.showAutoStop && this.autoView.stop_reason === 'het_vong' && this.autoView.stopped_in === 'qa') return false;
        return this.qaFixCause;
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
        return this.qaFixCause ?'ring-2 ring-amber-500 dark:ring-amber-400' : 'ring-1 ring-slate-200 dark:ring-slate-700';
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
          return 'Kill run ' + this.dialog.run_id + ' (' + window.AB.roleName(this.killRole(this.dialog)) + ')?';
        }
        if (this.dialog.id === 'relead') {
          var slug = (this.task && this.task.slug) ? this.task.slug : '';
          return 'Chạy lại Lead cho task ' + slug + '?';
        }
        if (this.dialog.id === 'return_lead') {
          var rslug = (this.task && this.task.slug) ? this.task.slug : '';
          return 'Trả plan về Lead cho task ' + rslug + '?';
        }
        return '';
      },

      get dialogBody() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') {
          var kbody = 'Agent và mọi tiến trình con của nó sẽ bị dừng ngay. Thay đổi chưa commit trong thư mục dự án vẫn còn nguyên; task chuyển sang trạng thái killed.';
          if (this.autoState === 'running' || this.autoState === 'stopping') {
            kbody += ' Autopilot cũng dừng với lý do "dừng bởi bạn".';
          }
          return kbody;
        }
        if (this.dialog.id === 'relead' || this.dialog.id === 'return_lead') {
          var slug = (this.task && this.task.slug) ? this.task.slug : '';
          var body = 'Lead sẽ lập lại .plan.md trên nhánh task/' + slug + ' và commit \'' + slug + ': plan\'. Bản plan hiện tại vẫn còn trong lịch sử git.';
          if (this.dialog.id === 'return_lead' && this.task && this.task.workflow && this.task.workflow.auto) {
            body += ' Autopilot sẽ chạy lại vòng plan ⇄︎ review plan.';
          }
          return body;
        }
        return '';
      },

      get dialogIsReturn() {
        return Boolean(this.dialog && this.dialog.id === 'return_lead');
      },

      get dialogOkLabel() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') return 'Kill run';
        if (this.dialog.id === 'relead') return 'Chạy lại Lead';
        if (this.dialog.id === 'return_lead') return 'Trả về Lead';
        return '';
      },

      get dialogCancelLabel() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') return 'Giữ chạy tiếp';
        if (this.dialog.id === 'relead' || this.dialog.id === 'return_lead') return 'Giữ plan hiện tại';
        return '';
      },

      get dialogOkClass() {
        if (!this.dialog) return '';
        if (this.dialog.id === 'kill') {
          return 'bg-rose-600 text-white hover:bg-rose-700';
        }
        return 'bg-indigo-600 text-white hover:bg-indigo-700';
      },

      // Role of the run a Kill targets (Review plan / Review code included);
      // falls back to the running step for runs not in the task's list.
      killRole: function (a) {
        var runs = (this.task && this.task.runs) ? this.task.runs : [];
        if (a && a.run_id !== null && a.run_id !== undefined) {
          for (var i = 0; i < runs.length; i++) {
            if (runs[i] && runs[i].id === a.run_id && runs[i].role) return runs[i].role;
          }
        }
        return this.runningStepRole;
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

      get visibleRows() {
        var show = this.showThinking;
        return this.rows.filter(function (r) {
          return show || r.ev !== 'system/thinking_tokens';
        });
      },

      get hiddenCount() {
        var count = 0;
        for (var i = 0; i < this.rows.length; i++) {
          if (this.rows[i].ev === 'system/thinking_tokens') {
            count++;
          }
        }
        return count;
      },

      get thinkingLabel() {
        if (this.showThinking) {
          return 'Ẩn thinking_tokens';
        }
        return 'Hiện thinking_tokens (' + this.hiddenCount + ' đang ẩn)';
      },

      get streamLine() {
        if (!this.runId) return '';
        var role = '';
        var status = '';
        if (this.streamRun) {
          role = window.AB.roleName(this.streamRun.role);
          status = this.streamRun.status;
        } else if (this.task && this.task.runs) {
          var id = this.runId;
          for (var i = 0; i < this.task.runs.length; i++) {
            if (this.task.runs[i].id === id) {
              role = window.AB.roleName(this.task.runs[i].role);
              status = this.task.runs[i].status;
              break;
            }
          }
        }
        return 'run ' + this.runId + ' · ' + role + ' · ' + status;
      },

      get showJump() {
        return !this.follow;
      },

      get showLogEmpty() {
        return this.visibleRows.length === 0;
      },

      get logEmptyText() {
        if (this.streamState === 'đã kết thúc') {
          return 'Run này không có dòng log nào.';
        }
        return 'Chưa có dòng log nào. Log hiện khi run bắt đầu.';
      },

      get rawOpen() {
        return Boolean(this.raw);
      },

      get rawTitle() {
        if (!this.raw) return '';
        return 'Dữ liệu thô (events.raw_json) · run ' + this.runId + ' · dòng ' + this.raw.seq;
      },

      get rawText() {
        return this.raw ? (this.raw.text || '') : '';
      },

      get rawError() {
        return this.raw ? (this.raw.error || '') : '';
      },

      get pipeState() {
        if (this.pipe) return 'ready';
        if (this.pipeError) return 'error';
        return 'loading';
      },

      get hasPipe() {
        return Boolean(this.pipe);
      },

      get pipeLoading() {
        return !this.pipe && !this.pipeError;
      },

      get pipeFailed() {
        return !this.pipe && Boolean(this.pipeError);
      },

      get pipeErrorBody() {
        return (this.pipeError || 'Lỗi không xác định') + '. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get showStale() {
        return Boolean(this.stale || this.pipeStale);
      },

      get placeholderGlyph() {
        return window.AB.stepGlyph('chua_tu_dong');
      },

      reloadPipeline: function () {
        this.pipeError = '';
        this.loadPipeline();
      },

      legendGlyph: function (state) {
        return window.AB.stepGlyph(state);
      },

      legendGlyphClass: function (state) {
        return 'w-6 text-center ' + window.AB.stateText(state);
      },

      card: function (key) {
        return (this.pipe && this.pipe.cards) ? this.pipe.cards[key] || null : null;
      },

      // Bắt đầu card (05.1 point card, always xong once the task exists)
      cardState: function (key) {
        var c = this.card(key);
        if (c && c.state) return c.state;
        return key === 'start' ? 'xong' : 'cho';
      },

      cardClass: function (key) {
        return 'flex h-full min-w-0 flex-col gap-1 rounded-xl border p-4 ' + window.AB.stateCard(this.cardState(key));
      },

      cardCurrentAttr: function (key) {
        return this.isCurrentKey(key) ? 'step' : false;
      },

      cardGlyph: function (key) {
        return window.AB.stepGlyph(this.cardState(key));
      },

      cardGlyphClass: function (key) {
        return this.stateGlyphClass(this.cardState(key));
      },

      cardSr: function (key) {
        var c = this.card(key);
        var w = (c && c.state_word) ? c.state_word : window.AB.stateWord(this.cardState(key));
        return w + ', ';
      },

      cardResult: function (key) {
        var c = this.card(key);
        if (c && c.result) return c.result;
        return key === 'start' ? 'bạn đã giao' : '—';
      },

      cardTime: function (key) {
        var c = this.card(key);
        if (!c || c.at_ms === null || c.at_ms === undefined) return '—';
        return window.AB.fmtClock(c.at_ms);
      },

      stateGlyphClass: function (state) {
        var cls = 'w-6 shrink-0 text-center ' + window.AB.stateText(state);
        if (state === 'dang_chay') {
          cls += ' motion-safe:animate-pulse';
        }
        return cls;
      },

      // Data-driven flow (05.2): groups and cards come from the pipeline payload.
      get flowGroups() {
        return (this.pipe && this.pipe.groups) ? this.pipe.groups : [];
      },

      flowGroup: function (key) {
        var gs = this.flowGroups;
        for (var i = 0; i < gs.length; i++) {
          if (gs[i] && gs[i].key === key) return gs[i];
        }
        return null;
      },

      hasGroup: function (key) {
        return Boolean(this.flowGroup(key));
      },

      groupCards: function (key) {
        var g = this.flowGroup(key);
        return (g && g.cards) ? g.cards : [];
      },

      isCurrentKey: function (key) {
        return Boolean(this.pipe && this.pipe.current_card && this.pipe.current_card === key);
      },

      flowCardState: function (c) {
        return (c && c.state) ? c.state : 'cho';
      },

      flowCardClass: function (c) {
        var cls = 'flex h-full min-w-0 flex-col gap-1 rounded-xl border p-4 ' + window.AB.stateCard(this.flowCardState(c));
        if (c && this.isCurrentKey(c.key)) {
          cls += ' ring-2 ring-indigo-600 dark:ring-indigo-400';
        }
        return cls;
      },

      flowCardCurrent: function (c) {
        return (c && this.isCurrentKey(c.key)) ? 'step' : false;
      },

      flowCardGlyph: function (c) {
        return window.AB.stepGlyph(this.flowCardState(c));
      },

      flowCardGlyphClass: function (c) {
        return this.stateGlyphClass(this.flowCardState(c));
      },

      flowCardSr: function (c) {
        var w = (c && c.state_word) ? c.state_word : window.AB.stateWord(this.flowCardState(c));
        return w + ', ';
      },

      flowCardResult: function (c) {
        return (c && c.result) ? c.result : '—';
      },

      flowCardCounts: function (c) {
        return window.AB.countParts(c ? c.counts : null);
      },

      flowCardHasCounts: function (c) {
        return this.flowCardCounts(c).length > 0;
      },

      countSep: function (i) {
        return i > 0;
      },

      flowCardMetrics: function (c) {
        if (!c || !c.has_run) return '—';
        if (this.isHumanEngine(c.engine) && !c.engine_planned) {
          // A person's step has no tokens or cost: the wait time stands in.
          if (c.state === 'cho_nguoi') return 'đã chờ ' + window.AB.fmtWait(c.duration_ms);
          return window.AB.fmtWait(c.duration_ms) + ' · không có token';
        }
        return window.AB.fmtMinutes(c.duration_ms) + ' · ' + window.AB.fmtUsd(c.cost_usd);
      },

      // Engine line (D-14): the latest run's engine, or `dự kiến {engine}` before the card runs.
      flowCardEngine: function (c) {
        if (!c || !c.engine) return '';
        void this.staffVer;
        var shown = window.AB.engineText(c.engine);
        return c.engine_planned ? 'dự kiến ' + shown : shown;
      },

      flowCardHasEngine: function (c) {
        return this.flowCardEngine(c) !== '';
      },

      groupStopped: function (key) {
        var g = this.flowGroup(key);
        return Boolean(g && g.stopped);
      },

      groupFrameClass: function (key) {
        return this.groupStopped(key)
          ? 'ring-2 ring-amber-500 dark:ring-amber-400'
          : 'ring-1 ring-slate-200 dark:ring-slate-700';
      },

      groupFooter: function (key) {
        var g = this.flowGroup(key);
        if (!g || typeof g.round !== 'number') return 'chưa chạy';
        var text = '⟲︎ vòng ' + g.round;
        if (typeof g.cap === 'number') {
          text += ' / ' + g.cap;
        }
        return text;
      },

      groupStopWord: function (key) {
        var g = this.flowGroup(key);
        return (g && g.stopped && g.stop_word) ? g.stop_word : '';
      },

      // Bạn card (the gate)
      get gate() {
        return (this.pipe && this.pipe.gate) ? this.pipe.gate : null;
      },

      get gatePresent() {
        return Boolean(this.gate && this.gate.present);
      },

      get gateState() {
        return (this.gate && this.gate.state) ? this.gate.state : 'cho';
      },

      get gateClass() {
        var cls = 'flex h-full min-w-0 flex-col gap-1 rounded-xl border p-4 ' + window.AB.stateCard(this.gateState);
        if (this.isCurrentKey('ban')) {
          cls += ' ring-2 ring-indigo-600 dark:ring-indigo-400';
        }
        return cls;
      },

      get gateGlyph() {
        return window.AB.stepGlyph(this.gateState);
      },

      get gateGlyphClass() {
        return this.stateGlyphClass(this.gateState);
      },

      get gateSr() {
        var w = (this.gate && this.gate.state_word) ? this.gate.state_word : window.AB.stateWord(this.gateState);
        return w + ', ';
      },

      get gateResult() {
        return (this.gate && this.gate.result) ? this.gate.result : '—';
      },

      get gateTime() {
        var g = this.gate;
        if (!g || g.at_ms === null || g.at_ms === undefined) return '—';
        if (g.state === 'cho_ban') return 'từ ' + window.AB.fmtClock(g.at_ms);
        if (g.state === 'xong') return window.AB.fmtClock(g.at_ms);
        return '—';
      },

      gateCurrent: function () {
        return this.isCurrentKey('ban') ? 'step' : false;
      },

      get gateNote() {
        return (this.gate && this.gate.note) ? this.gate.note : '';
      },

      get gateDerivedTitle() {
        return (this.gate && this.gate.derived && this.gate.note) ? this.gate.note : false;
      },

      get flowAriaLabel() {
        var titles = [this.stageTitles.start];
        var self = this;
        function add(key) {
          var cs = self.groupCards(key);
          for (var i = 0; i < cs.length; i++) {
            if (cs[i] && cs[i].title) titles.push(cs[i].title);
          }
        }
        add('plan');
        if (this.gatePresent) titles.push(this.stageTitles.ban);
        add('build');
        add('qa');
        return 'Pipeline: ' + titles.join(', ');
      },
      // Pipeline card header (05.2): autopilot badge, toggle, workflow and budget
      // lines. Badge, toggle and workflow line read only the task-detail payload.
      get autoView() {
        return (this.task && this.task.auto) ? this.task.auto : null;
      },

      get autoState() {
        return this.autoView ? this.autoView.state : 'off';
      },

      get autoBadge() {
        return window.AB.autoBadge(this.autoView);
      },

      get autoBadgeText() {
        return this.autoBadge.text;
      },

      get autoBadgeClass() {
        return window.AB.stepClass(this.autoBadge.key);
      },

      get autoBadgeGlyph() {
        return window.AB.stepGlyph(this.autoBadge.key);
      },

      get autoBadgeGlyphClass() {
        return window.AB.stepGlyphClass(this.autoBadge.key);
      },

      get workflowLine() {
        if (!this.task) return '';
        var wf = this.task.workflow;
        if (!wf) return 'Quy trình: chưa có (task tạo trước Phase 5.2) — chạy tay từng bước.';
        return 'Quy trình: ' + wf.name + (wf.summary ? ' · ' + wf.summary : '');
      },

      get budgetView() {
        return (this.pipe && this.pipe.budget) ? this.pipe.budget : null;
      },

      get hasBudget() {
        return Boolean(this.budgetView);
      },

      get budgetText() {
        return window.AB.budgetParts(this.budgetView).text;
      },

      get budgetLevel() {
        return window.AB.budgetParts(this.budgetView).level;
      },

      get budgetNote() {
        return window.AB.budgetParts(this.budgetView).note;
      },

      get headerControls() {
        var cs = (this.task && this.task.controls) ? this.task.controls : [];
        return cs.filter(function (c) {
          return c && (c.id === 'auto_start' || c.id === 'auto_stop');
        });
      },

      get driverLost() {
        return this.autoState === 'lost';
      },

      // Stop banner (05.2): autopilot stopped with a reason. Heading, body and
      // reasons are server strings; the banner reads only the task detail.
      get showAutoStop() {
        return this.autoState === 'stopped';
      },

      // The slot also carries the quota stop (06-17), for manual and autopilot tasks alike.
      get showBanner() {
        return this.hasQuota || this.showAutoStop;
      },

      get bannerHeading() {
        if (this.hasQuota) return 'Hết quota engine ' + this.quotaView.engine;
        if (!this.showAutoStop) return '';
        var word = this.autoView.stop_word;
        return word ? 'Autopilot dừng · ' + word : 'Autopilot dừng';
      },

      get bannerBody() {
        if (this.hasQuota) return this.quotaPostponed ? this.quotaHint : (this.quotaView.detail || '');
        return (this.showAutoStop && this.autoView.stop_detail) ? this.autoView.stop_detail : '';
      },

      get stopGlyph() {
        return window.AB.stepGlyph('cho_ban');
      },

      get bannerIsBudget() {
        return Boolean(!this.hasQuota && this.showAutoStop && this.autoView.stop_reason === 'het_ngan_sach');
      },

      get bannerBudget() {
        return this.bannerIsBudget ? this.budgetText : '';
      },

      get bannerControls() {
        if (this.hasQuota) {
          if (this.quotaPostponed) return [];
          var qout = [];
          if (this.quotaRerunAction) qout.push(this.quotaRerunAction);
          qout.push({ id: 'quota_later', label: 'Thử lại sau', enabled: true, reason: null });
          var qend = this.controlById('auto_end');
          if (qend) qout.push(qend);
          return qout;
        }
        if (!this.showAutoStop) return [];
        var out = [];
        var more = this.controlById('auto_more_round');
        if (more) out.push(more);
        var acts = this.actionList;
        for (var i = 0; i < acts.length; i++) {
          if (acts[i] && acts[i].id === 'relead') {
            out.push(acts[i]);
            break;
          }
        }
        var end = this.controlById('auto_end');
        if (end) out.push(end);
        return out;
      },

      get hasBannerControls() {
        return this.bannerControls.length > 0;
      },

      moreRoundHelp: function (a) {
        if (a && a.id === 'auto_more_round' && this.bannerIsBudget) {
          return 'Chạy vượt ngân sách một lần, chỉ cho vòng này.';
        }
        return '';
      },

      autoHelper: function (a) {
        var st = this.autoState;
        if (st === 'running' || st === 'stopping') {
          return 'Run hiện tại chạy nốt rồi autopilot dừng. Muốn dừng ngay: Kill.';
        }
        if (st === 'lost') {
          return 'Autopilot đã dừng vì agent-brain serve tắt giữa chừng. Bật autopilot để chạy tiếp từ bước dở.';
        }
        if (st === 'gate') {
          return 'Autopilot dừng ở bước duyệt plan — bấm Duyệt plan để chạy tiếp.';
        }
        if (!a || a.id !== 'auto_start' || !a.enabled) return '';
        var wf = (this.pipe && this.pipe.workflow) ? this.pipe.workflow : null;
        if (!wf || typeof wf.budget_minutes !== 'number' || typeof wf.budget_usd !== 'number') return '';
        var usd = wf.budget_usd;
        var money = (Math.abs(Math.round(usd) - usd) < 1e-9) ? ('$' + Math.round(usd)) : ('$' + usd.toFixed(2));
        var limits = wf.budget_minutes + ' phút hoặc ' + money + ' cho cả task.';
        var hasLoops = Boolean(wf.loops && wf.loops.length > 0);
        var text = hasLoops
          ? 'Giới hạn: ' + wf.max_rounds + ' vòng mỗi bước lặp, ' + limits
          : 'Giới hạn: ' + limits;
        return text + ' Autopilot luôn dừng ở bước bạn duyệt plan.';
      },

      get headerHelper() {
        var cs = this.headerControls;
        if (cs.length > 0) return this.autoHelper(cs[0]);
        return this.driverLost ? this.autoHelper(null) : '';
      },

      get actionList() {
        return (this.task && this.task.actions) ? this.task.actions : [];
      },

      get hasActionList() {
        return this.actionList.length > 0;
      },

      get controlList() {
        return (this.task && this.task.controls) ? this.task.controls : [];
      },

      controlById: function (id) {
        var cs = this.controlList;
        for (var i = 0; i < cs.length; i++) {
          if (cs[i] && cs[i].id === id) return cs[i];
        }
        return null;
      },

      // The gate buttons sit in the Bạn card while the pipeline (and its gate)
      // is drawn; otherwise (pipeline failed, or no gate card) in the strip.
      get gateInCard() {
        return Boolean(this.pipe && this.gatePresent);
      },

      gateControlList: function () {
        return this.controlList.filter(function (c) {
          return c && (c.id === 'approve' || c.id === 'return_lead');
        });
      },

      get gateControls() {
        return this.gateInCard ? this.gateControlList() : [];
      },

      get hasGateControls() {
        return this.gateControls.length > 0;
      },

      // Action strip: task actions, plus the gate controls when they cannot be
      // drawn in the Bạn card; relead moves to the stop banner while it shows.
      get stripActions() {
        // Rendered elsewhere: the team line, the quota banner and the human callout.
        var elsewhere = { change_team: true, quota_rerun: true, human_done: true, human_return: true };
        var acts = this.actionList.filter(function (a) {
          return a && !elsewhere[a.id];
        });
        if (this.showAutoStop && !this.hasQuota) {
          acts = acts.filter(function (a) {
            return a && a.id !== 'relead';
          });
        }
        if (!this.gateInCard && !this.pipeLoading) {
          acts = this.gateControlList().concat(acts);
        }
        return acts;
      },

      get hasStripActions() {
        return this.stripActions.length > 0;
      },

      get approveShown() {
        var lists = [this.gateControls, this.stripActions];
        for (var i = 0; i < lists.length; i++) {
          for (var j = 0; j < lists[i].length; j++) {
            if (lists[i][j] && lists[i][j].id === 'approve') return true;
          }
        }
        return false;
      },

      actionId: function (a) {
        return 'ab-act-' + (a ? a.id : '');
      },

      get stripCaption() {
        var map = this.stageTitles;
        if (this.pipe && this.pipe.current_card && map[this.pipe.current_card]) {
          return 'Bước hiện tại: ' + map[this.pipe.current_card];
        }
        var t = this.task;
        var step = t ? t.current_step : null;
        var title = 'Lên plan';
        if (step === 'lead') {
          title = 'Lên plan';
        } else if (step === 'qa') {
          title = 'QA';
        } else if (step === 'dev') {
          title = (t && t.status === 'planned') ? 'Bạn' : 'Code';
        }
        return 'Bước hiện tại: ' + title;
      },

      // The quota banner's primary is on screen (06-17).
      get quotaRerunShown() {
        var cs = this.bannerControls;
        for (var i = 0; i < cs.length; i++) {
          if (cs[i] && cs[i].id === 'quota_rerun') return true;
        }
        return false;
      },

      // One primary per section: Duyệt plan > Chạy lại bước này bằng đội hình… (quota) >
      // Chạy thêm 1 vòng > first enabled action of the strip. Red stays Kill only.
      actionClass: function (a) {
        var primary = 'bg-indigo-600 text-white hover:bg-indigo-700';
        var secondary = 'text-slate-900 ring-1 ring-slate-300 hover:bg-slate-100 dark:text-slate-100 dark:ring-slate-600 dark:hover:bg-slate-700';
        if (!a) return secondary;
        if (a.id === 'kill') {
          return 'bg-rose-600 text-white hover:bg-rose-700';
        }
        if (a.id === 'approve') return primary;
        if (a.id === 'return_lead' || a.id === 'auto_start' || a.id === 'auto_stop' || a.id === 'auto_end' ||
            a.id === 'change_team' || a.id === 'quota_later') {
          return secondary;
        }
        if (this.approveShown) return secondary;
        if (a.id === 'quota_rerun') return primary;
        if (this.quotaRerunShown) return secondary;
        if (a.id === 'auto_more_round') return primary;
        if (this.controlById('auto_more_round') && this.showAutoStop) return secondary;
        var strip = this.stripActions;
        var firstEnabled = null;
        for (var i = 0; i < strip.length; i++) {
          if (strip[i] && strip[i].enabled) {
            firstEnabled = strip[i];
            break;
          }
        }
        if (firstEnabled && firstEnabled.id === a.id) return primary;
        return secondary;
      },

      actionDisabled: function (a) {
        return !a.enabled || Boolean(this.busyAction);
      },

      actionText: function (a) {
        if (this.busyAction === a.id) {
          if (a.id === 'auto_start') return 'Đang bật...';
          if (a.id === 'approve') return 'Đang duyệt...';
          if (a.id === 'kill' || a.id === 'auto_stop' || a.id === 'auto_end') return 'Đang dừng...';
          if (a.id === 'change_team') return 'Đang đổi...';
          return 'Đang giao...';
        }
        return a.label;
      },

      reasonId: function (a) {
        return 'ab-act-reason-' + a.id;
      },

      showReason: function (a) {
        return this.reasonText(a) !== '';
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
        if (!a) return;
        if (a.id === 'change_team') {
          this.openChangeTeam();
        } else if (a.id === 'quota_rerun') {
          this.openQuotaRerun();
        } else if (a.id === 'quota_later') {
          this.postponeQuota();
        } else if (a.id === 'kill' || a.id === 'relead') {
          this.lastFocusId = this.actionId(a);
          this.dialog = a;
          window.AB.lockScroll(true);
          window.AB.focusLater('ab-dlg-cancel');
        } else if (a.id === 'return_lead') {
          // Not destructive (the plan stays in git history): focus the note.
          this.lastFocusId = this.actionId(a);
          this.returnNote = '';
          this.dialog = a;
          window.AB.lockScroll(true);
          window.AB.focusLater('ab-return-note');
        } else if (this.controlPaths[a.id]) {
          this.postControl(a);
        } else {
          this.runAction(a);
        }
      },

      controlPaths: {
        approve: '/approve',
        return_lead: '/return',
        auto_start: '/auto',
        auto_stop: '/auto/stop',
        auto_more_round: '/auto/more-round',
        auto_end: '/auto/end',
      },

      controlSuccess: function (a, data) {
        switch (a.id) {
          case 'approve':
            return 'Đã duyệt plan lúc ' + window.AB.fmtClock(data.approved_at) + '.';
          case 'return_lead':
            return 'Đã trả plan về Lead (job ' + data.job_id + ').';
          case 'auto_start':
            return 'Đã bật autopilot.';
          case 'auto_stop':
            // 05.2-29: Dừng autopilot at the gate turns autopilot off ({ended: true}).
            if (data && data.ended) {
              return 'Đã tắt autopilot cho task này — Duyệt plan giờ chỉ ghi nhận, không tự chạy tiếp. Bấm Bật autopilot để bật lại.';
            }
            return 'Đã yêu cầu dừng autopilot — run hiện tại chạy nốt rồi dừng.';
          case 'auto_more_round':
            return 'Đã giao thêm 1 vòng.';
          case 'auto_end':
            return 'Đã dừng hẳn autopilot. Bật autopilot để chạy tiếp.';
        }
        return '';
      },

      // Task-detail controls (gate, autopilot): one click sends one POST with
      // the task's rev, so a click on stale data is refused (409 kind stale)
      // and the task reloads. A refused action is never retried.
      postControl: function (a) {
        var self = this;
        if (!a || !this.controlPaths[a.id] || this.busyAction) return;
        var path = '/api/tasks/' + this.id + this.controlPaths[a.id];
        var rev = (this.task && this.task.rev !== undefined) ? this.task.rev : null;
        var body = { rev: rev };
        if (a.id === 'return_lead') {
          var note = (this.returnNote || '').trim();
          if (note) body.note = note;
        }
        this.busyAction = a.id;
        return window.AB.api('POST', path, body).then(function (res) {
          self.busyAction = '';
          if (res && (res.status === 202 || res.status === 200) && res.data) {
            self.message = self.controlSuccess(a, res.data);
            self.load();
          } else if (res && res.status === 409 && res.data && res.data.error) {
            self.message = res.data.error;
            if (res.data.kind === 'stale') {
              self.load();
            }
          } else {
            self.message = window.AB.errorText(res);
          }
        });
      },

      closeDialog: function () {
        var id = this.lastFocusId;
        this.lastFocusId = '';
        this.dialog = null;
        window.AB.lockScroll(false);
        window.AB.focusLater(id || 'screen', 'screen');
      },

      dialogCancel: function () {
        this.closeDialog();
      },

      dialogOk: function () {
        var act = this.dialog;
        this.closeDialog();
        if (!act) return;
        if (act.id === 'return_lead') {
          this.postControl(act);
        } else {
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
        return (name === 'Vướng mắc' && this.blockedCause)
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

      glyph: function (r) {
        if (!r) return 'ⓘ\uFE0E';
        switch (r.kind) {
          case 'tool': return '▶\uFE0E';
          case 'assistant': return '✎\uFE0E';
          case 'tool_result': return '↩\uFE0E';
          case 'result': return '✔\uFE0E';
          case 'error': return '⚠\uFE0E';
          case 'system':
          default:
            return 'ⓘ\uFE0E';
        }
      },

      kindLabel: function (r) {
        return r ? r.kind : '';
      },

      kindPrefix: function (r) {
        return r ? (r.kind + ': ') : '';
      },

      rowTextClass: function (r) {
        if (!r) return 'text-slate-400';
        switch (r.kind) {
          case 'tool': return 'font-mono text-slate-200';
          case 'assistant': return 'text-slate-100';
          case 'tool_result': return 'font-mono text-slate-400';
          case 'result': return 'text-emerald-300';
          case 'error': return 'text-rose-300';
          case 'system':
          default:
            return 'text-slate-400';
        }
      },

      groupRight: function (g) {
        return (g && g.right) ? g.right : '';
      },

      groupHasRight: function (g) {
        return Boolean(g && g.right);
      },

      rowGlyph: function (r) {
        return window.AB.stepGlyph(r ? r.state : 'cho');
      },

      rowGlyphClass: function (r) {
        var state = r ? r.state : 'cho';
        var cls = 'inline-block w-6 text-center ' + window.AB.stateText(state);
        if (state === 'dang_chay') {
          cls += ' motion-safe:animate-pulse';
        }
        return cls;
      },

      rowVerdictClass: function (r) {
        var state = r ? r.state : 'cho';
        return 'font-semibold ' + window.AB.stateText(state);
      },

      // Counts first (review / QA, unknown counts omitted), then the 05.1 figures.
      // `Vòng {n} · {kind} · {engine}` (the server label plus the engine, D-14).
      rowLabel: function (r) {
        if (!r) return '';
        var base = r.label || '';
        if (!r.engine) return base;
        void this.staffVer;
        return base + ' · ' + window.AB.engineText(r.engine);
      },

      rowMetrics: function (r) {
        if (!r) return '— · — · —';
        var turns = (r.turns !== null && r.turns !== undefined) ? (r.turns + ' lượt') : '—';
        var figures = turns + ' · ' + window.AB.fmtMinutes(r.duration_ms) + ' · ' + window.AB.fmtUsd(r.cost_usd);
        if (r.human) {
          // Turns and cost do not apply to a person's run; the wait time is shown instead.
          figures = window.AB.fmtWait(r.duration_ms) + ' · không có token';
        }
        var counts = window.AB.countParts(r.counts);
        return counts.length > 0 ? counts.join(' · ') + ' · ' + figures : figures;
      },

      // Visible token line, only for the selected row (05.1-UI-REVIEW fix 2).
      tokenLineFor: function (r) {
        if (!this.isSelectedRow(r)) return '';
        if (r.human) return 'Token: không có — run do người làm.';
        if (!window.AB.tokensKnown(r)) return 'Token: vào — · ra — · cache — / —';
        var AB = window.AB;
        return 'Token: vào ' + AB.fmtNum(r.input_tokens) + ' · ra ' + AB.fmtNum(r.output_tokens) +
          ' · cache ' + AB.fmtNum(r.cache_read_tokens) + ' / ' + AB.fmtNum(r.cache_write_tokens);
      },

      rowTitle: function (r) {
        if (!r) return '— · vào — · ra — · cache — / —';
        var dur = window.AB.fmtDuration(r.duration_ms);
        if (r.human) return dur + ' · không có token';
        var tokens = window.AB.tokensKnown(r)
          ? ('vào ' + window.AB.fmtNum(r.input_tokens) + ' · ra ' + window.AB.fmtNum(r.output_tokens) + ' · cache ' + window.AB.fmtNum(r.cache_read_tokens) + ' / ' + window.AB.fmtNum(r.cache_write_tokens))
          : 'vào — · ra — · cache — / —';
        return dur + ' · ' + tokens;
      },

      isSelectedRow: function (r) {
        return Boolean(r && r.run_id === this.runId);
      },

      ledgerRowCurrent: function (r) {
        return this.isSelectedRow(r) ? 'true' : false;
      },

      ledgerRowClass: function (r) {
        return this.isSelectedRow(r)
          ? 'bg-indigo-50 ring-2 ring-indigo-600 dark:bg-indigo-900/20 dark:ring-indigo-400'
          : '';
      },

      pickRun: function (r) {
        if (!r) return;
        this.selectRun(r.run_id);
        var el = document.getElementById('ab-log-card');
        if (el && el.scrollIntoView) {
          el.scrollIntoView({ block: 'nearest' });
        }
      },

      showMoreRuns: function () {
        this.ledgerVisible += 20;
      }
    };
  });
})();

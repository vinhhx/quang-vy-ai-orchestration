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
      runId: null,

      pipe: null,
      pipeError: '',
      pipeStale: false,
      ledgerVisible: 20,
      _pipeSeq: 0,
      _pipeBusy: false,
      _pipeAgain: false,
      _left: false,

      stageTitles: {
        start: 'Bắt đầu',
        plan: 'Lên plan',
        ban: 'Bạn',
        code: 'Code',
        qa: 'QA',
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
        window.AB.api('GET', '/api/tasks/' + this.id).then(function (res) {
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
          this._pipeAgain = true;
          return;
        }
        this._pipeBusy = true;
        function finish() {
          self._pipeBusy = false;
          if (self._pipeAgain) {
            self._pipeAgain = false;
            self.loadPipeline();
          }
        }
        var seq = ++this._pipeSeq;
        window.AB.api('GET', '/api/tasks/' + this.id + '/pipeline').then(function (res) {
          if (seq === self._pipeSeq) {
            if (res && res.ok && res.data) {
              self.pipe = res.data;
              self.pipeError = '';
              self.pipeStale = false;
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
        return total.runs + ' lượt chạy claude · ' + window.AB.fmtMinutes(total.duration_ms) + ' · ' + window.AB.fmtUsd(total.cost_usd);
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

      cardState: function (key) {
        var c = this.card(key);
        if (c && c.state) return c.state;
        return key === 'start' ? 'xong' : 'cho';
      },

      isCurrentCard: function (key) {
        return Boolean(this.pipe && this.pipe.current === key);
      },

      cardCurrentAttr: function (key) {
        return this.isCurrentCard(key) ? 'step' : false;
      },

      cardClass: function (key) {
        var state = this.cardState(key);
        var cls = 'flex h-full min-w-0 flex-col gap-1 rounded-xl border p-4 ' + window.AB.stateCard(state);
        if (this.isCurrentCard(key)) {
          cls += ' ring-2 ring-indigo-600 dark:ring-indigo-400';
        }
        return cls;
      },

      cardGlyph: function (key) {
        return window.AB.stepGlyph(this.cardState(key));
      },

      cardGlyphClass: function (key) {
        var state = this.cardState(key);
        var cls = 'w-6 shrink-0 text-center ' + window.AB.stateText(state);
        if (state === 'dang_chay') {
          cls += ' motion-safe:animate-pulse';
        }
        return cls;
      },

      cardSr: function (key) {
        var c = this.card(key);
        var w = (c && c.state_word) ? c.state_word : window.AB.stateWord(this.cardState(key));
        return w + ', ';
      },

      cardResult: function (key) {
        var c = this.card(key);
        if (c && c.result) return c.result;
        if (key === 'start') return 'bạn đã giao';
        if (key === 'ban') return 'chưa có plan';
        return 'chưa bắt đầu';
      },

      cardHasRun: function (key) {
        var c = this.card(key);
        return Boolean(c && typeof c.rounds === 'number' && c.rounds > 0);
      },

      cardNoRun: function (key) {
        return !this.cardHasRun(key);
      },

      cardMinutes: function (key) {
        var c = this.card(key);
        return window.AB.fmtMinutes(c ? c.duration_ms : null);
      },

      cardCost: function (key) {
        var c = this.card(key);
        return window.AB.fmtUsd(c ? c.cost_usd : null);
      },

      cardTime: function (key) {
        var c = this.card(key);
        if (!c || c.at_ms === null || c.at_ms === undefined) return '—';
        if (key === 'start') {
          return window.AB.fmtClock(c.at_ms);
        }
        if (key === 'ban') {
          if (c.state === 'cho_ban') {
            return 'từ ' + window.AB.fmtClock(c.at_ms);
          }
          if (c.state === 'xong') {
            return window.AB.fmtClock(c.at_ms);
          }
          return '—';
        }
        return '—';
      },

      loopFooter: function (key) {
        var c = this.card(key);
        if (!c || typeof c.rounds !== 'number' || c.rounds <= 0) return 'chưa chạy';
        return '\u27F2\uFE0E vòng ' + c.rounds;
      },

      get actionList() {
        return (this.task && this.task.actions) ? this.task.actions : [];
      },

      get hasActionList() {
        return this.actionList.length > 0;
      },

      actionId: function (a) {
        return 'ab-act-' + (a ? a.id : '');
      },

      get stripCaption() {
        var map = this.stageTitles || {
          plan: 'Lên plan',
          ban: 'Bạn',
          code: 'Code',
          qa: 'QA',
        };
        if (this.pipe && this.pipe.current && map[this.pipe.current]) {
          return 'Bước hiện tại: ' + map[this.pipe.current];
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
          this.lastFocusId = this.actionId(a);
          this.dialog = a;
          window.AB.lockScroll(true);
          window.AB.focusLater('ab-dlg-cancel');
        } else {
          this.runAction(a);
        }
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

      rowMetrics: function (r) {
        if (!r) return '— · — · —';
        var turns = (r.turns !== null && r.turns !== undefined) ? (r.turns + ' lượt') : '—';
        return turns + ' · ' + window.AB.fmtMinutes(r.duration_ms) + ' · ' + window.AB.fmtUsd(r.cost_usd);
      },

      rowTitle: function (r) {
        if (!r) return '— · vào — · ra — · cache — / —';
        var dur = window.AB.fmtDuration(r.duration_ms);
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

// js/queue.js — Màn hình Điều phối (Orchestrator Console)
(function () {
  var SIDE_GROUPS = ['intake', 'approval', 'slot', 'running', 'stopped', 'cancelled', 'done'];

  window.AB.onReady('queue', function () {
    return {
      board: null,
      roomsReady: false,
      boardLoaded: false,
      boardError: '',

      tasks: [],
      tasksLoaded: false,
      tasksError: '',

      projects: [],
      projectsLoaded: false,

      // Nhân viên các phòng ban (GET /api/departments), vẽ trong văn phòng 3D
      departments: [],

      // Quick intake form
      quickRequest: '',
      quickProject: '',
      quickPriority: 'normal',
      quickSending: false,
      quickError: '',

      // Forms and state by ID
      editForms: {},
      rejectForms: {},
      expandedReasons: {},
      expandedErrors: {},
      actionBusy: {},
      cardErrors: {},

      // 3D Office state
      showOffice: true,
      hasWebGL: true,
      officeInstance: null,
      selectedItem: null,
      selectedItemType: null,
      selectedTaskId: null,
      // Room picked in the horizontal room row (UI only, highlights its button)
      selectedRoomId: '',

      // Right sidebar accordion: the one open group ('' = all collapsed).
      // Remembered per viewer in localStorage 'ab.queue.side'.
      sideGroup: 'intake',

      nowSec: Math.floor(Date.now() / 1000),

      init: function () {
        var self = this;
        this.loadSavedSettings();
        this.loadAll();
        this.loadProjects();

        // Tick timer for countdowns
        window.AB.every(1000, function () {
          self.nowSec = Math.floor(Date.now() / 1000);
        });

        // Background poll every 5s if no active form is being edited
        window.AB.every(5000, function () {
          if (!self.hasActiveForm() && !self.quickSending) {
            self.loadAll();
          }
        });

        var profileHandler = function () {
          self.loadTasks();
        };
        window.addEventListener('ab:profile', profileHandler);

        var queueChangedHandler = function () {
          self.loadAll();
        };
        window.addEventListener('ab:queue-changed', queueChangedHandler);

        var keyHandler = function (e) {
          if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
          if (window.AB && window.AB.screenModal && window.AB.screenModal.isOpen()) return;
          var tag = (e.target && e.target.tagName ? e.target.tagName.toLowerCase() : '');
          if (tag === 'input' || tag === 'textarea' || tag === 'select' || (e.target && e.target.isContentEditable)) {
            return;
          }
          if (e.key === 'n' || e.key === 'N') {
            e.preventDefault();
            self.openAssignModal();
          } else if (e.key === 'o' || e.key === 'O') {
            e.preventDefault();
            self.openOverviewModal();
          }
        };
        window.addEventListener('keydown', keyHandler);

        // Preload office-rooms and initialize 3D office
        // office-rooms.js sets window.ABOfficeRooms; that global is not reactive, so flip a
        // component field once it is there to make the room tree and labels re-render.
        import('/js/office-rooms.js').then(function () {
          self.roomsReady = true;
          self.updateOffice();
        }).catch(function () {});
        setTimeout(function () {
          self.initOffice();
        }, 80);

        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
          window.removeEventListener('ab:queue-changed', queueChangedHandler);
          window.removeEventListener('keydown', keyHandler);
          if (self.officeInstance) {
            self.officeInstance.dispose();
            self.officeInstance = null;
          }
        });
      },

      get loadedAttr() {
        return (this.boardLoaded && this.tasksLoaded) ? 'yes' : 'no';
      },

      openAssignModal: function () {
        if (window.AB && window.AB.screenModal) {
          window.AB.screenModal.open('assign');
        }
      },

      openOverviewModal: function () {
        if (window.AB && window.AB.screenModal) {
          window.AB.screenModal.open('overview');
        }
      },

      hasActiveForm: function () {
        for (var k in this.editForms) {
          if (this.editForms[k] && this.editForms[k].open) return true;
        }
        for (var j in this.rejectForms) {
          if (this.rejectForms[j] && this.rejectForms[j].open) return true;
        }
        return false;
      },

      loadAll: function () {
        var self = this;
        return Promise.all([this.loadBoard(), this.loadTasks(), this.loadDepartments()]).then(function () {
          self.updateOffice();
        });
      },

      // Lỗi đọc phòng ban không chặn màn Điều phối: giữ danh sách cũ.
      loadDepartments: function () {
        var self = this;
        return window.AB.api('GET', '/api/departments').then(function (res) {
          if (res && res.ok && res.data && Array.isArray(res.data.departments)) {
            self.departments = res.data.departments;
          }
        }).catch(function () {});
      },

      get staffList() {
        var helper = window.ABOfficeRooms;
        if (!this.roomsReady || !helper || !helper.staffFromDepartments) return [];
        return helper.staffFromDepartments(this.departments);
      },

      get totalStaffCount() {
        return this.staffList.length;
      },

      get totalStaffWorking() {
        return this.staffList.filter(function (m) { return m.working; }).length;
      },

      get staffSummaryText() {
        var n = this.totalStaffCount;
        if (n === 0) return '';
        var w = this.totalStaffWorking;
        return n + ' nhân viên' + (w > 0 ? ' · ' + w + ' đang làm' : '');
      },

      roomStaffText: function (r) {
        if (!r.staff) return '';
        return r.staff + ' NV' + (r.staffWorking > 0 ? ' · ' + r.staffWorking + ' làm' : '');
      },

      loadBoard: function () {
        var self = this;
        return window.AB.api('GET', '/api/orchestrator/board').then(function (res) {
          if (res && res.ok && res.data) {
            self.board = res.data;
            self.boardLoaded = true;
            self.boardError = '';
          } else {
            self.boardLoaded = true;
            self.boardError = window.AB.errorText(res);
          }
        });
      },

      loadTasks: function () {
        var self = this;
        var url = '/api/tasks?limit=100';
        var q = window.AB.profileQuery();
        if (q) {
          url += '&' + q;
        }
        return window.AB.api('GET', url).then(function (res) {
          if (res && res.ok && res.data) {
            self.tasks = res.data.tasks || [];
            self.tasksLoaded = true;
            self.tasksError = '';
          } else {
            self.tasksLoaded = true;
            self.tasksError = window.AB.errorText(res);
          }
        });
      },

      loadProjects: function () {
        var self = this;
        return window.AB.api('GET', '/api/projects').then(function (res) {
          if (res && res.ok && res.data && res.data.projects) {
            self.projects = res.data.projects;
            self.projectsLoaded = true;
          }
        });
      },

      // Quick intake submit
      submitQuickIntake: function () {
        var self = this;
        var text = (this.quickRequest || '').trim();
        if (!text) return;
        this.quickSending = true;
        this.quickError = '';

        var payload = {
          request: text,
          priority: this.quickPriority || 'normal',
        };
        if (this.quickProject) {
          payload.project = this.quickProject;
        }

        window.AB.api('POST', '/api/intake', payload).then(function (res) {
          self.quickSending = false;
          if (res && res.ok) {
            self.quickRequest = '';
            self.quickError = '';
            self.loadAll();
          } else {
            self.quickError = window.AB.errorText(res);
          }
        });
      },

      setQuickPriority: function (p) {
        this.quickPriority = p;
      },

      // Cancel intake
      cancelIntake: function (id) {
        var self = this;
        this.actionBusy[id] = true;
        this.cardErrors[id] = '';
        window.AB.api('POST', '/api/intake/' + id + '/cancel', {}).then(function (res) {
          self.actionBusy[id] = false;
          if (res && res.ok) {
            self.loadAll();
          } else {
            self.cardErrors[id] = window.AB.errorText(res);
          }
        });
      },

      // Retry a failed intake: back to the queue, the next orchestrator tick triages it again.
      retryIntake: function (id) {
        var self = this;
        this.actionBusy[id] = true;
        this.cardErrors[id] = '';
        window.AB.api('POST', '/api/intake/' + id + '/retry', {}).then(function (res) {
          self.actionBusy[id] = false;
          if (res && res.ok) {
            self.loadAll();
          } else {
            self.cardErrors[id] = window.AB.errorText(res);
          }
        });
      },

      // ---- Per-id state read through methods ----
      // Alpine x-bind turns an undefined result into '' when the expression has a '.'
      // (`actionBusy[it.id]` before the key exists), and '' keeps a boolean attribute such as
      // `disabled` on the element. Templates read per-id maps through these methods instead,
      // which always return a boolean or a string.
      isBusy: function (id) {
        return !!this.actionBusy[id];
      },

      cardError: function (id) {
        return this.cardErrors[id] || '';
      },

      // The Sửa rồi duyệt / Từ chối form objects of one proposal, created once and then only
      // mutated in place: the proposal card's `proposalForm` component keeps references to
      // them as `edit` / `reject`, so its x-model bindings use static paths (`edit.title`).
      formsFor: function (p) {
        var id = p.id;
        if (!this.editForms[id]) {
          this.editForms[id] = {
            open: false,
            title: p.title || '',
            team: p.team || '',
            workflow: p.workflow || '',
            priority: p.priority || 'normal',
            sending: false,
            error: '',
          };
        }
        if (!this.rejectForms[id]) {
          this.rejectForms[id] = {
            open: false,
            reason: '',
            sending: false,
            error: '',
          };
        }
        return { edit: this.editForms[id], reject: this.rejectForms[id] };
      },

      // Move keyboard focus into a form once x-show has revealed it. x-show reveals on a later
      // animation frame, so wait until the field is rendered (bounded retries).
      focusField: function (elId) {
        var tries = 0;
        var run = function () {
          var el = document.getElementById(elId);
          var rendered = !!(el && el.getClientRects && el.getClientRects().length);
          if (rendered && typeof el.focus === 'function') {
            el.focus();
            return;
          }
          tries++;
          if (tries < 20) {
            setTimeout(run, 25);
          }
        };
        setTimeout(run, 0);
      },

      // Approve proposal
      approveProposal: function (id) {
        var self = this;
        this.actionBusy[id] = true;
        this.cardErrors[id] = '';
        window.AB.api('POST', '/api/proposals/' + id + '/approve', {}).then(function (res) {
          self.actionBusy[id] = false;
          if (res && res.ok) {
            self.loadAll();
          } else {
            self.cardErrors[id] = window.AB.errorText(res);
          }
        });
      },

      // Edit form
      openEdit: function (p) {
        this.cardErrors[p.id] = '';
        var f = this.formsFor(p).edit;
        f.title = p.title || '';
        f.team = p.team || '';
        f.workflow = p.workflow || '';
        f.priority = p.priority || 'normal';
        f.sending = false;
        f.error = '';
        f.open = true;
        this.focusField('edit-title-' + p.id);
      },

      closeEdit: function (id) {
        if (this.editForms[id]) {
          this.editForms[id].open = false;
        }
      },

      setEditPriority: function (id, prio) {
        if (this.editForms[id]) {
          this.editForms[id].priority = prio;
        }
      },

      submitApproveEdit: function (id) {
        var self = this;
        var f = this.editForms[id];
        if (!f) return;
        f.sending = true;
        f.error = '';

        var body = {
          title: (f.title || '').trim(),
          team: f.team,
          workflow: f.workflow,
          priority: f.priority,
        };

        window.AB.api('POST', '/api/proposals/' + id + '/approve', body).then(function (res) {
          f.sending = false;
          if (res && res.ok) {
            f.open = false;
            self.loadAll();
          } else {
            f.error = window.AB.errorText(res);
          }
        });
      },

      // Reject form
      openReject: function (p) {
        this.cardErrors[p.id] = '';
        var r = this.formsFor(p).reject;
        r.reason = '';
        r.sending = false;
        r.error = '';
        r.open = true;
        this.focusField('reject-reason-' + p.id);
      },

      closeReject: function (id) {
        if (this.rejectForms[id]) {
          this.rejectForms[id].open = false;
        }
      },

      confirmReject: function (id) {
        var self = this;
        var r = this.rejectForms[id];
        if (!r) return;
        r.sending = true;
        r.error = '';

        var body = {};
        if (r.reason && r.reason.trim()) {
          body.reason = r.reason.trim();
        }

        window.AB.api('POST', '/api/proposals/' + id + '/reject', body).then(function (res) {
          r.sending = false;
          if (res && res.ok) {
            r.open = false;
            self.loadAll();
          } else {
            r.error = window.AB.errorText(res);
          }
        });
      },

      // Change priority
      changePriority: function (id, newPriority) {
        var self = this;
        this.actionBusy[id] = true;
        this.cardErrors[id] = '';
        window.AB.api('POST', '/api/proposals/' + id + '/priority', { priority: newPriority }).then(function (res) {
          self.actionBusy[id] = false;
          if (res && res.ok) {
            self.loadAll();
          } else {
            self.cardErrors[id] = window.AB.errorText(res);
          }
        });
      },

      changeLevel: function (id, newLevel) {
        var self = this;
        this.actionBusy[id] = true;
        this.cardErrors[id] = '';
        window.AB.api('POST', '/api/proposals/' + id + '/level', { level: newLevel }).then(function (res) {
          self.actionBusy[id] = false;
          if (res && res.ok) {
            self.loadAll();
          } else {
            self.cardErrors[id] = window.AB.errorText(res);
          }
        });
      },

      // Toggle helpers
      isLongError: function (it) {
        return !!(it && it.error && it.error.length > 90);
      },

      isLongReason: function (p) {
        return !!(p && p.reason && p.reason.length > 120);
      },

      toggleReason: function (id) {
        this.expandedReasons[id] = !this.expandedReasons[id];
      },

      isReasonExpanded: function (id) {
        return !!this.expandedReasons[id];
      },

      toggleError: function (id) {
        this.expandedErrors[id] = !this.expandedErrors[id];
      },

      isErrorExpanded: function (id) {
        return !!this.expandedErrors[id];
      },

      // Quota stops lookup
      quotaStopsMap: function () {
        var map = {};
        if (this.board && this.board.quota_stops) {
          for (var i = 0; i < this.board.quota_stops.length; i++) {
            var qs = this.board.quota_stops[i];
            map[qs.task_id] = qs;
          }
        }
        return map;
      },

      // Single-column task classification: Đã hủy > Dừng > Đang chạy > Cần duyệt > Đã xong
      classifyTask: function (t) {
        var qMap = this.quotaStopsMap();
        // "Dừng hẳn" (task hủy) has its own group, ahead of the stopped one.
        if (t.group === 'cancelled' || t.auto_state === 'cancelled') {
          return 'cancelled';
        }
        if (t.quota_detail || qMap[t.id] || t.auto_state === 'stopped' || t.group === 'attention') {
          return 'stopped';
        }
        if (t.group === 'running') {
          return 'running';
        }
        if (t.group === 'waiting') {
          return 'waiting';
        }
        if (t.group === 'done') {
          return 'done';
        }
        return 'other';
      },

      countdownText: function (qs) {
        if (!qs || !qs.resume_at) return 'Chờ bạn chạy lại';
        var resumeSec = qs.resume_at < 10000000000 ? qs.resume_at : Math.floor(qs.resume_at / 1000);
        var diff = resumeSec - this.nowSec;
        if (diff <= 0) return 'Đang chạy lại...';
        if (diff < 60) return 'Tự chạy lại sau ' + diff + ' giây';
        var m = Math.floor(diff / 60);
        var s = diff % 60;
        return 'Tự chạy lại sau ' + m + ' phút ' + (s < 10 ? '0' : '') + s + ' giây';
      },

      // Style bindings return objects: Alpine applies them with style.setProperty, which the
      // dashboard CSP (style-src 'self') allows; a string would go through setAttribute('style').
      roomDotStyle: function (r) {
        return { backgroundColor: r.color };
      },

      // Horizontal room row: the picked room stands out, the others stay quiet.
      roomPressed: function (r) {
        return this.selectedRoomId === r.id ? 'true' : 'false';
      },

      roomButtonClass: function (r) {
        if (this.selectedRoomId === r.id) {
          return 'bg-indigo-50 text-indigo-900 ring-indigo-400 dark:bg-indigo-950/50 dark:text-indigo-100 dark:ring-indigo-500';
        }
        return 'bg-slate-50 text-slate-800 ring-slate-200 hover:bg-white dark:bg-slate-900/40 dark:text-slate-200 dark:ring-slate-700 dark:hover:bg-slate-800/80';
      },

      roomTitle: function (r) {
        var t = r.name + ': ' + r.items.length + ' việc';
        if (r.staff) {
          t += ', ' + r.staff + ' nhân viên (' + r.staffWorking + ' đang làm)';
        }
        return t;
      },

      progressStyle: function (task) {
        return { width: this.stepProgress(task).pct + '%' };
      },

      stepProgress: function (task) {
        var steps = task.steps || [];
        if (steps.length === 0) return { done: 0, total: 0, pct: 0, text: '0/0 bước' };
        var done = 0;
        for (var i = 0; i < steps.length; i++) {
          var st = steps[i].state;
          if (st === 'xong' || st === 'duyet' || st === 'dat') {
            done++;
          }
        }
        var total = steps.length;
        var pct = total > 0 ? Math.round((done / total) * 100) : 0;
        return {
          done: done,
          total: total,
          pct: pct,
          text: done + '/' + total + ' bước',
        };
      },

      fmtTime: function (ms) {
        if (!ms) return '—';
        return window.AB.fmtTime(ms);
      },

      // Getters for template
      get isOrchestratorEnabled() {
        return !!(this.board && this.board.status && this.board.status.enabled);
      },

      get slotText() {
        if (!this.board || !this.board.status) return 'Slot: —';
        var active = this.board.status.active || 0;
        var max = this.board.status.max_parallel || 0;
        return 'Slot: ' + active + '/' + max;
      },

      get hasAutoResume() {
        return !!(this.board && this.board.status && this.board.status.auto_resume_quota);
      },

      get autoResumeText() {
        if (!this.hasAutoResume) return '';
        var mins = this.board.status.resume_after_minutes || 60;
        return 'Tự chạy lại sau hết quota: ' + mins + ' phút';
      },

      get availableTeams() {
        return (this.board && this.board.teams) || ['mac-dinh', 'tiet-kiem'];
      },

      get availableWorkflows() {
        return (this.board && this.board.workflows) || ['mac-dinh', 'nhanh'];
      },

      // 3D Office methods
      loadSavedSettings: function () {
        try {
          var val = localStorage.getItem('ab.queue.showOffice');
          this.showOffice = (val !== 'false');
        } catch (e) {
          this.showOffice = true;
        }
        try {
          var side = localStorage.getItem('ab.queue.side');
          if (side === 'none') {
            this.sideGroup = '';
          } else if (SIDE_GROUPS.indexOf(side) !== -1) {
            this.sideGroup = side;
          }
        } catch (e) {}
      },

      // ---- Right sidebar accordion (UI only) ----
      setSide: function (g) {
        if (SIDE_GROUPS.indexOf(g) === -1) return;
        this.sideGroup = (this.sideGroup === g) ? '' : g;
        try {
          localStorage.setItem('ab.queue.side', this.sideGroup || 'none');
        } catch (e) {}
      },

      sideOpen: function (g) {
        return this.sideGroup === g;
      },

      sideExpanded: function (g) {
        return this.sideGroup === g ? 'true' : 'false';
      },

      sideGroupClass: function (g) {
        return this.sideGroup === g ? 'min-h-0 flex-1' : 'shrink-0';
      },

      sideHeaderClass: function (g) {
        if (this.sideGroup === g) {
          return 'bg-indigo-50 text-indigo-900 dark:bg-indigo-950/40 dark:text-indigo-100';
        }
        return 'text-slate-900 hover:bg-slate-50 dark:text-slate-100 dark:hover:bg-slate-700/60';
      },

      sideChevronClass: function (g) {
        return this.sideGroup === g ? 'rotate-90' : '';
      },

      sideCount: function (g) {
        if (g === 'intake') return this.countIntake;
        if (g === 'approval') return this.countApproval;
        if (g === 'slot') return this.countSlot;
        if (g === 'running') return this.countRunning;
        if (g === 'stopped') return this.countStopped;
        if (g === 'cancelled') return this.countCancelled;
        if (g === 'done') return this.countDone;
        return 0;
      },

      // Accents: Dừng rose and Cần duyệt amber when non-zero, Đang chạy indigo when non-zero.
      countBadgeClass: function (g) {
        var n = this.sideCount(g);
        if (n > 0 && g === 'stopped') {
          return 'bg-rose-600 text-white dark:bg-rose-500';
        }
        if (n > 0 && g === 'approval') {
          return 'bg-amber-400 text-amber-950 dark:bg-amber-400 dark:text-amber-950';
        }
        if (n > 0 && g === 'running') {
          return 'bg-indigo-600 text-white dark:bg-indigo-500';
        }
        return 'bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-300';
      },

      get countIntake() {
        return this.intakeItems.length;
      },

      get countApproval() {
        return this.approvalItems.length;
      },

      get countSlot() {
        return this.waitingSlotItems.length;
      },

      get countRunning() {
        return this.runningTasks.length;
      },

      get countStopped() {
        return this.stoppedTasks.length;
      },

      get countCancelled() {
        return this.cancelledTasks.length;
      },

      get countDone() {
        return this.allDoneTasks.length;
      },

      toggleOffice: function () {
        var self = this;
        this.showOffice = !this.showOffice;
        try {
          localStorage.setItem('ab.queue.showOffice', this.showOffice ? 'true' : 'false');
        } catch (e) {}

        if (this.showOffice) {
          setTimeout(function () {
            self.initOffice();
          }, 50);
        } else {
          if (this.officeInstance) {
            this.officeInstance.dispose();
            this.officeInstance = null;
          }
        }
      },

      initOffice: function () {
        var self = this;
        if (!this.showOffice) return;

        var container = document.getElementById('office3d-canvas-container');
        if (!container) return;

        var testCanvas = document.createElement('canvas');
        var gl = testCanvas.getContext('webgl2') || testCanvas.getContext('webgl');
        if (!gl) {
          self.hasWebGL = false;
          return;
        }
        self.hasWebGL = true;

        if (self.officeInstance) {
          self.updateOffice();
          return;
        }

        import('/js/office3d.js')
          .then(function (mod) {
            if (!self.showOffice) return;
            var c = document.getElementById('office3d-canvas-container');
            if (!c) return;

            self.officeInstance = mod.createOffice(c, {
              onSelect: function (item, type) {
                self.selectedItem = item;
                self.selectedItemType = type;
                self.selectedTaskId = item ? item.id : null;
              },
            });
            self.updateOffice();
          })
          .catch(function (err) {
            console.warn('Không thể khởi tạo văn phòng 3D:', err);
          });
      },

      updateOffice: function () {
        if (this.officeInstance && this.showOffice) {
          this.officeInstance.update({
            board: this.board,
            tasks: this.tasks,
            projects: this.projects,
            staff: this.staffList,
          });
        }
      },

      focusOfficeTask: function (id) {
        this.focusTask(id);
      },

      focusTask: function (id) {
        this.selectedTaskId = id;
        if (this.officeInstance) {
          this.officeInstance.focusTask(id);
        }
      },

      focusRoom: function (roomId) {
        this.selectedRoomId = roomId;
        if (this.officeInstance) {
          this.officeInstance.focusTask(roomId);
        }
      },

      resetOfficeCamera: function () {
        this.selectedRoomId = '';
        if (this.officeInstance) {
          this.officeInstance.focusTask(0);
        }
        this.closeDetailCard();
      },

      closeDetailCard: function () {
        this.selectedItem = null;
        this.selectedItemType = null;
        this.selectedTaskId = null;
        if (this.officeInstance && this.officeInstance.selectCharacter) {
          this.officeInstance.selectCharacter(null);
        }
      },

      // The 3D detail card opens the form on the proposal's card in the Cần duyệt group,
      // so that group is opened first.
      openApprovalGroup: function () {
        if (this.sideGroup !== 'approval') {
          this.setSide('approval');
        }
      },

      openEditFromDetail: function (p) {
        this.openApprovalGroup();
        this.openEdit(p);
      },

      openRejectFromDetail: function (p) {
        this.openApprovalGroup();
        this.openReject(p);
      },

      get totalOfficeCount() {
        var count = 0;
        var rList = this.officeRoomList;
        for (var i = 0; i < rList.length; i++) {
          count += rList[i].items.length;
        }
        return count;
      },

      get officeRoomList() {
        var helper = window.ABOfficeRooms;
        if (!this.roomsReady || !helper) return [];
        var rooms = helper.ROOMS || [];
        var board = this.board || {};
        var tasks = this.tasks || [];
        var projects = this.projects || [];

        var roomItemsMap = {};
        for (var i = 0; i < rooms.length; i++) {
          roomItemsMap[rooms[i].id] = [];
        }

        for (var t = 0; t < tasks.length; t++) {
          var task = tasks[t];
          var placed = helper.placeTask(task, board, projects);
          var rid = placed.room || 'dieu_phoi';
          if (roomItemsMap[rid]) {
            roomItemsMap[rid].push({
              id: task.id,
              type: 'task',
              title: task.title,
              project: task.project_name || task.project,
              step: task.current_step,
              item: task,
            });
          }
        }

        if (board.proposals && Array.isArray(board.proposals)) {
          for (var p = 0; p < board.proposals.length; p++) {
            var prop = board.proposals[p];
            if (prop.status === 'rejected') continue;
            var placedProp = helper.placeProposal(prop);
            var prid = placedProp.room;
            if (roomItemsMap[prid]) {
              roomItemsMap[prid].push({
                id: prop.id,
                type: 'proposal',
                title: prop.title,
                project: prop.project_name || prop.project,
                item: prop,
              });
            }
          }
        }

        if (board.intake && Array.isArray(board.intake)) {
          for (var k = 0; k < board.intake.length; k++) {
            var it = board.intake[k];
            if (it.status === 'cancelled' || it.status === 'converted') continue;
            var placedIt = helper.placeIntake(it);
            var itid = placedIt.room;
            if (roomItemsMap[itid]) {
              roomItemsMap[itid].push({
                id: it.id,
                type: 'intake',
                title: it.request || 'Intake #' + it.id,
                project: it.project_name || it.project,
                item: it,
              });
            }
          }
        }

        var staff = this.staffList;
        return rooms.map(function (r) {
          var inRoom = staff.filter(function (m) { return m.room === r.id; });
          return {
            id: r.id,
            name: r.name,
            color: r.color,
            items: roomItemsMap[r.id] || [],
            staff: inRoom.length,
            staffWorking: inRoom.filter(function (m) { return m.working; }).length,
          };
        });
      },

      get selectedItemTitle() {
        if (!this.selectedItem) return '';
        return this.selectedItem.title || this.selectedItem.request || this.selectedItem.slug || 'Chi tiết';
      },

      get selectedItemProject() {
        if (!this.selectedItem) return '—';
        return this.selectedItem.project_name || this.selectedItem.project || 'Chung';
      },

      get selectedItemStep() {
        if (!this.selectedItem) return '';
        return this.selectedItem.current_step || '';
      },

      get selectedItemRoomName() {
        if (!this.selectedItem) return '';
        var helper = window.ABOfficeRooms;
        if (!this.roomsReady || !helper) return '';
        var placed;
        if (this.selectedItemType === 'proposal') {
          placed = helper.placeProposal(this.selectedItem);
        } else if (this.selectedItemType === 'intake') {
          placed = helper.placeIntake(this.selectedItem);
        } else {
          placed = helper.placeTask(this.selectedItem, this.board, this.projects);
        }
        var room = helper.ROOMS_BY_ID && helper.ROOMS_BY_ID[placed.room];
        return room ? room.name : '';
      },

      get selectedItemReason() {
        if (!this.selectedItem) return '';
        return this.selectedItem.quota_detail || this.selectedItem.stop_reason || '';
      },

      get selectedItemBadgeClass() {
        if (!this.selectedItem) return '';
        if (this.selectedItemType === 'proposal') {
          return this.selectedItem.priority === 'urgent'
            ? 'bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200'
            : 'bg-purple-100 text-purple-800 dark:bg-purple-900/40 dark:text-purple-200';
        }
        if (this.selectedItemType === 'intake') {
          return 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200';
        }
        return 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-200';
      },

      get selectedItemBadgeText() {
        if (!this.selectedItem) return '';
        if (this.selectedItemType === 'proposal') {
          return 'Đề xuất' + (this.selectedItem.priority === 'urgent' ? ' · Gấp' : '');
        }
        if (this.selectedItemType === 'intake') {
          return 'Hàng chờ';
        }
        return 'Task #' + this.selectedItem.id;
      },

      // Column 1: Hàng chờ
      get intakeItems() {
        return (this.board && this.board.intake) || [];
      },

      // Column 2: Cần bạn duyệt
      // Sorted: urgent proposals first, then tasks waiting for plan, then other proposals
      get approvalItems() {
        var self = this;
        var proposals = (this.board && this.board.proposals) || [];
        var pending = proposals.filter(function (p) {
          return p.status === 'pending';
        });
        var urgentProps = pending.filter(function (p) {
          return p.priority === 'urgent';
        });
        var otherProps = pending.filter(function (p) {
          return p.priority !== 'urgent';
        });
        var waitingTasks = (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'waiting';
        });

        var out = [];
        for (var i = 0; i < urgentProps.length; i++) {
          out.push({ kind: 'proposal', data: urgentProps[i] });
        }
        for (var j = 0; j < waitingTasks.length; j++) {
          out.push({ kind: 'task', data: waitingTasks[j] });
        }
        for (var k = 0; k < otherProps.length; k++) {
          out.push({ kind: 'proposal', data: otherProps[k] });
        }
        return out;
      },

      // Column 3: Chờ slot (proposals approved)
      get waitingSlotItems() {
        var proposals = (this.board && this.board.proposals) || [];
        var approved = proposals.filter(function (p) {
          return p.status === 'approved';
        });
        return approved.map(function (p, index) {
          return { item: p, order: index + 1 };
        });
      },

      // Column 4: Đang chạy
      get runningTasks() {
        var self = this;
        return (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'running';
        });
      },

      // Column 5: Dừng
      get stoppedTasks() {
        var self = this;
        var qMap = this.quotaStopsMap();
        var list = (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'stopped';
        });
        return list.map(function (t) {
          var qs = qMap[t.id] || null;
          var isQuota = !!(t.quota_detail || qs);
          var engine = (qs && qs.engine) || 'claude';
          return {
            task: t,
            isQuota: isQuota,
            engine: engine,
            quotaStop: qs,
          };
        });
      },

      // Recent items
      get recentItems() {
        return (this.board && this.board.recent) || [];
      },

      // Cancelled tasks (Dừng hẳn): sidebar group Đã hủy
      get cancelledTasks() {
        var self = this;
        return (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'cancelled';
        });
      },

      // All done tasks among the loaded ones (sidebar group Đã xong)
      get allDoneTasks() {
        var self = this;
        return (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'done';
        });
      },

      // 10 latest done tasks
      get doneTasks() {
        return this.allDoneTasks.slice(0, 10);
      },
    };
  });

  // One pending proposal card in Cần duyệt (x-data="proposalForm" inside the approvalItems loop).
  // It only aliases that proposal's form objects from the queue component under static names,
  // because the CSP build's templates cannot safely use `editForms[item.data.id].title`
  // (it throws before the form exists, and x-model needs an assignable path). The queue
  // component stays the owner: its methods open, submit and close the forms, and its
  // background poll still sees an open form through editForms / rejectForms.
  window.AB.onReady('proposalForm', function () {
    return {
      edit: { open: false, title: '', team: '', workflow: '', priority: 'normal', sending: false, error: '' },
      reject: { open: false, reason: '', sending: false, error: '' },

      init: function () {
        var forms = this.formsFor(this.item.data);
        this.edit = forms.edit;
        this.reject = forms.reject;
      },

      editPriorityPressed: function (prio) {
        return this.edit.priority === prio ? 'true' : 'false';
      },

      editPriorityClass: function (prio) {
        if (this.edit.priority !== prio) {
          return 'bg-white dark:bg-slate-800 ring-1 ring-slate-300 dark:ring-slate-600';
        }
        if (prio === 'urgent') return 'bg-rose-600 text-white';
        if (prio === 'low') return 'bg-sky-600 text-white';
        return 'bg-indigo-600 text-white';
      },
    };
  });
})();

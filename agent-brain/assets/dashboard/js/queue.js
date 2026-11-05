// js/queue.js — Màn hình Điều phối (Orchestrator Console)
(function () {
  window.AB.onReady('queue', function () {
    return {
      board: null,
      boardLoaded: false,
      boardError: '',

      tasks: [],
      tasksLoaded: false,
      tasksError: '',

      projects: [],
      projectsLoaded: false,

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

        // Preload office-rooms and initialize 3D office
        if (typeof import === 'function' || true) {
          import('/js/office-rooms.js').catch(function () {});
        }
        setTimeout(function () {
          self.initOffice();
        }, 80);

        window.AB.onLeave(function () {
          window.removeEventListener('ab:profile', profileHandler);
          if (self.officeInstance) {
            self.officeInstance.dispose();
            self.officeInstance = null;
          }
        });
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
        return Promise.all([this.loadBoard(), this.loadTasks()]).then(function () {
          self.updateOffice();
        });
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
        this.editForms[p.id] = {
          open: true,
          title: p.title || '',
          team: p.team || '',
          workflow: p.workflow || '',
          priority: p.priority || 'normal',
          sending: false,
          error: '',
        };
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
        this.rejectForms[p.id] = {
          open: true,
          reason: '',
          sending: false,
          error: '',
        };
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

      // Single-column task classification: Dừng > Đang chạy > Cần duyệt > Đã xong
      classifyTask: function (t) {
        var qMap = this.quotaStopsMap();
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
        if (this.officeInstance) {
          this.officeInstance.focusTask(roomId);
        }
      },

      resetOfficeCamera: function () {
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

      openEditFromDetail: function (p) {
        this.openEdit(p.id, p);
      },

      openRejectFromDetail: function (p) {
        this.openReject(p.id);
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
        if (!helper) return [];
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

        return rooms.map(function (r) {
          return {
            id: r.id,
            name: r.name,
            color: r.color,
            items: roomItemsMap[r.id] || [],
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
        if (!helper) return '';
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

      // 10 latest done tasks
      get doneTasks() {
        var self = this;
        var list = (this.tasks || []).filter(function (t) {
          return self.classifyTask(t) === 'done';
        });
        return list.slice(0, 10);
      },
    };
  });
})();

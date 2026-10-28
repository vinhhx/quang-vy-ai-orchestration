// js/telegram.js — Telegram screen component (read-only status of the bot, plan 06.2-02)
(function () {
  var REFRESH_MS = 5000;
  var SETUP_COMMAND = 'agent-brain telegram setup';
  var OFF_TEXT = 'Bot Telegram chưa bật. Chạy "' + SETUP_COMMAND + '" trong terminal để ghép cặp bot, rồi khởi động lại agent-brain serve.';

  // state -> badge word and the vocabulary key whose colour and glyph it shares with the
  // readiness badges of Cài đặt (stepClass / stepGlyph in app.js).
  var STATES = {
    off: { word: 'Chưa cấu hình', key: 'chua_tu_dong' },
    error: { word: 'Lỗi cấu hình', key: 'loi' },
    starting: { word: 'Đang khởi động', key: 'cho_nguoi' },
    running: { word: 'Đang chạy', key: 'san_sang' },
    degraded: { word: 'Mất kết nối', key: 'vuong_mac' },
  };

  window.AB.onReady('telegram', function () {
    return {
      status: null,
      loaded: false,
      error: '',
      stale: false,

      init: function () {
        var self = this;
        this.load();
        // AB.every timers are cleared by the router when the route changes.
        window.AB.every(REFRESH_MS, function () {
          self.load();
        });
      },

      load: function () {
        var self = this;
        window.AB.api('GET', '/api/telegram').then(function (res) {
          if (res.ok && res.data) {
            self.status = res.data;
            self.loaded = true;
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

      reload: function () {
        this.error = '';
        this.load();
      },

      get loadedAttr() {
        return this.loaded ? 'yes' : 'no';
      },

      get isLoading() {
        return !this.loaded && !this.error;
      },

      get hasError() {
        return Boolean(this.error);
      },

      get errorBody() {
        return this.error + '. Trang tự thử lại; nếu vẫn lỗi, kiểm tra terminal đang chạy agent-brain serve.';
      },

      get hasStatus() {
        return this.loaded && Boolean(this.status);
      },

      get stateKey() {
        var s = this.status ? this.status.state : '';
        return STATES[s] ? s : 'off';
      },

      get stateWord() {
        return STATES[this.stateKey].word;
      },

      get stateBadgeClass() {
        return window.AB.stepClass(STATES[this.stateKey].key);
      },

      get stateGlyph() {
        return window.AB.stepGlyph(STATES[this.stateKey].key);
      },

      get isOff() {
        return this.hasStatus && this.stateKey === 'off';
      },

      get offText() {
        return OFF_TEXT;
      },

      get setupCommand() {
        return SETUP_COMMAND;
      },

      get isActive() {
        var k = this.stateKey;
        return k === 'starting' || k === 'running' || k === 'degraded';
      },

      get hasMessage() {
        var k = this.stateKey;
        return this.hasStatus && (k === 'error' || k === 'degraded') && Boolean(this.status.message);
      },

      get messageText() {
        return (this.status && this.status.message) ? String(this.status.message) : '';
      },

      get messageClass() {
        if (this.stateKey === 'error') {
          return 'rounded-xl bg-rose-50 p-4 ring-1 ring-rose-300 text-rose-900 dark:bg-rose-900/20 dark:ring-rose-700 dark:text-rose-200';
        }
        return 'rounded-xl bg-amber-50 p-4 ring-1 ring-amber-300 text-amber-900 dark:bg-amber-900/20 dark:ring-amber-700 dark:text-amber-200';
      },

      get limitText() {
        var s = this.status;
        if (!s || s.tasks_today === null || s.tasks_today === undefined) return '';
        var limit = s.tasks_per_day_limit;
        var text = window.AB.fmtNum(s.tasks_today) + '/' + window.AB.fmtNum(limit);
        // telegram_max_tasks_per_day = 0 turns /task off; it does not mean "no limit".
        if (limit === 0) text += ' (giao việc từ Telegram đang tắt)';
        return text;
      },

      // Rows appear only when there is data for them.
      get rows() {
        var s = this.status;
        var out = [];
        if (!this.hasStatus) return out;
        function known(v) {
          return v !== null && v !== undefined;
        }
        if (s.bot_username) {
          out.push({ id: 'bot', label: 'Bot', value: '@' + s.bot_username });
        }
        if (known(s.chat_id)) {
          out.push({ id: 'chat', label: 'Chat ID', value: String(s.chat_id) });
        }
        if (known(s.paired_at) && s.paired_at > 0) {
          out.push({ id: 'paired', label: 'Đã ghép cặp lúc', value: window.AB.fmtTime(s.paired_at) });
        }
        if (known(s.last_ok_at)) {
          out.push({ id: 'last-ok', label: 'Lần liên lạc thành công gần nhất', value: window.AB.fmtTime(s.last_ok_at) });
        }
        if (this.isActive && known(s.outbox_pending)) {
          out.push({ id: 'outbox', label: 'Tin chờ gửi', value: window.AB.fmtNum(s.outbox_pending) });
        }
        if (this.stateKey !== 'off' && this.limitText) {
          out.push({ id: 'tasks', label: 'Task giao qua Telegram hôm nay', value: this.limitText });
        }
        if (s.config_path) {
          out.push({ id: 'config', label: 'Tệp cấu hình', value: String(s.config_path) });
        }
        return out;
      },
    };
  });
})();

// js/app.js — deferred, before Alpine
(function () {
  var BADGE = 'inline-flex items-center gap-1 rounded-full px-2 py-1 text-sm font-semibold';

  var STEP_CLASSES = {
    cho: BADGE + ' bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-200',
    dang_chay: BADGE + ' bg-blue-100 text-blue-800 dark:bg-blue-900/50 dark:text-blue-200',
    xong: BADGE + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200',
    vuong_mac: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    can_sua: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    killed: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
    loi: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
    vi_pham: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
    cho_ban: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    dat: BADGE + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200',
    duyet: BADGE + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200',
    sua_lai: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    chua_tu_dong: BADGE + ' border border-dashed border-slate-400 text-slate-600 dark:border-slate-500 dark:text-slate-400',
  };

  var STEP_GLYPHS = {
    cho: '◌\uFE0E',
    dang_chay: '◐\uFE0E',
    xong: '✓\uFE0E',
    vuong_mac: '!\uFE0E',
    can_sua: '!\uFE0E',
    killed: '■\uFE0E',
    loi: '■\uFE0E',
    vi_pham: '■\uFE0E',
    cho_ban: '●\uFE0E',
    dat: '✓\uFE0E',
    duyet: '✓\uFE0E',
    sua_lai: '!\uFE0E',
    chua_tu_dong: '◌\uFE0E',
  };

  var STATE_TEXT = {
    xong: 'text-emerald-700 dark:text-emerald-300',
    dat: 'text-emerald-700 dark:text-emerald-300',
    duyet: 'text-emerald-700 dark:text-emerald-300',
    dang_chay: 'text-blue-700 dark:text-blue-300',
    cho_ban: 'text-amber-800 dark:text-amber-300',
    vuong_mac: 'text-amber-800 dark:text-amber-300',
    can_sua: 'text-amber-800 dark:text-amber-300',
    sua_lai: 'text-amber-800 dark:text-amber-300',
    killed: 'text-rose-700 dark:text-rose-300',
    loi: 'text-rose-700 dark:text-rose-300',
    vi_pham: 'text-rose-700 dark:text-rose-300',
    cho: 'text-slate-600 dark:text-slate-400',
    chua_tu_dong: 'text-slate-600 dark:text-slate-400',
  };

  var STATE_CARD = {
    xong: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
    dat: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
    duyet: 'border-emerald-300 bg-emerald-50 dark:border-emerald-700 dark:bg-emerald-900/20',
    dang_chay: 'border-blue-300 bg-blue-50 dark:border-blue-700 dark:bg-blue-900/20',
    cho_ban: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
    vuong_mac: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
    can_sua: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
    sua_lai: 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-900/20',
    killed: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
    loi: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
    vi_pham: 'border-rose-300 bg-rose-50 dark:border-rose-700 dark:bg-rose-900/20',
    cho: 'border-dashed border-slate-500 bg-white dark:border-slate-400 dark:bg-slate-800',
    chua_tu_dong: 'border-dashed border-slate-400 bg-transparent dark:border-slate-500 dark:bg-transparent',
  };

  var STATE_WORDS = {
    xong: 'xong',
    dat: 'đạt',
    duyet: 'duyệt',
    dang_chay: 'đang chạy',
    cho_ban: 'chờ bạn',
    vuong_mac: 'vướng mắc',
    can_sua: 'cần sửa',
    sua_lai: 'sửa lại',
    killed: 'đã dừng',
    loi: 'lỗi',
    vi_pham: 'vi phạm giao thức',
    cho: 'chưa bắt đầu',
    chua_tu_dong: 'chưa tự động',
  };

  var PROFILE_CLASSES = {
    work: BADGE + ' bg-sky-100 text-sky-800 dark:bg-sky-900/40 dark:text-sky-200',
    personal: BADGE + ' bg-violet-100 text-violet-800 dark:bg-violet-900/40 dark:text-violet-200',
  };

  window.AB = {
    route: { name: 'overview', params: {} },
    _timers: [],
    _leaveHooks: [],

    profile: function () {
      try {
        var p = localStorage.getItem('ab.profile');
        if (p === 'work' || p === 'personal') return p;
      } catch (e) {}
      return 'all';
    },

    setProfile: function (p) {
      try {
        localStorage.setItem('ab.profile', p);
      } catch (e) {}
      window.dispatchEvent(new CustomEvent('ab:profile', { detail: { profile: p } }));
    },

    profileQuery: function () {
      var p = window.AB.profile();
      return (p && p !== 'all') ? 'profile=' + encodeURIComponent(p) : '';
    },

    api: function (method, path, body) {
      var opts = {
        method: method || 'GET',
        credentials: 'same-origin',
        headers: {},
      };
      var m = (method || 'GET').toUpperCase();
      if (m === 'POST' || m === 'PUT') {
        opts.headers['Content-Type'] = 'application/json';
        if (body === undefined || body === null) {
          opts.body = '{}';
        } else if (typeof body === 'string') {
          opts.body = body;
        } else {
          opts.body = JSON.stringify(body);
        }
      }
      return fetch(path, opts)
        .then(function (res) {
          var status = res.status;
          var ct = res.headers.get('content-type') || '';
          var jsonPromise = ct.indexOf('application/json') !== -1 ? res.json() : Promise.resolve(null);
          return jsonPromise
            .then(function (data) {
              window.dispatchEvent(new CustomEvent('ab:net', { detail: { status: status } }));
              return { ok: res.ok, status: status, data: data };
            })
            .catch(function () {
              window.dispatchEvent(new CustomEvent('ab:net', { detail: { status: status } }));
              return { ok: res.ok, status: status, data: null };
            });
        })
        .catch(function () {
          window.dispatchEvent(new CustomEvent('ab:net', { detail: { status: 0 } }));
          return { ok: false, status: 0, data: null };
        });
    },

    every: function (ms, fn) {
      var id = setInterval(fn, ms);
      window.AB._timers.push(id);
      return id;
    },

    onLeave: function (fn) {
      window.AB._leaveHooks.push(fn);
    },

    onReady: function (name, factory) {
      if (window.Alpine) {
        window.Alpine.data(name, factory);
      } else {
        document.addEventListener('alpine:init', function () {
          window.Alpine.data(name, factory);
        }, { once: true });
      }
    },

    fmtNum: function (n) {
      if (n === null || n === undefined || isNaN(n)) return '0';
      return new Intl.NumberFormat('vi-VN').format(n);
    },

    fmtDuration: function (ms) {
      if (ms === null || ms === undefined || isNaN(ms)) return '—';
      var totalSec = Math.floor(ms / 1000);
      if (totalSec < 0) totalSec = 0;
      if (totalSec < 60) {
        return totalSec + ' giây';
      } else if (totalSec < 3600) {
        var m = Math.floor(totalSec / 60);
        var s = totalSec % 60;
        return m + ' phút ' + (s < 10 ? '0' : '') + s + ' giây';
      } else {
        var h = Math.floor(totalSec / 3600);
        var rem = totalSec % 3600;
        var min = Math.floor(rem / 60);
        return h + ' giờ ' + (min < 10 ? '0' : '') + min + ' phút';
      }
    },

    fmtTime: function (ms) {
      if (ms === null || ms === undefined || isNaN(ms)) return '—';
      var d = new Date(ms);
      if (isNaN(d.getTime())) return '—';
      var now = new Date();
      var isToday = d.getFullYear() === now.getFullYear() &&
                    d.getMonth() === now.getMonth() &&
                    d.getDate() === now.getDate();
      function pad(v) { return v < 10 ? '0' + v : String(v); }
      if (isToday) {
        return pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
      }
      return pad(d.getDate()) + '/' + pad(d.getMonth() + 1) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
    },

    fmtCost: function (usd) {
      if (usd === null || usd === undefined || isNaN(usd)) return '—';
      return '≈ $' + Number(usd).toFixed(4);
    },

    fmtClock: function (ms) {
      if (ms === null || ms === undefined || isNaN(ms)) return '—';
      var d = new Date(ms);
      if (isNaN(d.getTime())) return '—';
      var now = new Date();
      var isToday = d.getFullYear() === now.getFullYear() &&
                    d.getMonth() === now.getMonth() &&
                    d.getDate() === now.getDate();
      function pad(v) { return v < 10 ? '0' + v : String(v); }
      if (isToday) {
        return pad(d.getHours()) + ':' + pad(d.getMinutes());
      }
      return pad(d.getDate()) + '/' + pad(d.getMonth() + 1) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes());
    },

    fmtMinutes: function (ms) {
      if (ms === null || ms === undefined || isNaN(ms)) return '—';
      if (ms > 0 && ms < 30000) return '< 1 phút';
      return Math.round(Math.max(ms, 0) / 60000) + ' phút';
    },

    fmtUsd: function (usd) {
      if (usd === null || usd === undefined || isNaN(usd)) return '—';
      if (usd > 0 && usd < 0.005) return '< $0.01';
      return '$' + Number(usd).toFixed(2);
    },

    stepWord: function (step) {
      if (!step) return '';
      if (step.state === 'killed') return 'đã dừng';
      return step.label || '';
    },

    stepClass: function (state) {
      return STEP_CLASSES[state] || STEP_CLASSES.cho;
    },

    stepGlyph: function (state) {
      return STEP_GLYPHS[state] || STEP_GLYPHS.cho;
    },

    stepGlyphClass: function (state) {
      if (state === 'dang_chay') {
        return 'w-6 text-center motion-safe:animate-pulse';
      }
      return 'w-6 text-center';
    },

    stateText: function (state) {
      return STATE_TEXT[state] || STATE_TEXT.cho;
    },

    stateCard: function (state) {
      return STATE_CARD[state] || STATE_CARD.cho;
    },

    stateWord: function (state) {
      return STATE_WORDS[state] || STATE_WORDS.cho;
    },

    roleName: function (role) {
      if (role === 'lead') return 'Lead';
      if (role === 'dev') return 'Dev';
      if (role === 'qa') return 'QA';
      if (role === 'plan-review') return 'Review plan';
      if (role === 'code-review') return 'Review code';
      return role;
    },

    countParts: function (counts) {
      if (!counts) return [];
      var parts = [];
      function known(v) {
        return v !== null && v !== undefined && !isNaN(v);
      }
      if (known(counts.blocking)) parts.push(counts.blocking + ' chặn');
      if (known(counts.advisory)) parts.push(counts.advisory + ' góp ý');
      if (known(counts.specs)) parts.push(counts.specs + ' kiểm thử');
      if (known(counts.not_automatable)) parts.push(counts.not_automatable + ' không tự động được');
      if (known(counts.failing)) parts.push(counts.failing + ' lỗi');
      return parts;
    },

    budgetParts: function (b) {
      if (!b) return { text: '', level: '', note: '' };
      var used = window.AB.fmtUsd(b.used_usd);
      var text = 'Ngân sách: ' + b.used_minutes + ' / ' + b.limit_minutes + ' phút · ' +
        (b.estimated ? '≈ ' : '') + used + ' / ' + window.AB.fmtUsd(b.limit_usd);
      var level = '';
      if (b.level === 'over') {
        level = 'đã hết';
      } else if (b.level === 'near') {
        level = 'sắp hết';
      }
      var note = '';
      if (b.estimated) {
        note = 'Gồm chi phí ước tính từ token cho ' + b.estimated_runs + ' run bị dừng hoặc chưa có kết quả, nên có thể lớn hơn Tổng đến giờ.';
      }
      return { text: text, level: level, note: note };
    },

    autoBadge: function (auto) {
      var state = auto ? auto.state : 'off';
      if (state === 'running') return { text: 'Autopilot: đang chạy', key: 'dang_chay' };
      if (state === 'stopping') return { text: 'Autopilot: đang dừng', key: 'dang_chay' };
      if (state === 'gate') return { text: 'Autopilot: chờ bạn duyệt plan', key: 'cho_ban' };
      if (state === 'stopped') {
        var word = (auto && auto.stop_word) ? auto.stop_word : '—';
        return { text: 'Autopilot: chờ bạn · ' + word, key: 'cho_ban' };
      }
      if (state === 'lost') return { text: 'Autopilot: đã dừng', key: 'killed' };
      if (state === 'done') return { text: 'Autopilot: xong', key: 'xong' };
      return { text: 'Autopilot: tắt', key: 'cho' };
    },

    profileClass: function (p) {
      return PROFILE_CLASSES[p] || PROFILE_CLASSES.work;
    },

    filterText: function () {
      var p = window.AB.profile();
      return (!p || p === 'all') ? '' : 'Đang lọc: ' + p;
    },

    tokensKnown: function (row) {
      if (!row) return false;
      return !(row.input_tokens === 0 && row.output_tokens === 0 && row.cache_read_tokens === 0 && row.cache_write_tokens === 0);
    },

    costKnown: function (row) {
      if (!row) return false;
      return typeof row.cost_usd === 'number' && row.cost_usd > 0;
    },

    errorText: function (res) {
      if (res && res.data && res.data.error) {
        return res.data.error;
      }
      if (res && res.status && res.status > 0) {
        return 'HTTP ' + res.status;
      }
      return 'Mất kết nối tới agent-brain serve';
    },

    lockScroll: function (on) {
      if (on) {
        document.body.classList.add('overflow-hidden');
      } else {
        document.body.classList.remove('overflow-hidden');
      }
    },

    focusLater: function (id, fallbackId) {
      var attempts = 0;
      var maxAttempts = 40;
      function tryFocus() {
        var el = document.getElementById(id);
        if (el && el.offsetParent !== null) {
          el.focus();
          if (document.activeElement === el) {
            return;
          }
        }
        if (attempts < maxAttempts) {
          attempts += 1;
          setTimeout(tryFocus, 25);
        } else if (typeof fallbackId === 'string' && fallbackId && fallbackId !== id) {
          window.AB.focusLater(fallbackId);
        }
      }
      setTimeout(tryFocus, 0);
    },
  };

  var router = (function () {
    var loadedScripts = {};
    var screenTitles = {
      overview: 'Tổng quan',
      assign: 'Giao việc',
      task: 'Chi tiết task',
      stats: 'Thống kê',
      projects: 'Dự án',
    };

    function parseHash(hash) {
      var h = hash !== undefined ? hash : window.location.hash;
      if (!h || h === '#' || h === '#/') {
        return { name: 'overview', params: {} };
      }
      if (h === '#/assign') {
        return { name: 'assign', params: {} };
      }
      if (h === '#/stats') {
        return { name: 'stats', params: {} };
      }
      if (h === '#/projects') {
        return { name: 'projects', params: {} };
      }
      var m = h.match(/^#\/task\/(\d+)$/);
      if (m) {
        return { name: 'task', params: { id: m[1] } };
      }
      return { name: 'overview', params: {} };
    }

    function loadScript(name) {
      if (loadedScripts[name]) {
        return Promise.resolve();
      }
      return new Promise(function (resolve, reject) {
        var s = document.createElement('script');
        s.src = '/js/' + name + '.js';
        s.onload = function () {
          loadedScripts[name] = true;
          resolve();
        };
        s.onerror = function (err) {
          reject(err);
        };
        document.head.appendChild(s);
      });
    }

    function navigate(isHashChange) {
      var parsed = parseHash();
      window.AB.route = parsed;
      var name = parsed.name;

      var timers = window.AB._timers;
      window.AB._timers = [];
      timers.forEach(function (t) { clearInterval(t); });

      var hooks = window.AB._leaveHooks;
      window.AB._leaveHooks = [];
      hooks.forEach(function (h) {
        try { h(); } catch (e) { console.error(e); }
      });

      var screenEl = document.getElementById('screen');

      return loadScript(name)
        .then(function () {
          return fetch('/screens/' + name + '.html', { credentials: 'same-origin' });
        })
        .then(function (res) {
          if (!res.ok) {
            throw new Error('HTTP ' + res.status);
          }
          return res.text();
        })
        .then(function (html) {
          if (screenEl) {
            screenEl.innerHTML = html;
            if (window.Alpine && window.Alpine.initTree) {
              window.Alpine.initTree(screenEl);
            }
          }
          var titlePrefix = screenTitles[name] || name;
          document.title = titlePrefix + ' · agent-brain';
          window.dispatchEvent(new CustomEvent('ab:route', { detail: { name: name } }));
          if (isHashChange && screenEl) {
            screenEl.focus();
          }
        })
        .catch(function () {
          if (screenEl) {
            screenEl.textContent = 'Lỗi tải màn hình ' + name;
          }
          var titlePrefix = screenTitles[name] || name;
          document.title = titlePrefix + ' · agent-brain';
          window.dispatchEvent(new CustomEvent('ab:route', { detail: { name: name } }));
          if (isHashChange && screenEl) {
            screenEl.focus();
          }
        });
    }

    var initialized = false;
    function init() {
      if (initialized) return;
      initialized = true;
      window.addEventListener('hashchange', function () {
        navigate(true);
      });
      navigate(false);
    }

    return {
      init: init,
      parseHash: parseHash,
    };
  })();

  document.addEventListener('alpine:init', function () {
    Alpine.data('shell', function () {
    return {
      profile: window.AB.profile(),
      themeMode: window.ABTheme.mode(),
      routeName: window.AB.route.name,
      sessionLost: false,
      netFails: 0,

      init: function () {
        var self = this;
        window.addEventListener('ab:net', function (e) {
          var status = e.detail ? e.detail.status : 0;
          if (status === 401) {
            self.sessionLost = true;
          } else if (status === 0) {
            self.netFails += 1;
          } else {
            self.netFails = 0;
          }
        });
        window.addEventListener('ab:route', function (e) {
          if (e.detail && e.detail.name) {
            self.routeName = e.detail.name;
          }
        });
        router.init();
      },

      profileChanged: function () {
        window.AB.setProfile(this.profile);
      },

      toggleTheme: function () {
        var current = this.themeMode;
        var next = current === 'system' ? 'dark' : (current === 'dark' ? 'light' : 'system');
        this.themeMode = next;
        window.ABTheme.set(next);
      },

      get themeLabel() {
        if (this.themeMode === 'dark') return 'Giao diện: tối';
        if (this.themeMode === 'light') return 'Giao diện: sáng';
        return 'Giao diện: hệ thống';
      },

      get themeAria() {
        if (this.themeMode === 'dark') return 'Đổi giao diện (hiện tại: tối)';
        if (this.themeMode === 'light') return 'Đổi giao diện (hiện tại: sáng)';
        return 'Đổi giao diện (hiện tại: hệ thống)';
      },

      get connLost() {
        return this.netFails >= 2;
      },

      get hasBanner() {
        return this.sessionLost || this.connLost;
      },

      navClass: function (name) {
        if (this.routeName === name) {
          return 'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-200';
        }
        return 'text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700';
      },

      navCurrent: function (name) {
        return this.routeName === name ? 'page' : false;
      },
    };
  });
  });
})();

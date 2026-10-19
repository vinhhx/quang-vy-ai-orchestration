// js/app.js — deferred, before Alpine
(function () {
  var BADGE = 'inline-flex items-center gap-1 rounded-full px-2 py-1 text-sm font-semibold';

  var STEP_CLASSES = {
    cho: BADGE + ' bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-200',
    dang_chay: BADGE + ' bg-indigo-100 text-indigo-800 dark:bg-indigo-900/50 dark:text-indigo-200',
    xong: BADGE + ' bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200',
    vuong_mac: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    can_sua: BADGE + ' bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200',
    killed: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
    loi: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
    vi_pham: BADGE + ' bg-rose-100 text-rose-800 dark:bg-rose-900/40 dark:text-rose-200',
  };

  var STEP_GLYPHS = {
    cho: '○\uFE0E',
    dang_chay: '◐\uFE0E',
    xong: '✓\uFE0E',
    vuong_mac: '!\uFE0E',
    can_sua: '!\uFE0E',
    killed: '■\uFE0E',
    loi: '✕\uFE0E',
    vi_pham: '✕\uFE0E',
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

    roleName: function (role) {
      if (role === 'lead') return 'Lead';
      if (role === 'dev') return 'Dev';
      if (role === 'qa') return 'QA';
      return role;
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

    focusLater: function (id) {
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

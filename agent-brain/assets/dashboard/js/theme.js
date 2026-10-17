// js/theme.js — classic script in <head>; localStorage key 'ab.theme' = 'system' | 'dark' | 'light' (default 'system')
(function () {
  function getMode() {
    try {
      var m = localStorage.getItem('ab.theme');
      if (m === 'dark' || m === 'light' || m === 'system') return m;
    } catch (e) {}
    return 'system';
  }

  function setMode(mode) {
    try {
      localStorage.setItem('ab.theme', mode);
    } catch (e) {}
    applyTheme();
  }

  function applyTheme() {
    var mode = getMode();
    var isDark = false;
    if (mode === 'dark') {
      isDark = true;
    } else if (mode === 'system') {
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
        isDark = true;
      }
    }
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }

  window.ABTheme = {
    mode: getMode,
    set: setMode,
    apply: applyTheme,
  };

  applyTheme();

  var mql = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;
  if (mql) {
    var handler = function () {
      if (getMode() === 'system') {
        applyTheme();
      }
    };
    if (mql.addEventListener) {
      mql.addEventListener('change', handler);
    } else if (mql.addListener) {
      mql.addListener(handler);
    }
  }
})();

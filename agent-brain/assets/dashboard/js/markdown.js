// js/markdown.js — small, safe Markdown renderer for the dashboard.
//
// window.abMarkdown.render(text) returns a DocumentFragment built only with
// createElement / createTextNode. Nothing from the input is ever parsed as
// HTML, executed, or used as an attribute except a validated link target.
// Raw HTML in the input is shown as plain text. Images are never loaded: they
// show as a text link. Only http:, https: and relative links are kept.
(function () {
  'use strict';

  var MAX_DEPTH = 6;

  var CLS = {
    h1: 'mt-6 first:mt-0 border-b border-slate-200 pb-2 text-2xl font-semibold leading-tight break-words dark:border-slate-700',
    h2: 'mt-6 first:mt-0 text-xl font-semibold leading-tight break-words',
    h3: 'mt-5 first:mt-0 text-lg font-semibold leading-tight break-words',
    h4: 'mt-4 first:mt-0 text-base font-semibold leading-tight break-words',
    p: 'mt-3 first:mt-0 break-words leading-relaxed',
    ul: 'mt-3 first:mt-0 list-disc space-y-1 pl-6',
    ol: 'mt-3 first:mt-0 list-decimal space-y-1 pl-6',
    ulNested: 'mt-1 list-disc space-y-1 pl-6',
    olNested: 'mt-1 list-decimal space-y-1 pl-6',
    li: 'break-words leading-relaxed',
    quote: 'mt-3 first:mt-0 border-l-4 border-slate-300 pl-4 text-slate-600 dark:border-slate-600 dark:text-slate-300',
    hr: 'my-6 border-slate-200 dark:border-slate-700',
    pre: 'mt-3 first:mt-0 overflow-x-auto rounded-xl bg-slate-900 p-4 font-mono text-sm leading-normal text-slate-100 ring-1 ring-slate-800 dark:bg-slate-950',
    code: 'whitespace-pre font-mono',
    inlineCode: 'rounded bg-slate-100 px-1 py-0.5 font-mono text-[0.9em] text-slate-900 dark:bg-slate-700 dark:text-slate-100',
    tableWrap: 'mt-3 first:mt-0 overflow-x-auto',
    table: 'w-full border-collapse text-left text-sm',
    th: 'min-w-24 border border-slate-300 bg-slate-100 px-3 py-2 font-semibold dark:border-slate-600 dark:bg-slate-700',
    td: 'min-w-24 border border-slate-300 px-3 py-2 align-top dark:border-slate-600',
    alignLeft: 'text-left',
    alignCenter: 'text-center',
    alignRight: 'text-right',
    link: 'break-words text-indigo-700 underline hover:text-indigo-900 dark:text-indigo-300 dark:hover:text-indigo-200',
    strong: 'font-semibold',
    em: 'italic',
    del: 'line-through',
  };

  function el(tag, cls) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    return n;
  }

  function txt(s) {
    return document.createTextNode(s);
  }

  // ---- links ---------------------------------------------------------------

  // Returns the cleaned URL when it is safe to link to, else ''. Browsers drop
  // tabs, newlines and other control characters inside a URL, so strip them
  // before looking at the scheme ("java\tscript:" must not slip through).
  function safeUrl(raw) {
    if (typeof raw !== 'string') return '';
    var u = raw.replace(/[\u0000-\u0020\u007f-\u009f\u2028\u2029]/g, '');
    if (!u) return '';
    var m = /^([a-zA-Z][a-zA-Z0-9+.\-]*):/.exec(u);
    if (m) {
      var scheme = m[1].toLowerCase();
      return (scheme === 'http' || scheme === 'https') ? u : '';
    }
    // In-page anchors would be taken by the hash router, so only app routes
    // ("#/...") are kept among the "#" links.
    if (u.charAt(0) === '#') return u.charAt(1) === '/' ? u : '';
    if (u.indexOf('\\') !== -1) return '';
    return u;
  }

  function makeLink(url, depth) {
    var a = el('a', CLS.link);
    a.setAttribute('href', url);
    if (url.charAt(0) !== '#') {
      a.setAttribute('target', '_blank');
      a.setAttribute('rel', 'noopener noreferrer');
    }
    return a;
  }

  // ---- inline --------------------------------------------------------------

  function isAlnum(ch) {
    return !!ch && /[A-Za-z0-9]/.test(ch);
  }

  function isSpace(ch) {
    return !ch || /\s/.test(ch);
  }

  // Finds the "]" that closes the "[" at index `open`, honouring nesting and
  // backslash escapes. Returns -1 when there is none.
  function findBracketEnd(s, open) {
    var depth = 0;
    for (var i = open; i < s.length; i++) {
      var c = s.charAt(i);
      if (c === '\\') { i++; continue; }
      if (c === '[') depth++;
      else if (c === ']') {
        depth--;
        if (depth === 0) return i;
      }
    }
    return -1;
  }

  // Parses "(url "title")" starting at index `open` (a "("). Returns
  // { url, end } where end is the index after ")", or null.
  function parseLinkTarget(s, open) {
    var i = open + 1;
    var depth = 1;
    var start = i;
    for (; i < s.length; i++) {
      var c = s.charAt(i);
      if (c === '\\') { i++; continue; }
      if (c === '(') depth++;
      else if (c === ')') {
        depth--;
        if (depth === 0) break;
      }
    }
    if (depth !== 0) return null;
    var inner = s.slice(start, i).trim();
    // drop an optional title: url "title"
    var t = /^(\S+)\s+(?:"[^"]*"|'[^']*')$/.exec(inner);
    if (t) inner = t[1];
    if (inner.charAt(0) === '<' && inner.charAt(inner.length - 1) === '>') {
      inner = inner.slice(1, -1);
    }
    return { url: inner, end: i + 1 };
  }

  function appendInline(parent, s, depth) {
    if (depth > MAX_DEPTH) {
      parent.appendChild(txt(s));
      return;
    }
    var buf = '';
    var i = 0;
    var n = s.length;

    function flush() {
      if (buf) {
        parent.appendChild(txt(buf));
        buf = '';
      }
    }

    while (i < n) {
      var c = s.charAt(i);

      // backslash escape
      if (c === '\\' && i + 1 < n && /[\\`*_{}\[\]()#+\-.!|~<>]/.test(s.charAt(i + 1))) {
        buf += s.charAt(i + 1);
        i += 2;
        continue;
      }

      // hard break: backslash at the end of a line
      if (c === '\\' && s.charAt(i + 1) === '\n') {
        flush();
        parent.appendChild(el('br'));
        i += 2;
        continue;
      }
      if (c === '\n') {
        // two trailing spaces already in buf make a hard break
        if (/ {2,}$/.test(buf)) {
          buf = buf.replace(/ +$/, '');
          flush();
          parent.appendChild(el('br'));
        } else {
          buf = buf.replace(/ +$/, '') + ' ';
        }
        i++;
        continue;
      }

      // code span
      if (c === '`') {
        var run = 1;
        while (s.charAt(i + run) === '`') run++;
        var fence = new Array(run + 1).join('`');
        var close = -1;
        var from = i + run;
        while (from < n) {
          var at = s.indexOf(fence, from);
          if (at === -1) break;
          var after = s.charAt(at + run);
          if (after !== '`') { close = at; break; }
          // longer run of backticks: skip it entirely
          var skip = at;
          while (s.charAt(skip) === '`') skip++;
          from = skip;
        }
        if (close !== -1) {
          flush();
          var code = el('code', CLS.inlineCode);
          var body = s.slice(i + run, close).replace(/\n/g, ' ');
          if (body.length > 2 && body.charAt(0) === ' ' && body.charAt(body.length - 1) === ' ') {
            body = body.slice(1, -1);
          }
          code.textContent = body;
          parent.appendChild(code);
          i = close + run;
          continue;
        }
        buf += fence;
        i += run;
        continue;
      }

      // image: shown as a text link, never loaded
      if (c === '!' && s.charAt(i + 1) === '[') {
        var iend = findBracketEnd(s, i + 1);
        if (iend !== -1 && s.charAt(iend + 1) === '(') {
          var itarget = parseLinkTarget(s, iend + 1);
          if (itarget) {
            flush();
            var alt = s.slice(i + 2, iend).trim();
            var label = '[Ảnh' + (alt ? ': ' + alt : '') + ']';
            var iurl = safeUrl(itarget.url);
            if (iurl) {
              var ia = makeLink(iurl);
              ia.textContent = label;
              parent.appendChild(ia);
            } else {
              parent.appendChild(txt(label));
            }
            i = itarget.end;
            continue;
          }
        }
      }

      // link
      if (c === '[') {
        var lend = findBracketEnd(s, i);
        if (lend !== -1 && s.charAt(lend + 1) === '(') {
          var ltarget = parseLinkTarget(s, lend + 1);
          if (ltarget) {
            flush();
            var lurl = safeUrl(ltarget.url);
            var label2 = s.slice(i + 1, lend);
            if (lurl) {
              var la = makeLink(lurl);
              appendInline(la, label2, MAX_DEPTH);
              parent.appendChild(la);
            } else {
              // unsafe target: keep the text, drop the link
              appendInline(parent, label2, depth + 1);
            }
            i = ltarget.end;
            continue;
          }
        }
        // task list marker "[ ]" / "[x]" is handled by the list code
      }

      // bare http(s) URL
      if ((c === 'h' || c === 'H') && !isAlnum(s.charAt(i - 1))) {
        var um = /^https?:\/\/[^\s<>]+/i.exec(s.slice(i, i + 2048));
        if (um) {
          var url = um[0];
          // trailing punctuation is not part of the URL
          while (/[.,;:!?'")\]*_]$/.test(url)) {
            if (url.charAt(url.length - 1) === ')' && url.indexOf('(') !== -1) break;
            url = url.slice(0, -1);
          }
          var burl = safeUrl(url);
          if (burl && url.length > 8) {
            flush();
            var ba = makeLink(burl);
            ba.textContent = url;
            parent.appendChild(ba);
            i += url.length;
            continue;
          }
        }
      }

      // strong / emphasis / strike
      if (c === '*' || c === '_' || c === '~') {
        var rl = 1;
        while (s.charAt(i + rl) === c) rl++;
        var prev = s.charAt(i - 1);
        var markLen = 0;
        if (c === '~') {
          markLen = rl >= 2 ? 2 : 0;
        } else {
          markLen = rl >= 3 ? 3 : rl;
        }
        var canOpen = markLen > 0 && !isSpace(s.charAt(i + markLen));
        if (c === '_' && isAlnum(prev)) canOpen = false;
        if (canOpen) {
          var marker = new Array(markLen + 1).join(c);
          var j = i + markLen;
          var found = -1;
          while (j < n) {
            var k = s.indexOf(marker, j);
            if (k === -1) break;
            var before = s.charAt(k - 1);
            var next = s.charAt(k + markLen);
            var okClose = !isSpace(before);
            if (c === '_' && isAlnum(next)) okClose = false;
            // a longer run of the same char belongs to a different marker
            if ((next === c || before === c) && markLen < 3) okClose = false;
            if (k === i + markLen) okClose = false;
            if (okClose) { found = k; break; }
            j = k + 1;
          }
          if (found !== -1) {
            flush();
            var inner = s.slice(i + markLen, found);
            var node;
            if (c === '~') {
              node = el('del', CLS.del);
              appendInline(node, inner, depth + 1);
            } else if (markLen === 3) {
              node = el('strong', CLS.strong);
              var em = el('em', CLS.em);
              appendInline(em, inner, depth + 1);
              node.appendChild(em);
            } else if (markLen === 2) {
              node = el('strong', CLS.strong);
              appendInline(node, inner, depth + 1);
            } else {
              node = el('em', CLS.em);
              appendInline(node, inner, depth + 1);
            }
            parent.appendChild(node);
            i = found + markLen;
            continue;
          }
        }
        buf += s.slice(i, i + rl);
        i += rl;
        continue;
      }

      buf += c;
      i++;
    }
    flush();
  }

  // ---- blocks --------------------------------------------------------------

  var RE_FENCE = /^ {0,3}(`{3,}|~{3,})\s*([^`\s]*)[^`]*$/;
  var RE_HEADING = /^ {0,3}(#{1,6})[ \t]+(.*?)(?:[ \t]+#+)?[ \t]*$/;
  var RE_HEADING_EMPTY = /^ {0,3}(#{1,6})[ \t]*$/;
  var RE_HR = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
  var RE_QUOTE = /^ {0,3}>[ \t]?(.*)$/;
  var RE_LIST = /^([ \t]*)([-*+]|\d{1,9}[.)])[ \t]+(.*)$/;
  var RE_TABLE_SEP = /^[ \t]*\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;

  function indentOf(s) {
    var w = 0;
    for (var i = 0; i < s.length; i++) {
      var c = s.charAt(i);
      if (c === ' ') w++;
      else if (c === '\t') w += 4;
      else break;
    }
    return w;
  }

  function splitRow(line) {
    var t = line.trim();
    if (t.charAt(0) === '|') t = t.slice(1);
    if (t.charAt(t.length - 1) === '|' && t.charAt(t.length - 2) !== '\\') t = t.slice(0, -1);
    var cells = [];
    var cur = '';
    var inCode = false;
    for (var i = 0; i < t.length; i++) {
      var c = t.charAt(i);
      if (c === '\\' && t.charAt(i + 1) === '|') { cur += '|'; i++; continue; }
      if (c === '`') inCode = !inCode;
      if (c === '|' && !inCode) { cells.push(cur.trim()); cur = ''; continue; }
      cur += c;
    }
    cells.push(cur.trim());
    return cells;
  }

  function isTableStart(lines, i) {
    if (i + 1 >= lines.length) return false;
    var head = lines[i];
    var sep = lines[i + 1];
    if (head.indexOf('|') === -1 || sep.indexOf('-') === -1) return false;
    if (!RE_TABLE_SEP.test(sep) || (sep.indexOf('|') === -1 && head.indexOf('|') === -1)) return false;
    return splitRow(head).length === splitRow(sep).length;
  }

  function startsBlock(lines, i) {
    var l = lines[i];
    return RE_FENCE.test(l) || RE_HEADING.test(l) || RE_HEADING_EMPTY.test(l) ||
      RE_HR.test(l) || RE_QUOTE.test(l) || RE_LIST.test(l) || isTableStart(lines, i);
  }

  function renderTable(lines, i, out) {
    var head = splitRow(lines[i]);
    var seps = splitRow(lines[i + 1]);
    var aligns = [];
    for (var a = 0; a < seps.length; a++) {
      var sp = seps[a];
      var left = sp.charAt(0) === ':';
      var right = sp.charAt(sp.length - 1) === ':';
      aligns.push(left && right ? CLS.alignCenter : right ? CLS.alignRight : left ? CLS.alignLeft : '');
    }
    var wrap = el('div', CLS.tableWrap);
    var table = el('table', CLS.table);
    var thead = el('thead');
    var htr = el('tr');
    var c;
    for (c = 0; c < head.length; c++) {
      var th = el('th', CLS.th + (aligns[c] ? ' ' + aligns[c] : ''));
      th.setAttribute('scope', 'col');
      appendInline(th, head[c], 1);
      htr.appendChild(th);
    }
    thead.appendChild(htr);
    table.appendChild(thead);
    var tbody = el('tbody');
    var r = i + 2;
    while (r < lines.length && lines[r].trim() !== '' && lines[r].indexOf('|') !== -1 &&
      !RE_FENCE.test(lines[r]) && !RE_HEADING.test(lines[r])) {
      var cells = splitRow(lines[r]);
      var tr = el('tr');
      for (c = 0; c < head.length; c++) {
        var td = el('td', CLS.td + (aligns[c] ? ' ' + aligns[c] : ''));
        appendInline(td, cells[c] || '', 1);
        tr.appendChild(td);
      }
      tbody.appendChild(tr);
      r++;
    }
    table.appendChild(tbody);
    wrap.appendChild(table);
    out.appendChild(wrap);
    return r;
  }

  // Collects a list starting at line i. Returns the index after the list.
  // One nested level is kept: any deeper item is folded into the nested list.
  function renderList(lines, i, out) {
    var first = RE_LIST.exec(lines[i]);
    var baseIndent = indentOf(first[1]);
    var ordered = /\d/.test(first[2].charAt(0));
    var items = []; // { text, children: [ { text } ], childOrdered }
    var cur = null;
    var r = i;

    while (r < lines.length) {
      var line = lines[r];
      if (line.trim() === '') {
        // a blank line continues the list only when another item follows
        var q = r + 1;
        while (q < lines.length && lines[q].trim() === '') q++;
        if (q < lines.length && RE_LIST.test(lines[q]) && indentOf(RE_LIST.exec(lines[q])[1]) >= baseIndent) {
          r = q;
          continue;
        }
        break;
      }
      var m = RE_LIST.exec(line);
      if (m) {
        var ind = indentOf(m[1]);
        if (RE_HR.test(line) && ind <= baseIndent + 1) break;
        if (ind < baseIndent) break;
        var isOrd = /\d/.test(m[2].charAt(0));
        if (ind - baseIndent >= 2 && cur) {
          cur.children.push({ text: m[3] });
          cur.childOrdered = cur.children.length === 1 ? isOrd : cur.childOrdered;
        } else {
          if (ind === baseIndent && isOrd !== ordered && items.length > 0) break;
          cur = { text: m[3], children: [], childOrdered: false };
          items.push(cur);
        }
        r++;
        continue;
      }
      // continuation of the current item: indented, not a new block
      if (cur && indentOf(line) > baseIndent && !RE_FENCE.test(line)) {
        var target = cur.children.length ? cur.children[cur.children.length - 1] : cur;
        target.text += '\n' + line.trim();
        r++;
        continue;
      }
      break;
    }

    var list = el(ordered ? 'ol' : 'ul', ordered ? CLS.ol : CLS.ul);
    if (ordered) {
      var startNum = parseInt(first[2], 10);
      if (startNum > 1 && startNum < 1000000) list.setAttribute('start', String(startNum));
    }
    for (var k = 0; k < items.length; k++) {
      var it = items[k];
      var li = el('li', CLS.li);
      appendListText(li, it.text);
      if (it.children.length) {
        var sub = el(it.childOrdered ? 'ol' : 'ul', it.childOrdered ? CLS.olNested : CLS.ulNested);
        for (var s = 0; s < it.children.length; s++) {
          var sli = el('li', CLS.li);
          appendListText(sli, it.children[s].text);
          sub.appendChild(sli);
        }
        li.appendChild(sub);
      }
      list.appendChild(li);
    }
    out.appendChild(list);
    return r;
  }

  function appendListText(li, text) {
    var m = /^\[([ xX])\][ \t]+/.exec(text);
    if (m) {
      li.appendChild(txt(m[1] === ' ' ? '☐ ' : '☑ '));
      text = text.slice(m[0].length);
    }
    appendInline(li, text, 1);
  }

  function renderBlocks(text, out, depth) {
    var lines = text.split('\n');
    var i = 0;
    while (i < lines.length) {
      var line = lines[i];

      if (line.trim() === '') { i++; continue; }

      // fenced code
      var f = RE_FENCE.exec(line);
      if (f) {
        var marker = f[1];
        var body = [];
        var j = i + 1;
        var closeRe = new RegExp('^ {0,3}' + (marker.charAt(0) === '`' ? '`' : '~') + '{' + marker.length + ',}[ \\t]*$');
        while (j < lines.length && !closeRe.test(lines[j])) {
          body.push(lines[j]);
          j++;
        }
        var pre = el('pre', CLS.pre);
        pre.setAttribute('tabindex', '0');
        var code = el('code', CLS.code);
        if (f[2]) code.setAttribute('data-lang', f[2].slice(0, 32));
        code.textContent = body.join('\n');
        pre.appendChild(code);
        out.appendChild(pre);
        i = j + 1;
        continue;
      }

      // heading
      var h = RE_HEADING.exec(line);
      var he = h ? null : RE_HEADING_EMPTY.exec(line);
      if (h || he) {
        var level = Math.min((h || he)[1].length, 4);
        var hn = el('h' + level, CLS['h' + level]);
        if (h) appendInline(hn, h[2], 1);
        out.appendChild(hn);
        i++;
        continue;
      }

      if (RE_HR.test(line)) {
        out.appendChild(el('hr', CLS.hr));
        i++;
        continue;
      }

      // block quote
      if (RE_QUOTE.test(line)) {
        var qlines = [];
        var q = i;
        while (q < lines.length) {
          var qm = RE_QUOTE.exec(lines[q]);
          if (qm) { qlines.push(qm[1]); q++; continue; }
          // lazy continuation of a quoted paragraph
          if (lines[q].trim() !== '' && qlines.length && qlines[qlines.length - 1].trim() !== '' &&
            !startsBlock(lines, q)) {
            qlines.push(lines[q]);
            q++;
            continue;
          }
          break;
        }
        var bq = el('blockquote', CLS.quote);
        if (depth < MAX_DEPTH) renderBlocks(qlines.join('\n'), bq, depth + 1);
        else bq.textContent = qlines.join('\n');
        out.appendChild(bq);
        i = q;
        continue;
      }

      if (isTableStart(lines, i)) {
        i = renderTable(lines, i, out);
        continue;
      }

      if (RE_LIST.test(line)) {
        i = renderList(lines, i, out);
        continue;
      }

      // paragraph
      var plines = [line.replace(/^\s+/, '')];
      var p = i + 1;
      while (p < lines.length && lines[p].trim() !== '' && !startsBlock(lines, p)) {
        plines.push(lines[p].replace(/^\s+/, ''));
        p++;
      }
      var pn = el('p', CLS.p);
      appendInline(pn, plines.join('\n'), 0);
      out.appendChild(pn);
      i = p;
    }
  }

  function render(input) {
    var frag = document.createDocumentFragment();
    if (typeof input !== 'string' || input === '') return frag;
    var text = input.replace(/\r\n?/g, '\n').replace(/\u0000/g, '');
    renderBlocks(text, frag, 0);
    return frag;
  }

  window.abMarkdown = { render: render, safeUrl: safeUrl };
})();

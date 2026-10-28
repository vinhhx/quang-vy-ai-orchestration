// scripts/dashboard-keys.mjs — headless keyboard checks for dashboard
// plain Node ESM, no dependencies, global fetch and WebSocket

import { execFileSync } from 'node:child_process';

const [,, portArg, urlArg] = process.argv;
if (!portArg || !urlArg) {
  console.error('LỖI args: Thiếu port hoặc url');
  process.exit(1);
}

const port = parseInt(portArg, 10);
const dashboardUrl = urlArg;

let ws = null;
let msgId = 1;
const pending = new Map();

function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = msgId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function evaluate(expression) {
  const res = await cdp('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (res.exceptionDetails) {
    const desc = res.exceptionDetails.exception?.description || res.exceptionDetails.text || 'Runtime error';
    throw new Error(desc);
  }
  return res.result ? res.result.value : undefined;
}

async function pressKey(key, code, windowsVirtualKeyCode) {
  await cdp('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key,
    code,
    windowsVirtualKeyCode,
  });
  await cdp('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key,
    code,
    windowsVirtualKeyCode,
  });
}

async function pressKeyText(key, code, windowsVirtualKeyCode, text) {
  await cdp('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key,
    code,
    windowsVirtualKeyCode,
    text,
  });
  await cdp('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key,
    code,
    windowsVirtualKeyCode,
  });
}

async function insertText(text) {
  await cdp('Input.insertText', { text });
}

async function waitFor(expr, ms, checkName, detailMsg) {
  const start = Date.now();
  while (Date.now() - start < ms) {
    try {
      const val = await evaluate(expr);
      if (val) return val;
    } catch (_) {}
    await new Promise(r => setTimeout(r, 100));
  }
  const detail = detailMsg || `Hết thời gian chờ (${ms}ms): ${expr}`;
  throw new Error(`LỖI ${checkName}: ${detail}`);
}

async function main() {
  let target;
  try {
    const newRes = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(dashboardUrl)}`, { method: 'PUT' });
    if (newRes.ok) {
      target = await newRes.json();
    }
  } catch (_) {}

  if (!target || !target.webSocketDebuggerUrl) {
    const listRes = await fetch(`http://127.0.0.1:${port}/json/list`);
    const list = await listRes.json();
    target = list.find(t => t.type === 'page');
  }

  if (!target || !target.webSocketDebuggerUrl) {
    throw new Error('LỖI connect: Không tìm thấy page target trên CDP');
  }

  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = (e) => reject(new Error('WebSocket connect error: ' + (e.message || 'failed')));
  });

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.id && pending.has(msg.id)) {
        const { resolve, reject } = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error) {
          reject(new Error(msg.error.message || JSON.stringify(msg.error)));
        } else {
          resolve(msg.result);
        }
      }
    } catch (_) {}
  };

  await cdp('Page.enable', {});
  await cdp('Runtime.enable', {});

  // Check A, combobox
  const baseUrl = dashboardUrl.split('#')[0];
  const assignUrl = `${baseUrl}#/assign`;
  await cdp('Page.navigate', { url: assignUrl });
  await waitFor(
    'Boolean(document.querySelector(\'[data-screen="assign"][data-loaded="yes"]\'))',
    10000,
    'combobox',
    'Màn hình assign không tải kịp thời'
  );

  await evaluate('document.getElementById("ab-project").focus()');
  await insertText('web');
  // Wait out the 200 ms debounce timer and suggest request
  await new Promise(r => setTimeout(r, 300));
  await waitFor(`
    (() => {
      const suggest = document.getElementById("ab-suggest");
      if (!suggest) return false;
      const opts = Array.from(suggest.querySelectorAll('[role="option"]'));
      const hasHang = opts.some(o => o.textContent.includes("(web-ban-hang)"));
      const hasCu = opts.some(o => o.textContent.includes("(web-ban-hang-cu)"));
      return hasHang && hasCu;
    })()
  `, 5000, 'combobox', 'Gợi ý dự án không chứa cả (web-ban-hang) và (web-ban-hang-cu)');

  const cuDisabled = await evaluate(`
    (() => {
      const suggest = document.getElementById("ab-suggest");
      const cu = Array.from(suggest.querySelectorAll('[role="option"]')).find(o => o.textContent.includes("(web-ban-hang-cu)"));
      return cu ? cu.getAttribute("aria-disabled") : null;
    })()
  `);
  if (cuDisabled !== 'true') {
    throw new Error(`LỖI combobox: tùy chọn (web-ban-hang-cu) không có aria-disabled="true" (nhận được: ${cuDisabled})`);
  }

  for (let i = 1; i <= 3; i++) {
    await pressKey('ArrowDown', 'ArrowDown', 40);
    await new Promise(r => setTimeout(r, 100));
    const activeDesc = await evaluate(`
      (() => {
        const input = document.getElementById("ab-project");
        const activeId = input.getAttribute("aria-activedescendant");
        if (!activeId) return { error: "Không có aria-activedescendant" };
        const el = document.getElementById(activeId);
        if (!el) return { error: "Không tìm thấy phần tử id=" + activeId };
        return { id: activeId, disabled: el.getAttribute("aria-disabled") };
      })()
    `);
    if (activeDesc.error) {
      throw new Error(`LỖI combobox: ArrowDown lần ${i}: ${activeDesc.error}`);
    }
    if (activeDesc.disabled === 'true') {
      throw new Error(`LỖI combobox: ArrowDown lần ${i}: phần tử ${activeDesc.id} có aria-disabled="true"`);
    }
  }

  await pressKey('Enter', 'Enter', 13);
  await new Promise(r => setTimeout(r, 100));
  const projectVal = await evaluate('document.getElementById("ab-project").value');
  if (!projectVal || !projectVal.includes('(web-ban-hang)') || projectVal.includes('(web-ban-hang-cu)')) {
    throw new Error(`LỖI combobox: giá trị #ab-project không hợp lệ: "${projectVal}"`);
  }
  console.log('OK combobox');

  // Check gate-stop: autopilot waits at the Bạn gate on task 1. The header
  // offers Dừng autopilot and the strip shows no manual Chạy Dev / Chạy lại
  // Lead beside the gate. One click (one POST) turns autopilot off: Bật
  // autopilot becomes enabled and Chạy lại Lead comes back, with no new run.
  // This check never clicks Duyệt plan.
  await evaluate('window.location.hash = "#/task/1"');
  await waitFor(
    'Boolean(document.querySelector(\'[data-screen="task"][data-loaded="yes"]\'))',
    5000,
    'gate-stop',
    'Màn hình task/1 không tải kịp thời'
  );

  await waitFor(`
    (() => {
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      const btn = document.getElementById("ab-act-auto_stop");
      return vis(btn) && !btn.disabled && btn.textContent.trim() === "Dừng autopilot";
    })()
  `, 5000, 'gate-stop', 'Không thấy nút "Dừng autopilot" (#ab-act-auto_stop) hiển thị và đang bật');

  const manualShown = await evaluate(`
    (() => {
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      return ["ab-act-run_dev", "ab-act-relead"].filter((id) => vis(document.getElementById(id)));
    })()
  `);
  if (manualShown.length > 0) {
    throw new Error(`LỖI gate-stop: nút ${manualShown.join(', ')} hiển thị khi autopilot chờ ở cổng duyệt`);
  }

  const gateRunsBefore = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => (Array.isArray(d.runs) ? d.runs.length : 0))
  `);

  await evaluate(`
    (() => {
      const btn = document.getElementById("ab-act-auto_stop");
      if (btn && !btn.disabled) btn.click();
    })()
  `);

  await waitFor(`
    (() => {
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      const start = document.getElementById("ab-act-auto_start");
      const startOk = vis(start) && !start.disabled && start.textContent.trim() === "Bật autopilot";
      const relead = Array.from(document.querySelectorAll("button")).some(
        (b) => b.textContent.trim() === "Chạy lại Lead" && !b.disabled && vis(b)
      );
      return startOk && relead;
    })()
  `, 5000, 'gate-stop', 'Sau khi bấm Dừng autopilot không thấy nút "Bật autopilot" đang bật và nút "Chạy lại Lead" hiển thị');

  const gateRunsAfter = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => (Array.isArray(d.runs) ? d.runs.length : 0))
  `);
  if (gateRunsAfter !== gateRunsBefore) {
    throw new Error(`LỖI gate-stop: runs.length đã đổi từ ${gateRunsBefore} sang ${gateRunsAfter}`);
  }
  console.log('OK gate-stop');

  // Check B, confirmation dialog
  const runsBefore = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => (Array.isArray(d.runs) ? d.runs.length : 0))
  `);

  await evaluate('window.location.hash = "#/task/1"');
  await waitFor(
    'Boolean(document.querySelector(\'[data-screen="task"][data-loaded="yes"]\'))',
    5000,
    'confirm-dialog',
    'Màn hình task/1 không tải kịp thời'
  );

  await waitFor(`
    (() => {
      const buttons = Array.from(document.querySelectorAll("button"));
      return buttons.some(b => b.textContent.trim() === "Chạy lại Lead" && !b.disabled && (b.offsetWidth > 0 || b.offsetHeight > 0));
    })()
  `, 5000, 'confirm-dialog', 'Không tìm thấy nút "Chạy lại Lead" hiển thị và đang bật');

  await evaluate(`
    (() => {
      const buttons = Array.from(document.querySelectorAll("button"));
      const btn = buttons.find(b => b.textContent.trim() === "Chạy lại Lead" && !b.disabled && (b.offsetWidth > 0 || b.offsetHeight > 0));
      if (btn) btn.click();
    })()
  `);

  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-dlg-cancel"',
    1500,
    'confirm-dialog',
    'document.activeElement.id không phải là ab-dlg-cancel trong vòng 1.5s'
  );

  await pressKey('Escape', 'Escape', 27);
  await waitFor(
    '!Array.from(document.querySelectorAll("[role=alertdialog]")).some(el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length))',
    1000,
    'confirm-dialog',
    'alertdialog vẫn hiển thị sau khi bấm Escape'
  );

  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-relead"',
    1500,
    'confirm-dialog',
    'Focus không trở về nút Chạy lại Lead sau khi đóng dialog bằng Escape'
  );

  const runsAfter = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => (Array.isArray(d.runs) ? d.runs.length : 0))
  `);
  if (runsAfter !== runsBefore) {
    throw new Error(`LỖI confirm-dialog: runs.length đã đổi từ ${runsBefore} sang ${runsAfter}`);
  }
  console.log('OK confirm-dialog');

  // Check C, raw-JSON dialog
  await waitFor(`
    (() => {
      const btn = document.querySelector("#ab-log button");
      return Boolean(btn && (btn.offsetWidth > 0 || btn.offsetHeight > 0));
    })()
  `, 5000, 'raw-dialog', 'Không tìm thấy dòng log hiển thị trong #ab-log');

  await evaluate(`
    (() => {
      const btn = document.querySelector("#ab-log button");
      if (btn) btn.click();
    })()
  `);

  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-raw-close"',
    1500,
    'raw-dialog',
    'document.activeElement.id không phải là ab-raw-close trong vòng 1.5s'
  );

  await pressKey('Escape', 'Escape', 27);
  await waitFor(
    '!Array.from(document.querySelectorAll("[role=dialog]")).some(el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length))',
    1000,
    'raw-dialog',
    'dialog vẫn hiển thị sau khi bấm Escape'
  );
  console.log('OK raw-dialog');

  // Check D, ledger paging + Enter selects a run
  await waitFor(
    'document.querySelectorAll("[data-ledger-row]").length === 20',
    5000,
    'ledger',
    'Không tìm thấy đúng 20 phần tử [data-ledger-row] ban đầu'
  );

  const moreVisible = await evaluate(`
    (() => {
      const btn = document.querySelector("[data-ledger-more]");
      if (!btn) return false;
      const visible = !!(btn.offsetWidth || btn.offsetHeight || btn.getClientRects().length);
      const text = btn.textContent.trim();
      return visible && text === "Hiện thêm 20";
    })()
  `);
  if (!moreVisible) {
    throw new Error('LỖI ledger: nút [data-ledger-more] không hiển thị hoặc không có chữ "Hiện thêm 20"');
  }

  await evaluate('document.querySelector("[data-ledger-more]").focus()');
  await pressKeyText('Enter', 'Enter', 13, '\r');

  await waitFor(`
    (() => {
      const rows = document.querySelectorAll("[data-ledger-row]");
      const btn = document.querySelector("[data-ledger-more]");
      const btnHidden = !btn || !(btn.offsetWidth || btn.offsetHeight || btn.getClientRects().length);
      return rows.length === 26 && btnHidden;
    })()
  `, 3000, 'ledger', 'Không hiển thị đủ 26 dòng hoặc nút Hiện thêm 20 chưa ẩn sau khi nhấn');

  const firstRowRunId = await evaluate(`
    (() => {
      const row = document.querySelector("[data-ledger-row]");
      if (!row) return null;
      row.focus();
      return row.getAttribute("data-run-id");
    })()
  `);
  if (!firstRowRunId) {
    throw new Error('LỖI ledger: không tìm thấy dòng ledger đầu tiên hoặc thiếu data-run-id');
  }

  await pressKeyText('Enter', 'Enter', 13, '\r');

  await waitFor(`
    (() => {
      const row = document.querySelector("[data-ledger-row]");
      const stream = document.getElementById("ab-stream-line");
      if (!row || !stream) return false;
      const isCurrent = row.getAttribute("aria-current") === "true";
      const isActive = document.activeElement === row;
      const streamHasId = stream.textContent.includes("run " + ${JSON.stringify(firstRowRunId)});
      return isCurrent && isActive && streamHasId;
    })()
  `, 3000, 'ledger', `Dòng ${firstRowRunId} không có aria-current="true", chưa giữ focus hoặc #ab-stream-line không chứa run ${firstRowRunId}`);
  console.log('OK ledger');

  // Check E, layout at 1280 / 1024 / 768 / 480 px. The left sidebar (md:w-64)
  // takes 256 px, so the pipeline region needs a 1280 px viewport to fit
  // without scrolling (the old 1024 px header layout had the same content width).
  for (const width of [1280, 1024, 768, 480]) {
    await cdp('Emulation.setDeviceMetricsOverride', {
      width,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await new Promise(r => setTimeout(r, 400));

    const info = await evaluate(`
      (() => {
        const root = document.documentElement;
        const pageOverflow = root.scrollWidth > root.clientWidth;
        const flowEl = document.querySelector('[data-flow]');
        const regionEl = document.querySelector('[data-flow-region]');
        const rowEl = document.querySelector('[data-ledger-row]');

        const flowDir = flowEl ? getComputedStyle(flowEl).flexDirection : null;
        const regionScrolls = regionEl ? regionEl.scrollWidth > regionEl.clientWidth : false;
        const rowDir = rowEl ? getComputedStyle(rowEl).flexDirection : null;

        let widest = '';
        let maxRight = 0;
        const all = document.querySelectorAll('*');
        for (let i = 0; i < all.length; i++) {
          const el = all[i];
          const rect = el.getBoundingClientRect();
          if (rect.right > maxRight) {
            maxRight = rect.right;
            const tag = el.tagName.toLowerCase();
            const id = el.id ? '#' + el.id : '';
            const cls = el.className && typeof el.className === 'string'
              ? '.' + el.className.trim().split(/\\s+/).slice(0, 3).join('.')
              : '';
            widest = tag + id + cls;
          }
        }

        return {
          pageOverflow,
          flowDir,
          regionScrolls,
          rowDir,
          widest,
        };
      })()
    `);

    if (info.pageOverflow) {
      throw new Error(`LỖI layout: ${width}px trang bị tràn ngang (phần tử rộng nhất: ${info.widest})`);
    }

    if (width === 1280) {
      if (info.flowDir !== 'row') {
        throw new Error(`LỖI layout: 1280px flowDir là "${info.flowDir}", kỳ vọng "row"`);
      }
      if (info.regionScrolls !== false) {
        throw new Error(`LỖI layout: 1280px regionScrolls là ${info.regionScrolls}, kỳ vọng false`);
      }
      if (info.rowDir !== 'row') {
        throw new Error(`LỖI layout: 1280px rowDir là "${info.rowDir}", kỳ vọng "row"`);
      }
    } else if (width === 1024) {
      if (info.flowDir !== 'row') {
        throw new Error(`LỖI layout: 1024px flowDir là "${info.flowDir}", kỳ vọng "row"`);
      }
      if (info.rowDir !== 'row') {
        throw new Error(`LỖI layout: 1024px rowDir là "${info.rowDir}", kỳ vọng "row"`);
      }
    } else if (width === 768) {
      if (info.flowDir !== 'row') {
        throw new Error(`LỖI layout: 768px flowDir là "${info.flowDir}", kỳ vọng "row"`);
      }
    } else if (width === 480) {
      if (info.flowDir !== 'column') {
        throw new Error(`LỖI layout: 480px flowDir là "${info.flowDir}", kỳ vọng "column"`);
      }
      if (info.rowDir !== 'column') {
        throw new Error(`LỖI layout: 480px rowDir là "${info.rowDir}", kỳ vọng "column"`);
      }
    }
  }

  await cdp('Emulation.clearDeviceMetricsOverride');
  console.log('OK layout');

  // Check F, gate buttons (task 1 waits at the Bạn gate): Tab from the flow
  // region reaches Duyệt plan then Trả về Lead, both inside the Bạn card, and
  // at 1024 px the Trả về Lead text is not clipped.
  await cdp('Emulation.setDeviceMetricsOverride', {
    width: 1024,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await new Promise(r => setTimeout(r, 400));

  await waitFor(`
    (() => {
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      const approve = document.getElementById("ab-act-approve");
      const ret = document.getElementById("ab-act-return_lead");
      return vis(approve) && vis(ret) && !approve.disabled && !ret.disabled;
    })()
  `, 5000, 'gate-buttons', 'Không thấy hai nút ab-act-approve và ab-act-return_lead hiển thị và đang bật');

  const gateInfo = await evaluate(`
    (() => {
      const approve = document.getElementById("ab-act-approve");
      const ret = document.getElementById("ab-act-return_lead");
      const inBan = (el) => Boolean(el && el.closest('[data-card="ban"]'));
      return {
        approveTag: approve.tagName.toLowerCase(),
        retTag: ret.tagName.toLowerCase(),
        approveInBan: inBan(approve),
        retInBan: inBan(ret),
        approveTab: approve.tabIndex,
        retTab: ret.tabIndex,
        retScroll: ret.scrollWidth,
        retClient: ret.clientWidth,
        retText: ret.textContent.trim(),
      };
    })()
  `);
  if (gateInfo.approveTag !== 'button' || gateInfo.retTag !== 'button') {
    throw new Error(`LỖI gate-buttons: ab-act-approve là <${gateInfo.approveTag}>, ab-act-return_lead là <${gateInfo.retTag}>, kỳ vọng <button>`);
  }
  if (!gateInfo.approveInBan || !gateInfo.retInBan) {
    throw new Error(`LỖI gate-buttons: nút không nằm trong [data-card="ban"] (approve: ${gateInfo.approveInBan}, return_lead: ${gateInfo.retInBan})`);
  }
  if (gateInfo.approveTab < 0 || gateInfo.retTab < 0) {
    throw new Error(`LỖI gate-buttons: tabIndex âm (approve: ${gateInfo.approveTab}, return_lead: ${gateInfo.retTab})`);
  }
  if (gateInfo.retText !== 'Trả về Lead') {
    throw new Error(`LỖI gate-buttons: chữ của ab-act-return_lead là "${gateInfo.retText}", kỳ vọng "Trả về Lead"`);
  }
  if (gateInfo.retScroll > gateInfo.retClient) {
    throw new Error(`LỖI gate-buttons: 1024px nút Trả về Lead bị cắt chữ (scrollWidth ${gateInfo.retScroll} > clientWidth ${gateInfo.retClient})`);
  }

  // Tab order: the flow region, then the two Bạn-card buttons.
  await evaluate('document.querySelector("[data-flow-region]").focus()');
  await pressKey('Tab', 'Tab', 9);
  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-approve"',
    1500,
    'gate-buttons',
    'Tab từ vùng pipeline không tới nút Duyệt plan (ab-act-approve)'
  );
  await pressKey('Tab', 'Tab', 9);
  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-return_lead"',
    1500,
    'gate-buttons',
    'Tab sau Duyệt plan không tới nút Trả về Lead (ab-act-return_lead)'
  );
  console.log('OK gate-buttons');

  // Check G, Trả về Lead dialog: Enter opens it with focus in the note,
  // Enter in the note inserts a newline, Escape closes without any POST.
  const taskBefore = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => ({ rev: d.rev, runs: Array.isArray(d.runs) ? d.runs.length : 0 }))
  `);
  if (!taskBefore || !taskBefore.rev) {
    throw new Error('LỖI return-dialog: GET /api/tasks/1 không có rev');
  }

  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitFor(`
    (() => {
      const dlg = document.querySelector("[role=alertdialog]");
      const open = Boolean(dlg && (dlg.offsetWidth || dlg.offsetHeight || dlg.getClientRects().length));
      return open && document.activeElement && document.activeElement.id === "ab-return-note";
    })()
  `, 1500, 'return-dialog', 'Enter trên Trả về Lead không mở dialog hoặc focus không ở #ab-return-note');

  await insertText('dòng 1');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await insertText('dòng 2');
  await new Promise(r => setTimeout(r, 200));

  const noteInfo = await evaluate(`
    (() => {
      const dlg = document.querySelector("[role=alertdialog]");
      const open = Boolean(dlg && (dlg.offsetWidth || dlg.offsetHeight || dlg.getClientRects().length));
      const note = document.getElementById("ab-return-note");
      return { open, value: note ? note.value : null, active: document.activeElement ? document.activeElement.id : '' };
    })()
  `);
  if (!noteInfo.open) {
    throw new Error('LỖI return-dialog: Enter trong ô ghi chú đã đóng dialog (kỳ vọng xuống dòng)');
  }
  if (noteInfo.value !== 'dòng 1\ndòng 2') {
    throw new Error(`LỖI return-dialog: giá trị #ab-return-note là ${JSON.stringify(noteInfo.value)}, kỳ vọng "dòng 1\\ndòng 2"`);
  }

  await pressKey('Escape', 'Escape', 27);
  await waitFor(
    '!Array.from(document.querySelectorAll("[role=alertdialog]")).some(el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length))',
    1000,
    'return-dialog',
    'dialog Trả về Lead vẫn hiển thị sau khi bấm Escape'
  );
  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-return_lead"',
    1500,
    'return-dialog',
    'Focus không trở về nút Trả về Lead sau khi đóng dialog bằng Escape'
  );

  const taskAfter = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => ({ rev: d.rev, runs: Array.isArray(d.runs) ? d.runs.length : 0 }))
  `);
  if (taskAfter.rev !== taskBefore.rev || taskAfter.runs !== taskBefore.runs) {
    throw new Error(`LỖI return-dialog: task 1 đã đổi sau Escape (rev ${taskBefore.rev} → ${taskAfter.rev}, runs ${taskBefore.runs} → ${taskAfter.runs})`);
  }
  console.log('OK return-dialog');

  // Check L (06-17), change-team dialog on task 1: Enter on `Đổi đội hình` opens it with focus on
  // the selected radio (the current team, doi-thu), the primary is disabled with its reason while
  // the selection is the current team, ArrowDown picks another team (primary enabled), Escape
  // closes without any POST and focus returns to `Đổi đội hình`.
  await evaluate('window.location.hash = "#/task/1"');
  await waitFor(`
    (() => {
      if (!document.querySelector('[data-screen="task"][data-loaded="yes"]')) return false;
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      const btn = document.getElementById("ab-act-change_team");
      return vis(btn) && !btn.disabled && btn.textContent.trim() === "Đổi đội hình";
    })()
  `, 5000, 'change-team', 'Không thấy nút "Đổi đội hình" (#ab-act-change_team) hiển thị và đang bật trên task/1');

  const teamBefore = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => ({ rev: d.rev, team: d.team ? d.team.name : null, history: Array.isArray(d.team_history) ? d.team_history.length : -1 }))
  `);
  await evaluate('document.getElementById("ab-act-change_team").focus()');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitFor(`
    (() => {
      const dlg = document.querySelector("[role=dialog][aria-labelledby=ab-tdlg-title]");
      const open = Boolean(dlg && (dlg.offsetWidth || dlg.offsetHeight || dlg.getClientRects().length));
      const a = document.activeElement;
      return open && a && a.name === "dlg-team" && a.value === "doi-thu" && a.checked;
    })()
  `, 3000, 'change-team', 'Enter trên "Đổi đội hình" không mở dialog hoặc focus không ở radio doi-thu đang chọn');

  const dlgState = await evaluate(`
    (() => {
      const ok = document.getElementById("ab-tdlg-ok");
      const reason = document.getElementById("ab-tdlg-reason");
      const chips = Array.from(document.querySelectorAll("[data-tdlg-option=doi-thu] span")).map(e => e.textContent.trim());
      return {
        title: document.getElementById("ab-tdlg-title").textContent.trim(),
        okDisabled: ok.disabled,
        okLabel: ok.textContent.trim(),
        reason: reason ? reason.textContent.trim() : '',
        current: chips.includes("đang dùng"),
      };
    })()
  `);
  if (dlgState.title !== 'Đổi đội hình cho task sua-app' || !dlgState.okDisabled || dlgState.okLabel !== 'Đổi đội hình'
      || dlgState.reason !== 'Chọn đội hình khác đội hình hiện tại.' || !dlgState.current) {
    throw new Error(`LỖI change-team: trạng thái dialog sai: ${JSON.stringify(dlgState)}`);
  }

  await pressKey('ArrowDown', 'ArrowDown', 40);
  await waitFor(`
    (() => {
      const ok = document.getElementById("ab-tdlg-ok");
      const a = document.activeElement;
      return a && a.name === "dlg-team" && a.value !== "doi-thu" && a.checked && !ok.disabled;
    })()
  `, 2000, 'change-team', 'ArrowDown không chọn đội hình khác hoặc nút "Đổi đội hình" vẫn bị tắt');

  await pressKey('Escape', 'Escape', 27);
  await waitFor(
    '!Array.from(document.querySelectorAll("[role=dialog]")).some(el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length))',
    1000,
    'change-team',
    'dialog đổi đội hình vẫn hiển thị sau khi bấm Escape'
  );
  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-change_team"',
    1500,
    'change-team',
    'Focus không trở về nút "Đổi đội hình" sau khi đóng dialog bằng Escape'
  );
  const teamAfter = await evaluate(`
    fetch('/api/tasks/1')
      .then(r => r.json())
      .then(d => ({ rev: d.rev, team: d.team ? d.team.name : null, history: Array.isArray(d.team_history) ? d.team_history.length : -1 }))
  `);
  if (teamAfter.rev !== teamBefore.rev || teamAfter.team !== teamBefore.team || teamAfter.history !== teamBefore.history) {
    throw new Error(`LỖI change-team: task 1 đã đổi sau Escape (${JSON.stringify(teamBefore)} → ${JSON.stringify(teamAfter)})`);
  }
  console.log('OK change-team');

  // Check M (06-17), quota stop on task 3 (Dev on deepseek ran out of quota): Tab reaches
  // `Chạy lại bước này bằng đội hình…`, Enter opens the re-run dialog with focus on the first
  // radio and nothing preselected, Escape closes it and focus returns to the banner button.
  // `Thử lại sau` collapses the banner to the hint with no request; a reload shows the full banner.
  await evaluate('window.location.hash = "#/task/3"');
  await waitFor(`
    (() => {
      if (!document.querySelector('[data-screen="task"][data-loaded="yes"]')) return false;
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      const btn = document.getElementById("ab-act-quota_rerun");
      return vis(document.querySelector("[data-stop-banner]")) && vis(btn) && !btn.disabled
        && btn.textContent.trim() === "Chạy lại bước này bằng đội hình…";
    })()
  `, 5000, 'task-quota', 'Không thấy banner quota với nút "Chạy lại bước này bằng đội hình…" đang bật trên task/3');

  const quotaSnapshot = `
    fetch('/api/tasks/3')
      .then(r => r.json())
      .then(d => ({ rev: d.rev, runs: Array.isArray(d.runs) ? d.runs.length : 0, team: d.team ? d.team.name : null, history: Array.isArray(d.team_history) ? d.team_history.length : -1 }))
  `;
  const quotaBefore = await evaluate(quotaSnapshot);

  await evaluate('document.activeElement && document.activeElement.blur()');
  let reached = false;
  for (let i = 0; i < 12 && !reached; i++) {
    await pressKey('Tab', 'Tab', 9);
    reached = await evaluate('Boolean(document.activeElement && document.activeElement.id === "ab-act-quota_rerun")');
  }
  if (!reached) {
    throw new Error('LỖI task-quota: Tab không tới được nút "Chạy lại bước này bằng đội hình…" trong 12 lần bấm');
  }

  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitFor(`
    (() => {
      const dlg = document.querySelector("[role=dialog][aria-labelledby=ab-tdlg-title]");
      const open = Boolean(dlg && (dlg.offsetWidth || dlg.offsetHeight || dlg.getClientRects().length));
      const first = document.querySelector('input[name="dlg-team"]');
      return open && first && document.activeElement === first;
    })()
  `, 3000, 'task-quota', 'Enter trên nút quota không mở dialog hoặc focus không ở radio đầu tiên');

  const rerunState = await evaluate(`
    (() => {
      const ok = document.getElementById("ab-tdlg-ok");
      const reason = document.getElementById("ab-tdlg-reason");
      const radios = Array.from(document.querySelectorAll('input[name="dlg-team"]'));
      const opt = (name) => {
        const el = document.querySelector('[data-tdlg-option="' + name + '"]');
        return el ? el.textContent.replace(/\\s+/g, ' ').trim() : '';
      };
      return {
        title: document.getElementById("ab-tdlg-title").textContent.trim(),
        anyChecked: radios.some(r => r.checked),
        okDisabled: ok.disabled,
        okLabel: ok.textContent.trim(),
        reason: reason ? reason.textContent.trim() : '',
        tiet: opt("tiet-kiem"),
        mac: opt("mac-dinh"),
      };
    })()
  `);
  if (rerunState.title !== 'Chạy lại bước Code bằng đội hình nào?' || rerunState.anyChecked || !rerunState.okDisabled
      || rerunState.okLabel !== 'Chạy lại bước này' || rerunState.reason !== 'Chọn một đội hình.') {
    throw new Error(`LỖI task-quota: trạng thái dialog chạy lại sai: ${JSON.stringify(rerunState)}`);
  }
  if (!rerunState.tiet.includes('Bước Code sẽ chạy bằng: deepseek') || !rerunState.tiet.includes('vẫn là deepseek')
      || !rerunState.mac.includes('Bước Code sẽ chạy bằng: claude') || rerunState.mac.includes('vẫn là')) {
    throw new Error(`LỖI task-quota: dòng engine / chip của các đội hình sai: ${JSON.stringify(rerunState)}`);
  }

  await pressKey('Escape', 'Escape', 27);
  await waitFor(
    '!Array.from(document.querySelectorAll("[role=dialog]")).some(el => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length))',
    1000,
    'task-quota',
    'dialog chạy lại vẫn hiển thị sau khi bấm Escape'
  );
  await waitFor(
    'document.activeElement && document.activeElement.id === "ab-act-quota_rerun"',
    1500,
    'task-quota',
    'Focus không trở về nút "Chạy lại bước này bằng đội hình…" sau khi đóng dialog bằng Escape'
  );

  await evaluate('document.getElementById("ab-act-quota_later").focus()');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitFor(`
    (() => {
      const banner = document.querySelector("[data-stop-banner]");
      const text = banner ? banner.textContent.replace(/\\s+/g, ' ') : '';
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      return text.includes("Đã hoãn. Khi quota hồi phục, bấm ") && text.includes("ở dưới để chạy lại bằng engine deepseek.")
        && !vis(document.getElementById("ab-act-quota_rerun")) && !vis(document.getElementById("ab-act-quota_later"));
    })()
  `, 2000, 'task-quota', '"Thử lại sau" không thu banner về dòng "Đã hoãn. ..." (hoặc nút còn hiện)');
  const quotaAfter = await evaluate(quotaSnapshot);
  if (JSON.stringify(quotaAfter) !== JSON.stringify(quotaBefore)) {
    throw new Error(`LỖI task-quota: task 3 đã đổi sau Escape / Thử lại sau (${JSON.stringify(quotaBefore)} → ${JSON.stringify(quotaAfter)})`);
  }

  await evaluate('location.reload()');
  await waitFor(`
    (() => {
      if (!document.querySelector('[data-screen="task"][data-loaded="yes"]')) return false;
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      return vis(document.getElementById("ab-act-quota_rerun")) && vis(document.getElementById("ab-act-quota_later"));
    })()
  `, 8000, 'task-quota', 'Sau khi tải lại trang banner quota không hiện đủ hai nút');
  console.log('OK task-quota');

  // Check N (06-18), human step on task 4 (Hà holds the Code step): the callout sits before the
  // action strip in the tab order, Tab from the flow region reaches `Xong` without landing on a
  // strip control, and Enter on `Sao chép tên nhánh` shows `Đã sao chép tên nhánh`. `Xong` is
  // enabled only on the task branch, so the check switches the smoke repo to task/nguoi-lam and
  // back; it never presses `Xong` or `Trả lại` (no job starts, nothing changes on task 4).
  const smokeRepo = process.env.AB_SMOKE_REPO;
  if (!smokeRepo) {
    throw new Error('LỖI task-human: thiếu biến AB_SMOKE_REPO (thư mục repo của smoke)');
  }
  execFileSync('git', ['-C', smokeRepo, 'checkout', '-q', 'task/nguoi-lam']);
  try {
    await evaluate('window.location.hash = "#/task/4"');
    await waitFor(`
      (() => {
        if (!document.querySelector('[data-screen="task"][data-loaded="yes"]')) return false;
        const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
        const btn = document.getElementById("ab-act-human_done");
        return vis(document.querySelector("[data-human-callout]")) && vis(btn) && !btn.disabled
          && btn.textContent.trim() === "Xong";
      })()
    `, 12000, 'task-human', 'Không thấy callout người làm với nút "Xong" đang bật trên task/4');

    const humanSnapshot = `
      fetch('/api/tasks/4')
        .then(r => r.json())
        .then(d => ({ rev: d.rev, runs: Array.isArray(d.runs) ? d.runs.length : 0, mode: d.human ? d.human.mode : null }))
    `;
    const humanBefore = await evaluate(humanSnapshot);

    // Tab order: callout before the action strip (the strip's own buttons are disabled here, and
    // none of them may take focus before Xong).
    const order = await evaluate(`
      (() => {
        const callout = document.querySelector("[data-human-callout]");
        const strip = document.querySelector("[data-action-strip]");
        if (!callout || !strip) return { ok: false, why: "thiếu callout hoặc dải hành động" };
        const pos = callout.compareDocumentPosition(strip);
        return { ok: true, calloutBeforeStrip: Boolean(pos & Node.DOCUMENT_POSITION_FOLLOWING) };
      })()
    `);
    if (!order.ok) {
      throw new Error(`LỖI task-human: ${order.why}`);
    }

    await evaluate('document.activeElement && document.activeElement.blur()');
    await evaluate('document.querySelector("[data-flow-region]").focus()');
    let humanReached = false;
    const visited = [];
    for (let i = 0; i < 15 && !humanReached; i++) {
      await pressKey('Tab', 'Tab', 9);
      const info = await evaluate(`
        (() => {
          const el = document.activeElement;
          return {
            id: el ? el.id : "",
            text: el ? el.textContent.trim().slice(0, 40) : "",
            inStrip: Boolean(el && el.closest && el.closest("[data-action-strip]")),
            inCallout: Boolean(el && el.closest && el.closest("[data-human-callout]")),
          };
        })()
      `);
      visited.push(info.id || info.text);
      if (info.inStrip) {
        throw new Error(`LỖI task-human: Tab đã rơi vào dải hành động (${info.id || info.text}) trước khi tới "Xong" (đã qua: ${visited.join(' → ')})`);
      }
      humanReached = info.id === 'ab-act-human_done';
    }
    if (!humanReached) {
      throw new Error(`LỖI task-human: Tab không tới được nút "Xong" trong 15 lần bấm (đã qua: ${visited.join(' → ')})`);
    }
    if (!visited.includes('Sao chép tên nhánh')) {
      throw new Error(`LỖI task-human: "Sao chép tên nhánh" không nằm trước "Xong" trong thứ tự Tab (đã qua: ${visited.join(' → ')})`);
    }

    await evaluate(`
      (() => {
        const callout = document.querySelector("[data-human-callout]");
        const copy = Array.from(callout.querySelectorAll("button")).find(b => b.textContent.trim() === "Sao chép tên nhánh");
        copy.focus();
      })()
    `);
    await pressKeyText('Enter', 'Enter', 13, '\r');
    await waitFor(`
      (() => {
        const el = document.querySelector("[data-human-copy]");
        const vis = Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
        return vis && el.textContent.trim() === "Đã sao chép tên nhánh";
      })()
    `, 3000, 'task-human', 'Enter trên "Sao chép tên nhánh" không hiện "Đã sao chép tên nhánh"');

    const humanAfter = await evaluate(humanSnapshot);
    if (JSON.stringify(humanAfter) !== JSON.stringify(humanBefore)) {
      throw new Error(`LỖI task-human: task 4 đã đổi (${JSON.stringify(humanBefore)} → ${JSON.stringify(humanAfter)})`);
    }
  } finally {
    execFileSync('git', ['-C', smokeRepo, 'checkout', '-q', 'task/sua-app']);
  }
  console.log('OK task-human');

  // Check H, Quy trình picker on Giao việc: ArrowDown moves the selection
  // from mac-dinh to nhanh (native radio group).
  await evaluate('window.location.hash = "#/assign"');
  await waitFor(`
    (() => {
      if (!document.querySelector('[data-screen="assign"][data-loaded="yes"]')) return false;
      const radios = Array.from(document.querySelectorAll('input[type="radio"][name="workflow"]'));
      const checked = radios.find(r => r.checked);
      return radios.length >= 2 && checked && checked.value === "mac-dinh";
    })()
  `, 5000, 'picker', 'Màn hình assign không có nhóm radio Quy trình với mac-dinh được chọn');

  await evaluate(`
    (() => {
      const checked = Array.from(document.querySelectorAll('input[type="radio"][name="workflow"]')).find(r => r.checked);
      checked.focus();
    })()
  `);
  await pressKey('ArrowDown', 'ArrowDown', 40);
  await new Promise(r => setTimeout(r, 200));

  const pickerInfo = await evaluate(`
    (() => {
      const radios = Array.from(document.querySelectorAll('input[type="radio"][name="workflow"]'));
      const checked = radios.find(r => r.checked);
      const active = document.activeElement;
      return {
        checked: checked ? checked.value : null,
        activeValue: active && active.name === "workflow" ? active.value : null,
      };
    })()
  `);
  if (pickerInfo.checked !== 'nhanh') {
    throw new Error(`LỖI picker: sau ArrowDown radio được chọn là "${pickerInfo.checked}", kỳ vọng "nhanh"`);
  }
  if (pickerInfo.activeValue !== 'nhanh') {
    throw new Error(`LỖI picker: sau ArrowDown focus ở "${pickerInfo.activeValue}", kỳ vọng radio "nhanh"`);
  }
  console.log('OK picker');

  // Check I, layout of the stopped task (task 2) at 1024 / 768 / 480 px.
  await evaluate('window.location.hash = "#/task/2"');
  await waitFor(`
    (() => {
      if (!document.querySelector('[data-screen="task"][data-loaded="yes"]')) return false;
      const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
      return vis(document.querySelector("[data-stop-banner]")) && vis(document.querySelector("[data-flow]"));
    })()
  `, 5000, 'layout-5.2', 'Màn hình task/2 không hiện [data-stop-banner] và [data-flow]');

  for (const width of [1024, 768, 480]) {
    await cdp('Emulation.setDeviceMetricsOverride', {
      width,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await new Promise(r => setTimeout(r, 400));

    const info = await evaluate(`
      (() => {
        const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
        const root = document.documentElement;
        const pageOverflow = root.scrollWidth > root.clientWidth;
        const flowEl = document.querySelector('[data-flow]');
        const flowDir = flowEl ? getComputedStyle(flowEl).flexDirection : null;
        const banner = vis(document.querySelector('[data-stop-banner]'));

        let widest = '';
        let maxRight = 0;
        const all = document.querySelectorAll('*');
        for (let i = 0; i < all.length; i++) {
          const el = all[i];
          const rect = el.getBoundingClientRect();
          if (rect.right > maxRight) {
            maxRight = rect.right;
            const tag = el.tagName.toLowerCase();
            const id = el.id ? '#' + el.id : '';
            const cls = el.className && typeof el.className === 'string'
              ? '.' + el.className.trim().split(/\\s+/).slice(0, 3).join('.')
              : '';
            widest = tag + id + cls;
          }
        }

        return { pageOverflow, flowDir, banner, widest };
      })()
    `);

    if (info.pageOverflow) {
      throw new Error(`LỖI layout-5.2: ${width}px trang task/2 bị tràn ngang (phần tử rộng nhất: ${info.widest})`);
    }
    const wantDir = width === 480 ? 'column' : 'row';
    if (info.flowDir !== wantDir) {
      throw new Error(`LỖI layout-5.2: ${width}px flowDir là "${info.flowDir}", kỳ vọng "${wantDir}"`);
    }
    if (!info.banner) {
      throw new Error(`LỖI layout-5.2: ${width}px [data-stop-banner] không hiển thị`);
    }
  }

  await cdp('Emulation.clearDeviceMetricsOverride');
  console.log('OK layout-5.2');

  // Check J, work board (#/board): Enter and Space select a card, the panel opens,
  // a panel neighbour moves selection and focus, Esc clears and returns focus.
  const BVIS = 'const vis = (el) => Boolean(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));';
  const boardCard = (key) => `document.querySelector('[data-item-key="${key}"]')`;
  // Same as waitFor, but a timeout also reports the state of the board, so a failure under load
  // names what the page really looked like.
  const boardSnapshot = () => evaluate(`
    JSON.stringify({
      active: document.activeElement ? document.activeElement.tagName + '#' + document.activeElement.id : null,
      hasFocus: document.hasFocus(),
      pressed: Array.from(document.querySelectorAll('[data-item-key]')).map(c => c.dataset.itemKey + '=' + c.getAttribute('aria-pressed')),
      panel: (() => { const p = document.querySelector('[data-board-panel]'); return p ? p.style.display || 'shown' : 'none'; })(),
      deps: Array.from(document.querySelectorAll('[data-panel-dep]')).map(l => l.dataset.panelDep),
    })
  `);
  const waitBoard = async (expr, ms, name, detail) => {
    try {
      return await waitFor(expr, ms, name, detail);
    } catch (err) {
      let snap = '';
      try { snap = ' ' + await boardSnapshot(); } catch (_) {}
      throw new Error(err.message + snap);
    }
  };
  // Alpine shows an x-show element a frame after the state changes (requestAnimationFrame), so an
  // element can exist before it is visible and focus() on it is silently ignored. Poll until the
  // element is visible and really holds focus; only then press a key on it.
  const focusBoardEl = (elExpr, label) => waitBoard(`
    (() => {
      ${BVIS}
      const el = ${elExpr};
      if (!vis(el)) return false;
      el.focus();
      return document.activeElement === el;
    })()
  `, 5000, 'board-keys', `${label} không hiện ra hoặc không nhận được focus`);

  await evaluate('window.location.hash = "#/board"');
  await waitBoard(`
    (() => {
      ${BVIS}
      if (!document.querySelector('[data-screen="board"][data-loaded="yes"]')) return false;
      return vis(${boardCard('02-02')});
    })()
  `, 8000, 'board-keys', 'Màn hình board không hiện thẻ 02-02');

  const boardEpic = await evaluate('document.getElementById("board-epic").value');
  if (boardEpic !== '02') {
    throw new Error(`LỖI board-keys: board-epic là "${boardEpic}", kỳ vọng "02"`);
  }

  await focusBoardEl('document.getElementById("board-item-02-02")', 'Thẻ 02-02');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitBoard(`
    (() => {
      ${BVIS}
      const c = ${boardCard('02-02')};
      const panel = document.querySelector('[data-board-panel]');
      return c && c.getAttribute('aria-pressed') === 'true' && vis(panel) && panel.textContent.includes('02-02');
    })()
  `, 5000, 'board-keys', 'Enter trên thẻ 02-02 không chọn thẻ hoặc không mở bảng bên (data-board-panel)');

  const dimInfo = await evaluate(`
    (() => ({
      other: ${boardCard('02-03')}.className,
      upstream: ${boardCard('02-01')}.className,
      selected: ${boardCard('02-02')}.className,
    }))()
  `);
  if (!dimInfo.other.includes('opacity-25')) {
    throw new Error('LỖI board-keys: thẻ 02-03 ngoài chuỗi phụ thuộc không bị làm mờ (opacity-25)');
  }
  if (dimInfo.upstream.includes('opacity-25')) {
    throw new Error('LỖI board-keys: thẻ 02-01 trong chuỗi phụ thuộc bị làm mờ');
  }
  if (!dimInfo.selected.includes('ring-cyan-600')) {
    throw new Error('LỖI board-keys: thẻ 02-02 được chọn thiếu vòng ring-cyan-600');
  }

  // Put focus on the panel's Đóng button first: with focus already on the card, "focus is back on
  // the card" would hold before Esc did anything, and AB.focusLater (which runs on a timer) could
  // still fire after the next step has moved focus on.
  await focusBoardEl(`Array.from(document.querySelectorAll('[data-board-panel] button')).find(b => b.textContent.trim() === 'Đóng')`, 'Nút Đóng của bảng bên');
  await pressKey('Escape', 'Escape', 27);
  await waitBoard(`
    (() => {
      ${BVIS}
      const cards = Array.from(document.querySelectorAll('[data-item-key]'));
      const none = cards.length > 0 && cards.every(c => c.getAttribute('aria-pressed') === 'false');
      return none && !vis(document.querySelector('[data-board-panel]')) && document.activeElement && document.activeElement.id === 'board-item-02-02';
    })()
  `, 5000, 'board-keys', 'Esc không bỏ chọn, không đóng bảng bên hoặc focus không trở về thẻ board-item-02-02');

  await focusBoardEl('document.getElementById("board-item-02-03")', 'Thẻ 02-03');
  await pressKeyText(' ', 'Space', 32, ' ');
  await waitBoard(`${boardCard('02-03')}.getAttribute('aria-pressed') === 'true'`, 5000, 'board-keys', 'Space trên thẻ 02-03 không chọn thẻ');

  await focusBoardEl('document.querySelector(\'[data-panel-dep="02-01"] button\')', 'Nút Phụ thuộc 02-01 của bảng bên');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitBoard(`
    ${boardCard('02-01')}.getAttribute('aria-pressed') === 'true'
      && ${boardCard('02-03')}.getAttribute('aria-pressed') === 'false'
      && document.activeElement && document.activeElement.id === 'board-item-02-01'
  `, 5000, 'board-keys', 'Nút Phụ thuộc 02-01 không chuyển chọn và focus sang thẻ 02-01');

  // Esc from inside the panel must land on the card, not on <body>.
  await focusBoardEl(`Array.from(document.querySelectorAll('[data-board-panel] button')).find(b => b.textContent.trim() === 'Đóng')`, 'Nút Đóng của bảng bên');
  await pressKey('Escape', 'Escape', 27);
  await waitBoard(`
    (() => {
      ${BVIS}
      const none = Array.from(document.querySelectorAll('[data-item-key]')).every(c => c.getAttribute('aria-pressed') === 'false');
      return none && !vis(document.querySelector('[data-board-panel]')) && document.activeElement && document.activeElement.id === 'board-item-02-01';
    })()
  `, 5000, 'board-keys', 'Esc từ nút Đóng của bảng bên không đưa focus về thẻ board-item-02-01');
  console.log('OK board-keys');

  // Check K, board layout at 1024 / 768 / 480 px with every epic and the panel open.
  await evaluate(`
    (() => {
      const sel = document.getElementById('board-epic');
      sel.value = 'all';
      sel.dispatchEvent(new Event('change', { bubbles: true }));
    })()
  `);
  await waitBoard(`
    (() => {
      ${BVIS}
      const cards = Array.from(document.querySelectorAll('[data-item-key]')).filter(vis);
      return cards.length === 6 && Boolean(document.querySelector('[data-rail="1"]'));
    })()
  `, 8000, 'board-layout', 'Mọi epic không hiện đủ sáu thẻ và một cột thu gọn [data-rail="1"]');

  await focusBoardEl('document.getElementById("board-item-02-01")', 'Thẻ 02-01');
  await pressKeyText('Enter', 'Enter', 13, '\r');
  await waitBoard(`
    (() => {
      ${BVIS}
      return ${boardCard('02-01')}.getAttribute('aria-pressed') === 'true' && vis(document.querySelector('[data-board-panel]'));
    })()
  `, 5000, 'board-layout', 'Không chọn được thẻ 02-01 để mở bảng bên');

  for (const width of [1024, 768, 480]) {
    await cdp('Emulation.setDeviceMetricsOverride', {
      width,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await new Promise(r => setTimeout(r, 400));

    const info = await evaluate(`
      (() => {
        ${BVIS}
        const root = document.documentElement;
        const pageOverflow = root.scrollWidth > root.clientWidth;
        const region = document.querySelector('[data-board-region]');
        const regionScrolls = region ? region.scrollWidth > region.clientWidth : false;
        const panelVisible = vis(document.querySelector('[data-board-panel]'));

        let widest = '';
        let maxRight = 0;
        const all = document.querySelectorAll('*');
        for (let i = 0; i < all.length; i++) {
          const el = all[i];
          const rect = el.getBoundingClientRect();
          if (rect.right > maxRight) {
            maxRight = rect.right;
            const tag = el.tagName.toLowerCase();
            const id = el.id ? '#' + el.id : '';
            const cls = el.className && typeof el.className === 'string'
              ? '.' + el.className.trim().split(/\\s+/).slice(0, 3).join('.')
              : '';
            widest = tag + id + cls;
          }
        }

        return { pageOverflow, regionScrolls, panelVisible, widest };
      })()
    `);

    if (info.pageOverflow) {
      throw new Error(`LỖI board-layout: ${width}px trang bị tràn ngang (phần tử rộng nhất: ${info.widest})`);
    }
    if (!info.panelVisible) {
      throw new Error(`LỖI board-layout: ${width}px bảng bên [data-board-panel] không hiển thị`);
    }
    if (width === 480 && !info.regionScrolls) {
      throw new Error('LỖI board-layout: 480px vùng [data-board-region] không tự cuộn ngang (scrollWidth không lớn hơn clientWidth)');
    }
  }

  await cdp('Emulation.clearDeviceMetricsOverride');
  await pressKey('Escape', 'Escape', 27);
  await waitBoard(`
    (() => {
      ${BVIS}
      const none = Array.from(document.querySelectorAll('[data-item-key]')).every(c => c.getAttribute('aria-pressed') === 'false');
      return none && !vis(document.querySelector('[data-board-panel]')) && document.activeElement && document.activeElement.id === 'board-item-02-01';
    })()
  `, 5000, 'board-layout', 'Esc cuối không bỏ chọn, không đóng bảng bên hoặc focus không về thẻ board-item-02-01');
  console.log('OK board-layout');

  ws.close();
}

main().catch((err) => {
  console.error(err.message || String(err));
  try {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.close();
    }
  } catch (_) {}
  process.exit(1);
});

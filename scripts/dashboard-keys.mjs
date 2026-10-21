// scripts/dashboard-keys.mjs — headless keyboard checks for dashboard
// plain Node ESM, no dependencies, global fetch and WebSocket

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

  // Check E, layout at 1024 / 768 / 480 px
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

    if (width === 1024) {
      if (info.flowDir !== 'row') {
        throw new Error(`LỖI layout: 1024px flowDir là "${info.flowDir}", kỳ vọng "row"`);
      }
      if (info.regionScrolls !== false) {
        throw new Error(`LỖI layout: 1024px regionScrolls là ${info.regionScrolls}, kỳ vọng false`);
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

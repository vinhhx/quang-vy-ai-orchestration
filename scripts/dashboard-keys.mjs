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

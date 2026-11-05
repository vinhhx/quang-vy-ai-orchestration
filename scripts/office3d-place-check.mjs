// scripts/office3d-place-check.mjs — Kiểm tra thuần Node.js cho logic xếp phòng
import assert from 'node:assert/strict';
import {
  ROOMS,
  ROOMS_BY_ID,
  placeTask,
  placeProposal,
  placeIntake,
  classifyU1Task,
  isStudioTask,
  projectColor,
} from '../agent-brain/assets/dashboard/js/office-rooms.js';

console.log('--- Kiểm tra office-rooms.js: Định nghĩa 8 phòng ---');
assert.equal(ROOMS.length, 8, 'Phải có đúng 8 phòng');
const roomIds = ROOMS.map((r) => r.id);
const expectedRooms = ['ba', 'ky_thuat', 'qa', 'dieu_phoi', 'giam_doc', 'studio', 'phong_nghi', 'kho_luu'];
for (const id of expectedRooms) {
  assert(roomIds.includes(id), `Thiếu phòng ${id}`);
  assert(ROOMS_BY_ID[id], `ROOMS_BY_ID thiếu ${id}`);
  assert(ROOMS_BY_ID[id].name, `Phòng ${id} thiếu tên tiếng Việt`);
}
console.log('✓ 8 phòng ban hợp lệ:', roomIds.join(', '));

console.log('\n--- Kiểm tra 1: Task done -> Kho lưu ---');
{
  const task = {
    id: 101,
    group: 'done',
    status: 'succeeded',
    title: 'Đã xong tính năng',
    project: 'web-ban-hang',
  };
  const placed = placeTask(task, {});
  assert.equal(placed.room, 'kho_luu', 'Done task phải vào kho_luu');
  assert.equal(placed.flags.done, true);
  assert.equal(classifyU1Task(task, {}), 'done');
  console.log('✓ Done task vào đúng Kho lưu');
}

console.log('\n--- Kiểm tra 2: Quota stop & attention -> Phòng nghỉ ---');
{
  // 2a: Có quota_detail
  const taskQuota = {
    id: 102,
    group: 'stopped',
    quota_detail: 'Hết quota Anthropic',
    title: 'Task chạm quota',
    project: 'web-ban-hang',
  };
  const placedQuota = placeTask(taskQuota, {});
  assert.equal(placedQuota.room, 'phong_nghi', 'Task có quota_detail phải vào phong_nghi');
  assert.equal(placedQuota.flags.quota, true);
  assert.equal(classifyU1Task(taskQuota, {}), 'stopped');

  // 2b: Nằm trong board.quota_stops
  const taskBoardQuota = {
    id: 103,
    group: 'running', // U1 ghi đè group nếu có trong quota_stops
    title: 'Task trong quota_stops',
    project: 'web-ban-hang',
  };
  const board = { quota_stops: [{ task_id: 103, engine: 'claude', resume_at: 1700000000 }] };
  const placedBoardQuota = placeTask(taskBoardQuota, board);
  assert.equal(placedBoardQuota.room, 'phong_nghi');
  assert.equal(placedBoardQuota.flags.quota, true);
  assert.equal(classifyU1Task(taskBoardQuota, board), 'stopped');

  // 2c: auto_state === 'stopped' do lỗi (không phải quota)
  const taskError = {
    id: 104,
    group: 'attention',
    auto_state: 'stopped',
    title: 'Task bị dừng do lỗi',
    project: 'web-ban-hang',
  };
  const placedError = placeTask(taskError, {});
  assert.equal(placedError.room, 'phong_nghi');
  assert.equal(placedError.flags.error, true);
  assert.equal(placedError.flags.quota, false);
  assert.equal(classifyU1Task(taskError, {}), 'stopped');
  console.log('✓ Quota stop và task lỗi vào đúng Phòng nghỉ');
}

console.log('\n--- Kiểm tra 3: Task waiting -> Phòng Giám đốc ---');
{
  const taskWait = {
    id: 105,
    group: 'waiting',
    title: 'Chờ duyệt plan',
    project: 'web-ban-hang',
  };
  const placed = placeTask(taskWait, {});
  assert.equal(placed.room, 'giam_doc', 'Task waiting phải vào giam_doc');
  assert.equal(placed.flags.waiting, true);
  assert.equal(classifyU1Task(taskWait, {}), 'waiting');
  console.log('✓ Task waiting vào đúng Phòng Giám đốc');
}

console.log('\n--- Kiểm tra 4: Running theo step role -> BA, Kỹ thuật, QA ---');
{
  // 4a: BA / Phân tích (lead)
  const taskLead = {
    id: 106,
    group: 'running',
    current_step: 'lead',
    steps: [
      { role: 'lead', state: 'dang_chay', label: 'Lập plan' },
      { role: 'dev', state: 'cho', label: 'Dev' },
    ],
    project: 'web-ban-hang',
  };
  const placedLead = placeTask(taskLead, {});
  assert.equal(placedLead.room, 'ba', 'Role lead phải vào ba');
  assert.equal(classifyU1Task(taskLead, {}), 'running');

  // 4b: BA / Phân tích (plan-review)
  const taskPlanRev = {
    id: 107,
    group: 'running',
    steps: [
      { role: 'lead', state: 'xong', label: 'Plan' },
      { role: 'plan-review', state: 'dang_chay', label: 'Duyệt plan' },
    ],
    project: 'web-ban-hang',
  };
  const placedPlanRev = placeTask(taskPlanRev, {});
  assert.equal(placedPlanRev.room, 'ba', 'Role plan-review phải vào ba');

  // 4c: Kỹ thuật (dev)
  const taskDev = {
    id: 108,
    group: 'running',
    current_step: 'dev',
    steps: [
      { role: 'lead', state: 'xong', label: 'Plan' },
      { role: 'dev', state: 'dang_chay', label: 'Code' },
    ],
    project: 'web-ban-hang',
  };
  const placedDev = placeTask(taskDev, {});
  assert.equal(placedDev.room, 'ky_thuat', 'Role dev phải vào ky_thuat');

  // 4d: Kỹ thuật (code-review)
  const taskCodeRev = {
    id: 109,
    group: 'running',
    steps: [
      { role: 'dev', state: 'xong', label: 'Dev' },
      { role: 'code-review', state: 'dang_chay', label: 'Review' },
    ],
    project: 'web-ban-hang',
  };
  const placedCodeRev = placeTask(taskCodeRev, {});
  assert.equal(placedCodeRev.room, 'ky_thuat', 'Role code-review phải vào ky_thuat');

  // 4e: Kỹ thuật (fix)
  const taskFix = {
    id: 110,
    group: 'running',
    steps: [
      { role: 'qa', state: 'can_sua', label: 'QA báo lỗi' },
      { role: 'fix', state: 'dang_chay', label: 'Sửa lỗi' },
    ],
    project: 'web-ban-hang',
  };
  const placedFix = placeTask(taskFix, {});
  assert.equal(placedFix.room, 'ky_thuat', 'Role fix phải vào ky_thuat');

  // 4f: QA
  const taskQA = {
    id: 111,
    group: 'running',
    current_step: 'qa',
    steps: [
      { role: 'lead', state: 'xong', label: 'Plan' },
      { role: 'dev', state: 'xong', label: 'Code' },
      { role: 'qa', state: 'dang_chay', label: 'Kiểm thử' },
    ],
    project: 'web-ban-hang',
  };
  const placedQA = placeTask(taskQA, {});
  assert.equal(placedQA.room, 'qa', 'Role qa phải vào qa');
  assert.equal(classifyU1Task(taskQA, {}), 'running');

  console.log('✓ Các bước BA (lead/plan-review), Kỹ thuật (dev/code-review/fix), QA vào đúng phòng');
}

console.log('\n--- Kiểm tra 5: Studio (dự án media hoặc workflow generate-video) ---');
{
  const projects = [
    { id: 'web-ban-hang', type: 'code' },
    { id: 'video-tiktok', type: 'media' },
  ];

  // 5a: Dự án type=media
  const taskMedia = {
    id: 112,
    group: 'running',
    project: 'video-tiktok',
    current_step: 'dev',
    steps: [{ role: 'dev', state: 'dang_chay', label: 'Dựng cảnh' }],
  };
  assert.equal(isStudioTask(taskMedia, projects), true);
  const placedMedia = placeTask(taskMedia, {}, projects);
  assert.equal(placedMedia.room, 'studio', 'Dự án media running phải vào Studio');
  assert.equal(placedMedia.flags.studio, true);

  // 5b: Workflow generate-video trên dự án khác
  const taskVideoWf = {
    id: 113,
    group: 'running',
    project: 'web-ban-hang',
    workflow: 'generate-video',
    current_step: 'lead',
    steps: [{ role: 'lead', state: 'dang_chay', label: 'Lên kịch bản' }],
  };
  assert.equal(isStudioTask(taskVideoWf, projects), true);
  const placedVideoWf = placeTask(taskVideoWf, {}, projects);
  assert.equal(placedVideoWf.room, 'studio', 'Workflow generate-video running phải vào Studio');
  console.log('✓ Dự án media và workflow generate-video vào đúng Studio');
}

console.log('\n--- Kiểm tra 6: Đề xuất (proposals) và Hàng chờ (intake) ---');
{
  // 6a: Proposal pending đứng ở cửa phòng Giám đốc
  const propPending = { id: 1, status: 'pending', priority: 'urgent' };
  const placedPending = placeProposal(propPending);
  assert.equal(placedPending.room, 'giam_doc');
  assert.equal(placedPending.desk, 'door');
  assert.equal(placedPending.flags.urgent, true);

  // 6b: Proposal approved đứng xếp hàng chờ slot ở phòng Điều phối
  const propApproved = { id: 2, status: 'approved', priority: 'normal' };
  const placedApproved = placeProposal(propApproved);
  assert.equal(placedApproved.room, 'dieu_phoi');
  assert.equal(placedApproved.desk, 'waiting_slot');

  // 6c: Intake item ở phòng Điều phối
  const intakeItem = { id: 10, status: 'queued', priority: 'urgent' };
  const placedIntake = placeIntake(intakeItem);
  assert.equal(placedIntake.room, 'dieu_phoi');
  assert.equal(placedIntake.flags.urgent, true);
  console.log('✓ Proposal pending ở cửa phòng Giám đốc, approved và intake ở phòng Điều phối');
}

console.log('\n--- Kiểm tra 7: Tính duy nhất của phòng và nhất quán với U1 ---');
{
  const tasks = [
    { id: 1, group: 'done' },
    { id: 2, group: 'attention', auto_state: 'stopped' },
    { id: 3, group: 'waiting' },
    { id: 4, group: 'running', current_step: 'lead' },
    { id: 5, group: 'running', current_step: 'dev' },
    { id: 6, group: 'running', current_step: 'qa' },
    { id: 7, group: 'running', project: 'video-tiktok' },
  ];
  const projects = [{ id: 'video-tiktok', type: 'media' }];

  for (const t of tasks) {
    const p = placeTask(t, {}, projects);
    assert(p.room, `Task ${t.id} phải có room`);
    assert(ROOMS_BY_ID[p.room], `Room ${p.room} của task ${t.id} phải tồn tại`);
    const u1 = classifyU1Task(t, {});
    if (u1 === 'done') assert.equal(p.room, 'kho_luu');
    if (u1 === 'stopped') assert.equal(p.room, 'phong_nghi');
    if (u1 === 'waiting') assert.equal(p.room, 'giam_doc');
    if (u1 === 'running') {
      assert(['ba', 'ky_thuat', 'qa', 'studio'].includes(p.room), `Running task ${t.id} phải thuộc 1 trong 4 phòng việc`);
    }
  }
  console.log('✓ Tất cả task được gán duy nhất 1 phòng hợp lệ và khớp với phân loại U1');
}

console.log('\n--- Kiểm tra 8: Màu sắc dự án (projectColor) ---');
{
  const c1 = projectColor('web-ban-hang');
  const c2 = projectColor('video-tiktok');
  const c3 = projectColor('web-ban-hang');
  assert(c1.startsWith('#'), 'Mã màu phải là hex');
  assert.equal(c1, c3, 'Cùng ID dự án phải ra cùng màu');
  console.log('✓ projectColor băm màu ổn định:', { 'web-ban-hang': c1, 'video-tiktok': c2 });
}

console.log('\n=== TẤT CẢ KIỂM TRA CHO office3d-place-check ĐỀU ĐẠT ===');

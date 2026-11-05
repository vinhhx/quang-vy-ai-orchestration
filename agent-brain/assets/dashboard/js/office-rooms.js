// js/office-rooms.js — Định nghĩa 8 phòng ban và quy tắc xếp nhân vật
// ES Module thuần, không phụ thuộc Three.js hay DOM, kiểm thử được bằng Node.js.

export const ROOMS = [
  {
    id: 'ba',
    num: 3,
    name: 'BA / Phân tích',
    short: 'BA',
    color: '#0284c7', // Sky
    row: 0,
    col: 0,
    desc: 'Lập kế hoạch và xem xét yêu cầu',
    furniture: ['whiteboard', 'desks'],
  },
  {
    id: 'ky_thuat',
    num: 4,
    name: 'Kỹ thuật',
    short: 'Kỹ thuật',
    color: '#10b981', // Emerald
    row: 0,
    col: 1,
    desc: 'Lập trình, sửa lỗi và rà soát mã nguồn',
    furniture: ['server_rack', 'monitors', 'desks'],
  },
  {
    id: 'qa',
    num: 5,
    name: 'QA',
    short: 'QA',
    color: '#f59e0b', // Amber
    row: 0,
    col: 2,
    desc: 'Kiểm thử, nghiệm thu và đo chất lượng',
    furniture: ['test_devices', 'desks'],
  },
  {
    id: 'dieu_phoi',
    num: 1,
    name: 'Điều phối',
    short: 'Lễ tân',
    color: '#64748b', // Slate
    row: 1,
    col: 0,
    desc: 'Tiếp nhận yêu cầu và xếp hàng chờ slot',
    furniture: ['reception_desk', 'queue_bench'],
  },
  {
    id: 'giam_doc',
    num: 2,
    name: 'Phòng Giám đốc',
    short: 'Bạn duyệt',
    color: '#8b5cf6', // Violet
    row: 1,
    col: 1,
    desc: 'Phê duyệt kế hoạch, đề xuất và ngân sách',
    furniture: ['director_desk', 'chair_executive', 'approval_tray'],
  },
  {
    id: 'studio',
    num: 8,
    name: 'Studio',
    short: 'Studio',
    color: '#ec4899', // Pink
    row: 1,
    col: 2,
    desc: 'Sản xuất kịch bản, âm thanh và video',
    furniture: ['camera_tripod', 'studio_lights', 'desks'],
  },
  {
    id: 'phong_nghi',
    num: 6,
    name: 'Phòng nghỉ',
    short: 'Nghỉ',
    color: '#ef4444', // Red
    row: 1,
    col: 3,
    desc: 'Nơi tạm dừng do hạn mức hoặc chờ can thiệp',
    furniture: ['sofa', 'coffee_table', 'water_cooler'],
  },
  {
    id: 'kho_luu',
    num: 7,
    name: 'Kho lưu',
    short: 'Kho',
    color: '#6b7280', // Gray
    row: 1,
    col: 4,
    desc: 'Lưu trữ hồ sơ các công việc đã hoàn thành',
    furniture: ['archive_cabinets', 'boxes'],
  },
];

export const ROOMS_BY_ID = Object.freeze(
  ROOMS.reduce((acc, r) => {
    acc[r.id] = r;
    return acc;
  }, {})
);

export const PROJECT_PALETTE = [
  '#3b82f6', // blue
  '#10b981', // emerald
  '#f59e0b', // amber
  '#8b5cf6', // purple
  '#ec4899', // pink
  '#06b6d4', // cyan
  '#6366f1', // indigo
  '#84cc16', // lime
];

export function projectColor(projectId) {
  if (!projectId) return PROJECT_PALETTE[0];
  var hash = 0;
  var str = String(projectId);
  for (var i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) & 0xffffffff;
  }
  var idx = Math.abs(hash) % PROJECT_PALETTE.length;
  return PROJECT_PALETTE[idx];
}

export function buildQuotaStopsMap(board) {
  var map = {};
  if (board && Array.isArray(board.quota_stops)) {
    for (var i = 0; i < board.quota_stops.length; i++) {
      var qs = board.quota_stops[i];
      if (qs && qs.task_id != null) {
        map[qs.task_id] = qs;
      }
    }
  }
  return map;
}

export function classifyU1Task(task, boardOrQuotaMap) {
  if (!task) return 'other';
  var qMap = (boardOrQuotaMap && boardOrQuotaMap.quota_stops)
    ? buildQuotaStopsMap(boardOrQuotaMap)
    : (boardOrQuotaMap || {});

  if (
    task.quota_detail ||
    qMap[task.id] ||
    task.auto_state === 'stopped' ||
    task.group === 'attention'
  ) {
    return 'stopped';
  }
  if (task.group === 'running') {
    return 'running';
  }
  if (task.group === 'waiting') {
    return 'waiting';
  }
  if (task.group === 'done') {
    return 'done';
  }
  return 'other';
}

export function getCurrentStep(task) {
  if (!task || !Array.isArray(task.steps)) return null;
  for (var i = 0; i < task.steps.length; i++) {
    var s = task.steps[i];
    if (s && (s.state === 'dang_chay' || s.state === 'running')) {
      return s;
    }
  }
  return null;
}

export function isStudioTask(task, projects) {
  if (!task) return false;
  if (task.workflow === 'generate-video') {
    return true;
  }
  if (projects && Array.isArray(projects)) {
    var p = projects.find(function (proj) {
      return proj && proj.id === task.project;
    });
    if (p && (p.type === 'media' || p.kind === 'media')) {
      return true;
    }
  }
  if (task.profile === 'media' || task.project === 'media') {
    return true;
  }
  return false;
}

/**
 * Xếp một task vào đúng phòng theo thứ tự ưu tiên của Plan B1:
 * 1. Kho lưu (done)
 * 2. Phòng nghỉ (stopped do quota hoặc lỗi)
 * 3. Phòng Giám đốc (waiting cần bạn duyệt)
 * 4. Studio (running dự án media hoặc workflow generate-video)
 * 5. BA / Phân tích (running role lead hoặc plan-review)
 * 6. Kỹ thuật (running role dev, code-review hoặc fix)
 * 7. QA (running role qa)
 * 8. Fallback (dieu_phoi)
 */
export function placeTask(task, board, projects) {
  if (!task) {
    return { room: 'dieu_phoi', desk: 'bench', flags: {} };
  }

  var u1Group = classifyU1Task(task, board);

  // 1. Kho lưu (#7)
  if (u1Group === 'done') {
    return {
      room: 'kho_luu',
      desk: 'archive',
      flags: { done: true },
    };
  }

  // 2. Phòng nghỉ (#6)
  if (u1Group === 'stopped') {
    var qMap = (board && board.quota_stops) ? buildQuotaStopsMap(board) : (board || {});
    var isQuota = !!(task.quota_detail || qMap[task.id]);
    return {
      room: 'phong_nghi',
      desk: 'rest',
      flags: {
        quota: isQuota,
        error: !isQuota,
        stopped: true,
      },
    };
  }

  // 3. Phòng Giám đốc (#2)
  if (u1Group === 'waiting') {
    return {
      room: 'giam_doc',
      desk: 'director',
      flags: { waiting: true },
    };
  }

  // Các trường hợp running
  if (u1Group === 'running') {
    // 4. Studio (#8)
    if (isStudioTask(task, projects)) {
      var curStep = getCurrentStep(task);
      var role = (curStep && curStep.role) || task.current_step || 'media';
      return {
        room: 'studio',
        desk: role,
        flags: { running: true, studio: true },
      };
    }

    // Xác định role của bước đang chạy
    var cur = getCurrentStep(task);
    var curRole = (cur && cur.role) || task.current_step || 'dev';

    // 5. BA / Phân tích (#3)
    if (curRole === 'lead' || curRole === 'plan-review') {
      return {
        room: 'ba',
        desk: curRole,
        flags: { running: true, analyzing: true },
      };
    }

    // 6. Kỹ thuật (#4)
    if (curRole === 'dev' || curRole === 'code-review' || curRole === 'fix') {
      return {
        room: 'ky_thuat',
        desk: curRole === 'fix' ? 'dev' : curRole,
        flags: { running: true, coding: true },
      };
    }

    // 7. QA (#5)
    if (curRole === 'qa') {
      return {
        room: 'qa',
        desk: 'qa',
        flags: { running: true, testing: true },
      };
    }

    // Fallback cho running nếu có role lạ
    return {
      room: 'ky_thuat',
      desk: 'dev',
      flags: { running: true },
    };
  }

  // Fallback
  return {
    room: 'dieu_phoi',
    desk: 'bench',
    flags: {},
  };
}

export function placeProposal(proposal) {
  if (!proposal) {
    return { room: 'dieu_phoi', desk: 'bench', flags: {} };
  }
  var isUrgent = proposal.priority === 'urgent';
  if (proposal.status === 'pending') {
    return {
      room: 'giam_doc',
      desk: 'door',
      flags: { proposal: true, pending: true, urgent: isUrgent },
    };
  }
  if (proposal.status === 'approved') {
    return {
      room: 'dieu_phoi',
      desk: 'waiting_slot',
      flags: { proposal: true, approved: true, urgent: isUrgent },
    };
  }
  return {
    room: 'dieu_phoi',
    desk: 'bench',
    flags: { proposal: true, urgent: isUrgent },
  };
}

export function placeIntake(item) {
  if (!item) {
    return { room: 'dieu_phoi', desk: 'bench', flags: {} };
  }
  var isUrgent = item.priority === 'urgent';
  var isTriaging = item.status === 'triaging';
  return {
    room: 'dieu_phoi',
    desk: isTriaging ? 'desk' : 'intake_bench',
    flags: { intake: true, triaging: isTriaging, urgent: isUrgent },
  };
}

if (typeof window !== 'undefined') {
  window.ABOfficeRooms = {
    ROOMS: ROOMS,
    ROOMS_BY_ID: ROOMS_BY_ID,
    PROJECT_PALETTE: PROJECT_PALETTE,
    projectColor: projectColor,
    buildQuotaStopsMap: buildQuotaStopsMap,
    classifyU1Task: classifyU1Task,
    getCurrentStep: getCurrentStep,
    isStudioTask: isStudioTask,
    placeTask: placeTask,
    placeProposal: placeProposal,
    placeIntake: placeIntake,
  };
}


// js/office3d.js — Văn phòng 3D trên màn Điều phối (Three.js ES Module)
import * as THREE from '/vendor/three.module.js';
import {
  ROOMS,
  ROOMS_BY_ID,
  placeTask,
  placeProposal,
  placeIntake,
  classifyU1Task,
  projectColor,
} from '/js/office-rooms.js';

// Kích thước và toạ độ mặt bằng văn phòng
const ROOM_LAYOUT = {
  // Hàng 0: 3 phòng công đoạn chính (BA -> Kỹ thuật -> QA)
  ba:        { x: -25, z: -12, w: 22, d: 15, doorX: -25, doorZ: -4.5 },
  ky_thuat:  { x: 0,   z: -12, w: 24, d: 15, doorX: 0,   doorZ: -4.5 },
  qa:        { x: 25,  z: -12, w: 22, d: 15, doorX: 25,  doorZ: -4.5 },

  // Hàng 1: Điều phối, Phòng Giám đốc, Studio, Phòng nghỉ, Kho lưu
  dieu_phoi: { x: -33, z: 12,  w: 16, d: 15, doorX: -33, doorZ: 4.5 },
  giam_doc:  { x: -16, z: 12,  w: 16, d: 15, doorX: -16, doorZ: 4.5 },
  studio:    { x: 0,   z: 12,  w: 14, d: 15, doorX: 0,   doorZ: 4.5 },
  phong_nghi:{ x: 16,  z: 12,  w: 16, d: 15, doorX: 16,  doorZ: 4.5 },
  kho_luu:   { x: 33,  z: 12,  w: 16, d: 15, doorX: 33,  doorZ: 4.5 },
};

// Vị trí ghế ngồi cơ bản trong từng phòng
function getDeskPositions(roomId) {
  const r = ROOM_LAYOUT[roomId] || { x: 0, z: 0 };
  const cx = r.x;
  const cz = r.z;

  switch (roomId) {
    case 'ba':
      return [
        { x: cx - 4.5, z: cz - 1.5, rot: 0 },
        { x: cx,       z: cz - 1.5, rot: 0 },
        { x: cx + 4.5, z: cz - 1.5, rot: 0 },
        { x: cx - 3.0, z: cz + 3.0, rot: Math.PI },
        { x: cx + 3.0, z: cz + 3.0, rot: Math.PI },
      ];
    case 'ky_thuat':
      return [
        { x: cx - 6.0, z: cz - 1.5, rot: 0 },
        { x: cx,       z: cz - 1.5, rot: 0 },
        { x: cx + 6.0, z: cz - 1.5, rot: 0 },
        { x: cx - 4.0, z: cz + 3.0, rot: Math.PI },
        { x: cx + 4.0, z: cz + 3.0, rot: Math.PI },
      ];
    case 'qa':
      return [
        { x: cx - 4.5, z: cz - 1.5, rot: 0 },
        { x: cx,       z: cz - 1.5, rot: 0 },
        { x: cx + 4.5, z: cz - 1.5, rot: 0 },
        { x: cx - 3.0, z: cz + 3.0, rot: Math.PI },
        { x: cx + 3.0, z: cz + 3.0, rot: Math.PI },
      ];
    case 'studio':
      return [
        { x: cx - 2.5, z: cz - 1.5, rot: 0 },
        { x: cx + 2.5, z: cz - 1.5, rot: 0 },
        { x: cx,       z: cz + 3.0, rot: Math.PI },
      ];
    case 'giam_doc':
      return [
        { x: cx,       z: cz - 2.0, rot: 0 },       // Bàn Giám đốc
        { x: cx - 3.5, z: cz + 2.5, rot: -Math.PI / 4 }, // Ghế tiếp khách 1
        { x: cx + 3.5, z: cz + 2.5, rot: Math.PI / 4 },  // Ghế tiếp khách 2
        { x: cx - 3.0, z: r.doorZ - 1.0, rot: 0 }, // Cửa phòng chờ duyệt
        { x: cx + 3.0, z: r.doorZ - 1.0, rot: 0 },
      ];
    case 'phong_nghi':
      return [
        { x: cx - 3.5, z: cz - 2.0, rot: 0 }, // Sofa 1
        { x: cx + 3.5, z: cz - 2.0, rot: 0 }, // Sofa 2
        { x: cx - 3.0, z: cz + 2.5, rot: Math.PI }, // Ghế nghỉ
        { x: cx + 3.0, z: cz + 2.5, rot: Math.PI },
      ];
    case 'kho_luu':
      return [
        { x: cx - 3.5, z: cz - 1.5, rot: 0 },
        { x: cx + 3.5, z: cz - 1.5, rot: 0 },
        { x: cx,       z: cz + 2.0, rot: Math.PI },
        { x: cx - 3.5, z: cz + 3.5, rot: 0 },
        { x: cx + 3.5, z: cz + 3.5, rot: 0 },
      ];
    case 'dieu_phoi':
    default:
      return [
        { x: cx,       z: cz - 2.0, rot: 0 },        // Bàn lễ tân
        { x: cx - 4.0, z: cz + 2.0, rot: Math.PI },  // Ghế chờ 1 (gấp)
        { x: cx - 1.5, z: cz + 2.0, rot: Math.PI },  // Ghế chờ 2
        { x: cx + 1.5, z: cz + 2.0, rot: Math.PI },  // Ghế chờ 3
        { x: cx + 4.0, z: cz + 2.0, rot: Math.PI },  // Ghế chờ 4
      ];
  }
}

/**
 * Tạo văn phòng 3D và gắn vào container
 * @param {HTMLElement} container — DOM node chứa canvas và lớp phủ nhãn
 * @param {Object} callbacks — các hàm gọi lại khi tương tác
 */
export function createOffice(container, callbacks) {
  callbacks = callbacks || {};

  // Kiểm tra hỗ trợ WebGL
  const testCanvas = document.createElement('canvas');
  const gl = testCanvas.getContext('webgl2') || testCanvas.getContext('webgl');
  if (!gl) {
    console.warn('Thiết bị không hỗ trợ WebGL, văn phòng 3D sẽ bị ẩn.');
    return null;
  }

  // Tùy chọn chuyển động người dùng (reduced-motion)
  let reducedMotion = false;
  // Render-loop state. Declared before anything can call requestDirty() (applyThemeColors runs
  // during setup), otherwise the `let` bindings are still in their temporal dead zone.
  let isDirty = true;
  let rafId = null;
  let isDisposed = false;
  try {
    reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch (e) {}

  // 1. Tạo Canvas & Renderer
  const width = container.clientWidth || 800;
  const height = container.clientHeight || 450;

  const renderer = new THREE.WebGLRenderer({
    antialias: true,
    alpha: true,
    powerPreference: 'high-performance',
  });
  renderer.setSize(width, height);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x000000, 0); // Trong suốt, dùng màu theme CSS
  container.style.position = 'relative';
  container.style.userSelect = 'none';
  renderer.domElement.className = 'w-full h-full block rounded-xl focus:outline-none';
  renderer.domElement.tabIndex = 0;
  container.appendChild(renderer.domElement);

  // Lớp phủ HTML cho nhãn phòng và tooltip
  const overlay = document.createElement('div');
  overlay.className = 'pointer-events-none absolute inset-0 overflow-hidden';
  container.appendChild(overlay);

  // Tạo Scene & Camera
  const scene = new THREE.Scene();

  const camera = new THREE.PerspectiveCamera(35, width / height, 0.5, 500);
  const defaultTarget = new THREE.Vector3(0, 0, 0);
  const cameraTarget = defaultTarget.clone();

  // Góc camera Isometric
  let orbitAngle = 0;
  let pitchAngle = 0.95; // ~54.4 độ
  const DEFAULT_DISTANCE = 84; // fits all eight rooms in the default view
  let cameraDistance = DEFAULT_DISTANCE;

  function updateCameraTransform() {
    const y = cameraTarget.y + cameraDistance * Math.cos(pitchAngle);
    const horizDist = cameraDistance * Math.sin(pitchAngle);
    const x = cameraTarget.x + horizDist * Math.sin(orbitAngle);
    const z = cameraTarget.z + horizDist * Math.cos(orbitAngle);
    camera.position.set(x, y, z);
    camera.lookAt(cameraTarget);
  }
  updateCameraTransform();

  // 2. Ánh sáng
  const hemiLight = new THREE.HemisphereLight(0xf1f5f9, 0x94a3b8, 1.2);
  scene.add(hemiLight);

  const dirLight = new THREE.DirectionalLight(0xffffff, 1.5);
  dirLight.position.set(30, 50, 40);
  scene.add(dirLight);

  // Quản lý theme sáng / tối
  function applyThemeColors() {
    const isDark = document.documentElement.classList.contains('dark');
    if (isDark) {
      hemiLight.color.setHex(0x334155);
      hemiLight.groundColor.setHex(0x0f172a);
      hemiLight.intensity = 1.0;
      dirLight.color.setHex(0x94a3b8);
      dirLight.intensity = 1.3;
    } else {
      hemiLight.color.setHex(0xf8fafc);
      hemiLight.groundColor.setHex(0xcbd5e1);
      hemiLight.intensity = 1.3;
      dirLight.color.setHex(0xffffff);
      dirLight.intensity = 1.6;
    }
    requestDirty();
  }
  applyThemeColors();

  // Theo dõi đổi class dark trên <html>
  const themeObserver = new MutationObserver(() => applyThemeColors());
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });

  // 3. Dựng Kiến trúc Văn phòng (Sàn, Tường, Nội thất đặc trưng)
  const officeGroup = new THREE.Group();
  scene.add(officeGroup);

  // Sàn hành lang trung tâm
  const corridorGeo = new THREE.PlaneGeometry(86, 38);
  corridorGeo.rotateX(-Math.PI / 2);
  const corridorMat = new THREE.MeshLambertMaterial({ color: 0xe2e8f0 });
  const corridorMesh = new THREE.Mesh(corridorGeo, corridorMat);
  corridorMesh.position.y = -0.02;
  officeGroup.add(corridorMesh);

  // Shared Geometries & Materials để tối ưu GPU
  const wallMat = new THREE.MeshLambertMaterial({ color: 0x94a3b8 });
  const deskMat = new THREE.MeshLambertMaterial({ color: 0xcfd8dc });
  const chairMat = new THREE.MeshLambertMaterial({ color: 0x475569 });
  const monitorMat = new THREE.MeshLambertMaterial({ color: 0x1e293b });
  const screenGlowMat = new THREE.MeshBasicMaterial({ color: 0x38bdf8 });

  const deskGeo = new THREE.BoxGeometry(2.2, 0.8, 1.2);
  const chairGeo = new THREE.BoxGeometry(0.8, 0.9, 0.8);
  const monitorGeo = new THREE.BoxGeometry(0.9, 0.6, 0.1);

  // Tạo từng phòng ban
  const roomBadges = [];

  ROOMS.forEach((room) => {
    const cfg = ROOM_LAYOUT[room.id];
    if (!cfg) return;

    const roomGroup = new THREE.Group();
    roomGroup.position.set(cfg.x, 0, cfg.z);

    // Sàn phòng có màu riêng nhạt
    const floorGeo = new THREE.PlaneGeometry(cfg.w - 0.4, cfg.d - 0.4);
    floorGeo.rotateX(-Math.PI / 2);
    const floorMat = new THREE.MeshLambertMaterial({ color: new THREE.Color(room.color).lerp(new THREE.Color(0xffffff), 0.75) });
    const floorMesh = new THREE.Mesh(floorGeo, floorMat);
    floorMesh.position.y = 0.01;
    roomGroup.add(floorMesh);

    // Tường thấp bao quanh (cao 0.9)
    const wallHeight = 0.9;
    const wallThick = 0.25;
    const halfW = cfg.w / 2;
    const halfD = cfg.d / 2;

    // Tường sau
    const backWallGeo = new THREE.BoxGeometry(cfg.w, wallHeight, wallThick);
    const backWall = new THREE.Mesh(backWallGeo, wallMat);
    backWall.position.set(0, wallHeight / 2, -halfD);
    roomGroup.add(backWall);

    // Tường trái & phải
    const sideWallGeo = new THREE.BoxGeometry(wallThick, wallHeight, cfg.d);
    const leftWall = new THREE.Mesh(sideWallGeo, wallMat);
    leftWall.position.set(-halfW, wallHeight / 2, 0);
    roomGroup.add(leftWall);

    const rightWall = new THREE.Mesh(sideWallGeo, wallMat);
    rightWall.position.set(halfW, wallHeight / 2, 0);
    roomGroup.add(rightWall);

    // Tường trước có khe cửa (door gap rộng 3.2 ở giữa)
    const frontWallW = (cfg.w - 3.2) / 2;
    if (frontWallW > 0) {
      const frontLeftGeo = new THREE.BoxGeometry(frontWallW, wallHeight, wallThick);
      const frontLeft = new THREE.Mesh(frontLeftGeo, wallMat);
      frontLeft.position.set(-halfW + frontWallW / 2, wallHeight / 2, halfD);
      roomGroup.add(frontLeft);

      const frontRight = new THREE.Mesh(frontLeftGeo, wallMat);
      frontRight.position.set(halfW - frontWallW / 2, wallHeight / 2, halfD);
      roomGroup.add(frontRight);
    }

    // Đồ vật đặc trưng cho từng phòng
    addRoomDecorations(room.id, roomGroup, cfg, { deskGeo, deskMat, chairGeo, chairMat, monitorGeo, monitorMat, screenGlowMat });

    officeGroup.add(roomGroup);

    // Tạo badge HTML cho tên phòng
    const badgeEl = document.createElement('div');
    badgeEl.className = 'absolute -translate-x-1/2 -translate-y-full text-[11px] font-semibold px-2 py-0.5 rounded shadow pointer-events-auto cursor-pointer transition-transform hover:scale-105 bg-white/90 dark:bg-slate-800/90 text-slate-800 dark:text-slate-100 border border-slate-300 dark:border-slate-700 select-none';
    badgeEl.innerHTML = `<span>${room.name}</span> <span class="badge-count ml-1 font-bold text-indigo-600 dark:text-indigo-400">(0)</span>`;
    badgeEl.addEventListener('click', () => {
      focusPoint(new THREE.Vector3(cfg.x, 0, cfg.z), 38);
    });
    overlay.appendChild(badgeEl);

    roomBadges.push({
      roomId: room.id,
      name: room.name,
      worldPos: new THREE.Vector3(cfg.x, 1.4, cfg.z - cfg.d / 2 + 1.2),
      el: badgeEl,
      countEl: badgeEl.querySelector('.badge-count'),
      count: 0,
    });
  });

  // Thêm nội thất từng phòng
  function addRoomDecorations(id, parent, cfg, mats) {
    if (id === 'ba') {
      // Bảng trắng lớn ở tường sau
      const wbGeo = new THREE.BoxGeometry(6.0, 1.8, 0.08);
      const wbMat = new THREE.MeshLambertMaterial({ color: 0xffffff });
      const wb = new THREE.Mesh(wbGeo, wbMat);
      wb.position.set(0, 1.4, -cfg.d / 2 + 0.1);
      parent.add(wb);
    } else if (id === 'ky_thuat') {
      // Tủ server rack ở góc tường
      const rackGeo = new THREE.BoxGeometry(1.6, 2.2, 1.4);
      const rackMat = new THREE.MeshLambertMaterial({ color: 0x1e293b });
      const rack = new THREE.Mesh(rackGeo, rackMat);
      rack.position.set(-cfg.w / 2 + 1.6, 1.1, -cfg.d / 2 + 1.5);
      parent.add(rack);
    } else if (id === 'qa') {
      // Kệ thiết bị kiểm thử
      const shelfGeo = new THREE.BoxGeometry(4.0, 1.6, 0.8);
      const shelfMat = new THREE.MeshLambertMaterial({ color: 0x78716c });
      const shelf = new THREE.Mesh(shelfGeo, shelfMat);
      shelf.position.set(cfg.w / 2 - 2.5, 0.8, -cfg.d / 2 + 1.0);
      parent.add(shelf);
    } else if (id === 'studio') {
      // Đèn và máy quay trên chân 3 chạc
      const tripodGeo = new THREE.CylinderGeometry(0.08, 0.4, 1.8, 6);
      const tripodMat = new THREE.MeshLambertMaterial({ color: 0x111827 });
      const tripod = new THREE.Mesh(tripodGeo, tripodMat);
      tripod.position.set(-cfg.w / 2 + 2.5, 0.9, -cfg.d / 2 + 2.5);
      parent.add(tripod);

      const camBoxGeo = new THREE.BoxGeometry(0.6, 0.4, 0.8);
      const camBox = new THREE.Mesh(camBoxGeo, tripodMat);
      camBox.position.set(-cfg.w / 2 + 2.5, 1.9, -cfg.d / 2 + 2.5);
      parent.add(camBox);
    } else if (id === 'giam_doc') {
      // Bàn giám đốc lớn
      const execDeskGeo = new THREE.BoxGeometry(4.0, 0.85, 2.0);
      const execDeskMat = new THREE.MeshLambertMaterial({ color: 0x581c87 });
      const execDesk = new THREE.Mesh(execDeskGeo, execDeskMat);
      execDesk.position.set(0, 0.425, -2.0);
      parent.add(execDesk);
    } else if (id === 'phong_nghi') {
      // Sofa dài
      const sofaGeo = new THREE.BoxGeometry(5.0, 0.7, 1.6);
      const sofaMat = new THREE.MeshLambertMaterial({ color: 0xd97706 });
      const sofa = new THREE.Mesh(sofaGeo, sofaMat);
      sofa.position.set(0, 0.35, -2.5);
      parent.add(sofa);

      // Bàn trà tròn
      const tableGeo = new THREE.CylinderGeometry(1.2, 1.2, 0.4, 16);
      const tableMat = new THREE.MeshLambertMaterial({ color: 0x92400e });
      const table = new THREE.Mesh(tableGeo, tableMat);
      table.position.set(0, 0.2, 0.5);
      parent.add(table);
    } else if (id === 'kho_luu') {
      // Các tủ tài liệu
      for (let i = -1; i <= 1; i++) {
        const cabGeo = new THREE.BoxGeometry(1.6, 2.2, 1.0);
        const cabMat = new THREE.MeshLambertMaterial({ color: 0x475569 });
        const cab = new THREE.Mesh(cabGeo, cabMat);
        cab.position.set(i * 3.2, 1.1, -cfg.d / 2 + 1.2);
        parent.add(cab);
      }
    } else if (id === 'dieu_phoi') {
      // Quầy lễ tân
      const recDeskGeo = new THREE.BoxGeometry(4.5, 0.95, 1.4);
      const recDeskMat = new THREE.MeshLambertMaterial({ color: 0x334155 });
      const recDesk = new THREE.Mesh(recDeskGeo, recDeskMat);
      recDesk.position.set(0, 0.475, -2.0);
      parent.add(recDesk);
    }
  }

  // 4. Quản lý Nhân vật (Characters)
  const characters = new Map(); // key = "task:12" hoặc "prop:5" hoặc "intake:3"
  const trails = []; // Vệt sáng mờ dần

  // Shared geometry cho nhân vật
  const bodyGeo = new THREE.CapsuleGeometry(0.35, 0.6, 8, 16);
  const headGeo = new THREE.SphereGeometry(0.32, 16, 16);
  const ringGeo = new THREE.RingGeometry(0.38, 0.52, 16);
  ringGeo.rotateX(-Math.PI / 2);

  // Status icon geometry
  const iconWarningGeo = new THREE.ConeGeometry(0.18, 0.35, 8);
  const iconPendingGeo = new THREE.SphereGeometry(0.12, 8, 8);

  function createCharacterMesh(item, itemType) {
    const group = new THREE.Group();

    // Vòng tròn dưới chân (đổi màu khi chọn hoặc Gấp)
    const isUrgent = item.priority === 'urgent';
    const ringMat = new THREE.MeshBasicMaterial({
      color: isUrgent ? 0xef4444 : 0x64748b,
      transparent: true,
      opacity: isUrgent ? 0.9 : 0.4,
      side: THREE.DoubleSide,
    });
    const ring = new THREE.Mesh(ringGeo, ringMat);
    ring.position.y = 0.03;
    group.add(ring);

    // Thân nhân vật (áo nhuộm theo dự án)
    const pColor = projectColor(item.project);
    const bodyMat = new THREE.MeshLambertMaterial({ color: pColor });
    const body = new THREE.Mesh(bodyGeo, bodyMat);
    body.position.y = 0.65;
    group.add(body);

    // Đầu nhân vật
    const headMat = new THREE.MeshLambertMaterial({ color: 0xfde047 });
    const head = new THREE.Mesh(headGeo, headMat);
    head.position.y = 1.35;
    group.add(head);

    // Biểu tượng trạng thái trên đầu
    if (isUrgent || item.auto_state === 'stopped' || item.quota_detail) {
      const warnMat = new THREE.MeshBasicMaterial({ color: 0xef4444 });
      const warnMesh = new THREE.Mesh(iconWarningGeo, warnMat);
      warnMesh.position.y = 1.9;
      group.add(warnMesh);
    } else if (item.group === 'running') {
      const runMat = new THREE.MeshBasicMaterial({ color: 0x38bdf8 });
      const runMesh = new THREE.Mesh(iconPendingGeo, runMat);
      runMesh.position.y = 1.85;
      group.add(runMesh);
    }

    // Gắn metadata phục vụ raycasting và tương tác
    group.userData = {
      id: item.id,
      item: item,
      itemType: itemType,
      ringMesh: ring,
      bodyMesh: body,
      headMesh: head,
      currentPos: new THREE.Vector3(0, 0, 0),
      targetPos: new THREE.Vector3(0, 0, 0),
      waypoints: [],
      isMoving: false,
      moveProgress: 1,
      bobOffset: Math.random() * Math.PI * 2,
    };

    scene.add(group);
    return group;
  }

  // Tạo đường đi gấp khúc (waypoint) qua hành lang và cửa phòng
  function createWaypoints(startPos, targetPos, startRoomId, targetRoomId) {
    if (startRoomId === targetRoomId || !startRoomId || !targetRoomId) {
      return [targetPos.clone()];
    }

    const r1 = ROOM_LAYOUT[startRoomId];
    const r2 = ROOM_LAYOUT[targetRoomId];
    if (!r1 || !r2) return [targetPos.clone()];

    const pts = [];
    // 1. Đi tới cửa phòng hiện tại
    pts.push(new THREE.Vector3(r1.doorX, 0, r1.doorZ));
    // 2. Bước ra hành lang
    pts.push(new THREE.Vector3(r1.doorX, 0, 0));
    // 3. Đi dọc hành lang tới cửa phòng đích
    pts.push(new THREE.Vector3(r2.doorX, 0, 0));
    // 4. Bước vào cửa phòng đích
    pts.push(new THREE.Vector3(r2.doorX, 0, r2.doorZ));
    // 5. Tới bàn làm việc đích
    pts.push(targetPos.clone());
    return pts;
  }

  // Thêm vệt sáng trên sàn khi nhân vật di chuyển
  function addTrailSegment(p1, p2, colorHex) {
    if (reducedMotion) return;
    const dir = new THREE.Vector3().subVectors(p2, p1);
    const len = dir.length();
    if (len < 0.1) return;

    const trailGeo = new THREE.PlaneGeometry(0.3, len);
    trailGeo.rotateX(-Math.PI / 2);
    const trailMat = new THREE.MeshBasicMaterial({
      color: colorHex || 0x38bdf8,
      transparent: true,
      opacity: 0.7,
      side: THREE.DoubleSide,
    });
    const mesh = new THREE.Mesh(trailGeo, trailMat);
    mesh.position.copy(p1).add(dir.clone().multiplyScalar(0.5));
    mesh.position.y = 0.02;
    mesh.lookAt(p2.x, mesh.position.y, p2.z);

    scene.add(mesh);
    trails.push({
      mesh: mesh,
      mat: trailMat,
      geo: trailGeo,
      born: performance.now(),
      lifetime: 10000, // 10 giây
    });
    requestDirty();
  }

  // 5. Quản lý Dirty Rendering & Animation Loop
  function requestDirty() {
    isDirty = true;
    if (!rafId && !isDisposed && !document.hidden) {
      rafId = requestAnimationFrame(animate);
    }
  }

  // Animation mượt cho camera
  let cameraTransition = null;

  function focusPoint(targetPos, distance, durationMs) {
    durationMs = durationMs || 600;
    const startTarget = cameraTarget.clone();
    const endTarget = targetPos.clone();
    const startDist = cameraDistance;
    const endDist = distance || 36;
    const startTime = performance.now();

    cameraTransition = {
      startTarget,
      endTarget,
      startDist,
      endDist,
      startTime,
      duration: durationMs,
    };
    requestDirty();
  }

  function focusTask(id) {
    const key = String(id);
    for (const [k, char] of characters.entries()) {
      if (k.endsWith(':' + key) || String(char.userData.id) === key) {
        focusPoint(char.position, 32);
        selectCharacter(char);
        return true;
      }
    }
    return false;
  }

  // 6. Cập nhật Dữ liệu ({ board, tasks })
  function update(data) {
    if (!data || isDisposed) return;
    const board = data.board || {};
    const tasks = data.tasks || [];
    const projects = data.projects || [];

    const activeKeys = new Set();
    const roomCounts = {};
    ROOMS.forEach((r) => { roomCounts[r.id] = 0; });

    // A. Xếp các task vào phòng
    tasks.forEach((task) => {
      const key = 'task:' + task.id;
      activeKeys.add(key);

      const placed = placeTask(task, board, projects);
      const roomId = placed.room || 'dieu_phoi';
      roomCounts[roomId] = (roomCounts[roomId] || 0) + 1;

      const desks = getDeskPositions(roomId);
      const deskIdx = (roomCounts[roomId] - 1) % desks.length;
      const targetPos = new THREE.Vector3(desks[deskIdx].x, 0, desks[deskIdx].z);

      let char = characters.get(key);
      if (!char) {
        char = createCharacterMesh(task, 'task');
        char.position.copy(targetPos);
        char.userData.currentPos.copy(targetPos);
        char.userData.targetPos.copy(targetPos);
        char.userData.roomId = roomId;
        characters.set(key, char);
      } else {
        // Cập nhật dữ liệu
        char.userData.item = task;
        if (char.userData.roomId !== roomId) {
          const oldRoom = char.userData.roomId;
          char.userData.roomId = roomId;
          if (reducedMotion) {
            char.position.copy(targetPos);
            char.userData.currentPos.copy(targetPos);
            char.userData.targetPos.copy(targetPos);
          } else {
            char.userData.waypoints = createWaypoints(char.position, targetPos, oldRoom, roomId);
            char.userData.targetPos.copy(targetPos);
            char.userData.isMoving = true;
            char.userData.moveProgress = 0;
          }
        }
      }
    });

    // B. Xếp các đề xuất (proposals)
    if (board.proposals && Array.isArray(board.proposals)) {
      board.proposals.forEach((prop) => {
        if (prop.status === 'rejected') return;
        const key = 'prop:' + prop.id;
        activeKeys.add(key);

        const placed = placeProposal(prop);
        const roomId = placed.room;
        roomCounts[roomId] = (roomCounts[roomId] || 0) + 1;

        const desks = getDeskPositions(roomId);
        const deskIdx = (roomCounts[roomId] - 1) % desks.length;
        const targetPos = new THREE.Vector3(desks[deskIdx].x, 0, desks[deskIdx].z);

        let char = characters.get(key);
        if (!char) {
          char = createCharacterMesh(prop, 'proposal');
          char.position.copy(targetPos);
          char.userData.currentPos.copy(targetPos);
          char.userData.targetPos.copy(targetPos);
          char.userData.roomId = roomId;
          characters.set(key, char);
        } else {
          char.userData.item = prop;
          if (char.userData.roomId !== roomId) {
            const oldRoom = char.userData.roomId;
            char.userData.roomId = roomId;
            if (reducedMotion) {
              char.position.copy(targetPos);
            } else {
              char.userData.waypoints = createWaypoints(char.position, targetPos, oldRoom, roomId);
              char.userData.isMoving = true;
              char.userData.moveProgress = 0;
            }
          }
        }
      });
    }

    // C. Xếp các mục intake (chờ duyệt/triaging)
    if (board.intake && Array.isArray(board.intake)) {
      board.intake.forEach((it) => {
        if (it.status === 'cancelled' || it.status === 'converted') return;
        const key = 'intake:' + it.id;
        activeKeys.add(key);

        const placed = placeIntake(it);
        const roomId = placed.room;
        roomCounts[roomId] = (roomCounts[roomId] || 0) + 1;

        const desks = getDeskPositions(roomId);
        const deskIdx = (roomCounts[roomId] - 1) % desks.length;
        const targetPos = new THREE.Vector3(desks[deskIdx].x, 0, desks[deskIdx].z);

        let char = characters.get(key);
        if (!char) {
          char = createCharacterMesh(it, 'intake');
          char.position.copy(targetPos);
          char.userData.currentPos.copy(targetPos);
          char.userData.targetPos.copy(targetPos);
          char.userData.roomId = roomId;
          characters.set(key, char);
        } else {
          char.userData.item = it;
        }
      });
    }

    // D. Dọn các nhân vật không còn trong dữ liệu
    for (const [key, char] of characters.entries()) {
      if (!activeKeys.has(key)) {
        scene.remove(char);
        char.traverse((obj) => {
          if (obj.geometry && obj.geometry !== bodyGeo && obj.geometry !== headGeo && obj.geometry !== ringGeo) {
            obj.geometry.dispose();
          }
          if (obj.material) {
            obj.material.dispose();
          }
        });
        characters.delete(key);
      }
    }

    // Cập nhật số đếm trên badge phòng
    roomBadges.forEach((rb) => {
      const c = roomCounts[rb.roomId] || 0;
      rb.count = c;
      if (rb.countEl) {
        rb.countEl.textContent = `(${c})`;
      }
    });

    requestDirty();
  }

  // 7. Raycasting & Tương tác Chuột / Cảm ứng
  const raycaster = new THREE.Raycaster();
  const mouse = new THREE.Vector2(-999, -999);
  let hoveredChar = null;
  let selectedChar = null;

  // Tooltip DOM element
  const tooltip = document.createElement('div');
  tooltip.className = 'absolute pointer-events-none hidden px-2.5 py-1.5 rounded-lg text-xs shadow-lg bg-slate-900/90 text-white dark:bg-slate-800/95 border border-slate-700 backdrop-blur z-20 max-w-xs transition-opacity duration-150';
  overlay.appendChild(tooltip);

  function selectCharacter(char) {
    if (selectedChar && selectedChar.userData.ringMesh) {
      const u = selectedChar.userData.item && selectedChar.userData.item.priority === 'urgent';
      selectedChar.userData.ringMesh.material.color.setHex(u ? 0xef4444 : 0x64748b);
      selectedChar.userData.ringMesh.material.opacity = u ? 0.9 : 0.4;
    }
    selectedChar = char;
    if (selectedChar && selectedChar.userData.ringMesh) {
      selectedChar.userData.ringMesh.material.color.setHex(0x6366f1);
      selectedChar.userData.ringMesh.material.opacity = 1.0;
      if (callbacks.onSelect) {
        callbacks.onSelect(selectedChar.userData.item, selectedChar.userData.itemType);
      }
    } else {
      if (callbacks.onSelect) {
        callbacks.onSelect(null);
      }
    }
    requestDirty();
  }

  // Xử lý kéo thả Camera (Orbit & Pan)
  let isDragging = false;
  let dragMode = 'orbit'; // 'orbit' hoặc 'pan'
  let lastPointerX = 0;
  let lastPointerY = 0;
  let pointerDownTime = 0;

  function onPointerDown(e) {
    if (e.target !== renderer.domElement) return;
    isDragging = true;
    dragMode = (e.button === 2 || e.shiftKey) ? 'pan' : 'orbit';
    lastPointerX = e.clientX;
    lastPointerY = e.clientY;
    pointerDownTime = performance.now();
    renderer.domElement.focus();
  }

  function onPointerMove(e) {
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;

    if (isDragging) {
      const dx = e.clientX - lastPointerX;
      const dy = e.clientY - lastPointerY;
      lastPointerX = e.clientX;
      lastPointerY = e.clientY;

      if (dragMode === 'orbit') {
        orbitAngle -= dx * 0.006;
        // Giới hạn góc xoay ±60 độ so với góc nhìn thẳng
        orbitAngle = Math.max(-Math.PI / 3, Math.min(Math.PI / 3, orbitAngle));

        pitchAngle += dy * 0.006;
        pitchAngle = Math.max(0.45, Math.min(1.2, pitchAngle));
      } else {
        // Pan camera song song với mặt đất
        const panSpeed = cameraDistance * 0.0012;
        const forward = new THREE.Vector3(-Math.sin(orbitAngle), 0, -Math.cos(orbitAngle));
        const right = new THREE.Vector3(Math.cos(orbitAngle), 0, -Math.sin(orbitAngle));

        cameraTarget.add(right.multiplyScalar(-dx * panSpeed));
        cameraTarget.add(forward.multiplyScalar(dy * panSpeed));

        // Giới hạn trong khuôn viên văn phòng
        cameraTarget.x = Math.max(-38, Math.min(38, cameraTarget.x));
        cameraTarget.z = Math.max(-22, Math.min(22, cameraTarget.z));
      }

      updateCameraTransform();
      requestDirty();
      return;
    }

    // Raycast hover khi không kéo thả
    checkHover(e.clientX - rect.left, e.clientY - rect.top);
  }

  function onPointerUp(e) {
    if (!isDragging) return;
    isDragging = false;
    const dur = performance.now() - pointerDownTime;

    // Nếu thời gian nhấn chuột ngắn và ít xê dịch -> Click
    if (dur < 250) {
      raycastClick();
    }
  }

  function onWheel(e) {
    e.preventDefault();
    cameraDistance += e.deltaY * 0.05;
    cameraDistance = Math.max(22, Math.min(120, cameraDistance));
    updateCameraTransform();
    requestDirty();
  }

  function checkHover(screenX, screenY) {
    raycaster.setFromCamera(mouse, camera);
    const hitMeshes = [];
    for (const char of characters.values()) {
      hitMeshes.push(char.userData.bodyMesh, char.userData.headMesh);
    }
    const intersects = raycaster.intersectObjects(hitMeshes, false);

    if (intersects.length > 0) {
      const topHit = intersects[0].object.parent;
      renderer.domElement.style.cursor = 'pointer';
      if (hoveredChar !== topHit) {
        hoveredChar = topHit;
        const it = hoveredChar.userData.item;
        const type = hoveredChar.userData.itemType;
        showTooltip(it, type, screenX, screenY);
      } else {
        moveTooltip(screenX, screenY);
      }
    } else {
      renderer.domElement.style.cursor = isDragging ? 'grabbing' : 'default';
      if (hoveredChar) {
        hoveredChar = null;
        tooltip.classList.add('hidden');
      }
    }
  }

  function showTooltip(item, type, x, y) {
    if (!item) return;
    let title = item.title || item.request || item.slug || 'Mục';
    if (title.length > 36) title = title.slice(0, 36) + '…';
    const proj = item.project_name || item.project || 'Chung';

    let statusLine = '';
    if (item.quota_detail) {
      statusLine = `<div class="text-rose-400 font-medium">⏱ ${item.quota_detail}</div>`;
    } else if (item.current_step) {
      statusLine = `<div class="text-sky-300">Bước: ${item.current_step}</div>`;
    }

    tooltip.innerHTML = `
      <div class="font-semibold text-slate-100">${title}</div>
      <div class="text-[11px] text-slate-300">Dự án: ${proj}</div>
      ${statusLine}
    `;
    tooltip.classList.remove('hidden');
    moveTooltip(x, y);
  }

  function moveTooltip(x, y) {
    tooltip.style.left = `${x + 12}px`;
    tooltip.style.top = `${y - 12}px`;
  }

  function raycastClick() {
    raycaster.setFromCamera(mouse, camera);
    const hitMeshes = [];
    for (const char of characters.values()) {
      hitMeshes.push(char.userData.bodyMesh, char.userData.headMesh);
    }
    const intersects = raycaster.intersectObjects(hitMeshes, false);
    if (intersects.length > 0) {
      const targetChar = intersects[0].object.parent;
      selectCharacter(targetChar);
    } else {
      selectCharacter(null);
    }
  }

  // Keyboard navigation
  function onKeyDown(e) {
    if (e.target !== renderer.domElement) return;
    let handled = false;
    if (e.key === 'ArrowLeft') {
      orbitAngle = Math.max(-Math.PI / 3, orbitAngle - 0.05);
      handled = true;
    } else if (e.key === 'ArrowRight') {
      orbitAngle = Math.min(Math.PI / 3, orbitAngle + 0.05);
      handled = true;
    } else if (e.key === 'ArrowUp') {
      pitchAngle = Math.max(0.45, pitchAngle - 0.04);
      handled = true;
    } else if (e.key === 'ArrowDown') {
      pitchAngle = Math.min(1.2, pitchAngle + 0.04);
      handled = true;
    } else if (e.key === '+' || e.key === '=') {
      cameraDistance = Math.max(22, cameraDistance - 4);
      handled = true;
    } else if (e.key === '-' || e.key === '_') {
      cameraDistance = Math.min(120, cameraDistance + 4);
      handled = true;
    } else if (e.key === '0') {
      focusPoint(defaultTarget, DEFAULT_DISTANCE, 400);
      orbitAngle = 0;
      pitchAngle = 0.95;
      handled = true;
    } else if (e.key === 'Escape') {
      selectCharacter(null);
      handled = true;
    }

    if (handled) {
      e.preventDefault();
      updateCameraTransform();
      requestDirty();
    }
  }

  // Gắn các listener tương tác
  const dom = renderer.domElement;
  dom.addEventListener('pointerdown', onPointerDown);
  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  dom.addEventListener('wheel', onWheel, { passive: false });
  dom.addEventListener('keydown', onKeyDown);
  dom.addEventListener('contextmenu', (e) => e.preventDefault());

  // Tạm dừng khi chuyển tab
  function onVisibilityChange() {
    if (!document.hidden) {
      requestDirty();
    }
  }
  document.addEventListener('visibilitychange', onVisibilityChange);

  // 8. Animation & Render Loop
  let lastTime = performance.now();

  function animate(now) {
    if (isDisposed) return;
    rafId = null;

    const dt = Math.min((now - lastTime) / 1000, 0.1);
    lastTime = now;

    let needNextFrame = false;

    // A. Camera smooth transition
    if (cameraTransition) {
      needNextFrame = true;
      const progress = Math.min((now - cameraTransition.startTime) / cameraTransition.duration, 1);
      // Easing cubic out
      const ease = 1 - Math.pow(1 - progress, 3);
      cameraTarget.lerpVectors(cameraTransition.startTarget, cameraTransition.endTarget, ease);
      cameraDistance = cameraTransition.startDist + (cameraTransition.endDist - cameraTransition.startDist) * ease;
      updateCameraTransform();

      if (progress >= 1) {
        cameraTransition = null;
      }
    }

    // B. Cập nhật bước đi của nhân vật
    for (const char of characters.values()) {
      const u = char.userData;
      if (u.isMoving && u.waypoints && u.waypoints.length > 0) {
        needNextFrame = true;
        const currentTarget = u.waypoints[0];
        const step = 7.0 * dt; // Tốc độ di chuyển ~7 units/sec
        const prevPos = char.position.clone();
        char.position.moveTowards(currentTarget, step);

        // Tạo vệt sáng
        if (Math.random() < 0.3) {
          addTrailSegment(prevPos, char.position, projectColor(u.item.project));
        }

        // Nhấp nhô khi đi bộ
        u.bobOffset += dt * 14;
        char.position.y = Math.abs(Math.sin(u.bobOffset)) * 0.12;

        if (char.position.distanceTo(currentTarget) < 0.1) {
          u.waypoints.shift();
          if (u.waypoints.length === 0) {
            u.isMoving = false;
            char.position.y = 0;
          }
        }
      } else if (u.item && u.item.group === 'running') {
        // Nhấp nhô nhẹ tại chỗ khi đang chạy
        needNextFrame = true;
        u.bobOffset += dt * 3;
        char.position.y = Math.sin(u.bobOffset) * 0.05;
      }
    }

    // C. Mờ dần các vệt sáng (trails)
    for (let i = trails.length - 1; i >= 0; i--) {
      needNextFrame = true;
      const t = trails[i];
      const age = now - t.born;
      if (age >= t.lifetime) {
        scene.remove(t.mesh);
        t.geo.dispose();
        t.mat.dispose();
        trails.splice(i, 1);
      } else {
        t.mat.opacity = 0.7 * (1 - age / t.lifetime);
      }
    }

    // D. Cập nhật vị trí các badge tên phòng trên HTML overlay
    const rect = renderer.domElement.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      roomBadges.forEach((rb) => {
        const p = rb.worldPos.clone().project(camera);
        if (p.z < 1) {
          const sx = (p.x * 0.5 + 0.5) * rect.width;
          const sy = (-p.y * 0.5 + 0.5) * rect.height;
          rb.el.style.transform = `translate(${Math.round(sx)}px, ${Math.round(sy)}px) translate(-50%, -100%)`;
          rb.el.style.display = 'block';
        } else {
          rb.el.style.display = 'none';
        }
      });
    }

    // Render frame
    renderer.render(scene, camera);

    if (needNextFrame && !document.hidden) {
      rafId = requestAnimationFrame(animate);
    }
  }

  // 9. Resize Handling
  function resize() {
    if (isDisposed) return;
    const w = container.clientWidth;
    const h = container.clientHeight;
    if (w === 0 || h === 0) return;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
    requestDirty();
  }

  const resizeObserver = new ResizeObserver(() => resize());
  resizeObserver.observe(container);

  // 10. Giải phóng tài nguyên (Dispose)
  function dispose() {
    isDisposed = true;
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    resizeObserver.disconnect();
    themeObserver.disconnect();
    document.removeEventListener('visibilitychange', onVisibilityChange);
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);

    // Xoá DOM overlay và renderer canvas
    if (overlay && overlay.parentNode) {
      overlay.parentNode.removeChild(overlay);
    }
    if (renderer.domElement && renderer.domElement.parentNode) {
      renderer.domElement.parentNode.removeChild(renderer.domElement);
    }

    // Dọn trails
    trails.forEach((t) => {
      scene.remove(t.mesh);
      t.geo.dispose();
      t.mat.dispose();
    });
    trails.length = 0;

    // Giải phóng geometries & materials dùng chung
    bodyGeo.dispose();
    headGeo.dispose();
    ringGeo.dispose();
    iconWarningGeo.dispose();
    iconPendingGeo.dispose();
    corridorGeo.dispose();
    corridorMat.dispose();
    deskGeo.dispose();
    deskMat.dispose();
    chairGeo.dispose();
    chairMat.dispose();
    monitorGeo.dispose();
    monitorMat.dispose();
    screenGlowMat.dispose();

    // Dọn nhân vật
    characters.forEach((char) => {
      scene.remove(char);
      char.traverse((obj) => {
        if (obj.geometry) obj.geometry.dispose();
        if (obj.material) obj.material.dispose();
      });
    });
    characters.clear();

    renderer.dispose();
    renderer.forceContextLoss();
  }

  // Khởi động frame đầu tiên
  requestDirty();

  return {
    update,
    focusTask,
    resize,
    dispose,
    selectCharacter,
  };
}

// Helper moveTowards cho THREE.Vector3
if (!THREE.Vector3.prototype.moveTowards) {
  THREE.Vector3.prototype.moveTowards = function (target, maxDistance) {
    const dx = target.x - this.x;
    const dy = target.y - this.y;
    const dz = target.z - this.z;
    const distSq = dx * dx + dy * dy + dz * dz;
    if (distSq === 0 || (maxDistance >= 0 && distSq <= maxDistance * maxDistance)) {
      this.copy(target);
      return this;
    }
    const dist = Math.sqrt(distSq);
    this.x += (dx / dist) * maxDistance;
    this.y += (dy / dist) * maxDistance;
    this.z += (dz / dist) * maxDistance;
    return this;
  };
}

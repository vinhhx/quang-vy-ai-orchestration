# Plan U2: văn phòng 3D trên màn "Điều phối"

Repo `/mnt/data/IDE/QuangVy-Orchestration`, crate `agent-brain/`, nhánh `main`. Đọc toàn bộ file này trước khi làm. U1 đã xong (commit `2ed38a3`, `db45a7f`): màn `#/queue` gồm `assets/dashboard/js/queue.js` và `screens/queue.html`, có `<section data-queue-visual>` để trống cho U2.

## Mục tiêu
Trong vùng `data-queue-visual`, ngay trên 5 cột của U1 và vẫn **trên cùng một màn**, vẽ một **văn phòng 3D nhìn chéo từ trên xuống** bằng three.js.
- Mỗi **phòng** là một công đoạn công việc.
- Mỗi **task** là một nhân vật. Nhân vật **đi từ phòng này sang phòng kia** theo tiến độ thật và để lại vệt sáng trên sàn.
- Bấm vào nhân vật thì mở thẻ chi tiết, dùng lại các hành động của U1.
- Năm cột U1 giữ nguyên bên dưới.

Thiết kế là của mình: chỉ lấy ý tưởng "văn phòng nhiều phòng ban nhìn từ trên xuống", không sao chép giao diện, logo hay bố cục của sản phẩm nào khác.

## Bối cảnh (đã kiểm tra trong code)
- **CSP** (`src/dashboard/auth.rs:13`): `script-src 'self'`. Không được có script inline, không dùng importmap inline, không dùng CDN.
- **Router** (`js/app.js`, `loadScript`): nạp `/js/<screen>.js` bằng thẻ `<script>` thường. `import()` động tới file cùng origin vẫn được CSP cho phép.
- **Dữ liệu U1** (`queue.js`): `GET /api/orchestrator/board` trả `status`, `intake`, `proposals`, `quota_stops`, `recent`; `GET /api/tasks?limit=100` trả các task. Màn tự tải lại mỗi 5 giây qua `window.AB.every`. Văn phòng 3D **dùng chung** dữ liệu này, không gọi API riêng.
- **Mỗi task trong `/api/tasks`** có các trường sau:
  - `steps[]`: mỗi phần tử là `StepView {role, state, label, run_id, …}` (`src/dashboard/taskstate.rs:31`).
  - `current_step`: chỉ có 3 giá trị thô `lead|dev|qa`, xem `current_step_of`.
  - `group`, `auto_state`, `quota_detail`, `profile`, `project`, `project_name`, `title`, `status`.
  - **Không có** tên `workflow` và tên `team`.
- **Công đoạn của quy trình:** `src/workflows.rs` khai báo `STAGES = plan, plan-review, gate:you, implement, code-review, qa, fix`. Các vai: `lead, dev, qa, plan-review, code-review` (`src/teams.rs:15`).
- **Cấu hình hiện tại của người dùng:**
  - quy trình `analyze` (`plan → gate:you → implement → qa`) và `generate-video` (`plan → gate:you → implement → qa → fix`)
  - đội hình `claude-max` và `antigravity-full`
  - loại dự án: `code | media | idea` (`src/projects.rs:609`).

## Phần A: vendor three.js

Dùng three.js **0.186.0**, cùng phiên bản với các skill trong `.claude/skills/threejs-*`. Bản npm 0.186.0 không có file `.min`; bản ES module gồm 2 file, và `three.module.js` import `./three.core.js`.

1. Tải và kiểm tra:
   ```bash
   D=$(mktemp -d) && curl -fsSL -o "$D/three.tgz" https://registry.npmjs.org/three/-/three-0.186.0.tgz \
   && echo "61eeff9d7616005c9a481c796f52287d81fbbbc0d55eaca5565322924252c1aa  $D/three.tgz" | sha256sum -c - \
   && tar xzf "$D/three.tgz" -C "$D" package/build/three.core.js package/build/three.module.js package/LICENSE
   ```
2. Chép sang `agent-brain/assets/dashboard/vendor/`, giữ nguyên nội dung, không sửa byte nào:
   - `three.core.js` (1458113 byte, sha256 `9edde002b066a9a05676a6127f67735b62baf399bdea529f2f7e31657da769e6`)
   - `three.module.js` (662772 byte, sha256 `9052042d676cb0fdc1ddfefe193053f34b7ac0513a616fdac4535d49987812ea`)

   Kiểm tra lại bằng `sha256sum`.
3. Ghi vào `VENDOR.md`: thêm 2 dòng vào bảng (gói npm `three` 0.186.0, tarball integrity `sha512-cr/fIM2ddMSVbYVgkfD4jLJv7Fh/8ZTjvo+7gQeSVGUZHxpx9FDwoL5iC7hUz/LiRA8wMbqfnb90xKfm1/HHkQ==`, giấy phép MIT, repo `github.com/mrdoob/three.js`) và các lệnh tái tạo. Không đổi các dòng có sẵn.
4. Thêm 2 mục vào `ASSETS` trong `src/dashboard/assets.rs`: `/vendor/three.core.js` và `/vendor/three.module.js`, mime `text/javascript; charset=utf-8`.
5. **Không** dùng OrbitControls, CSS2DRenderer hay bất kỳ file nào trong `examples/jsm`, vì chúng import theo tên `'three'` (bare specifier) và cần importmap. Điều khiển camera và nhãn chữ tự viết (xem B3, B4).

## Phần B: văn phòng 3D

### B0. File và cách nạp
- File mới `agent-brain/assets/dashboard/js/office3d.js` là **ES module**:
  - dòng đầu là `import * as THREE from '/vendor/three.module.js';`
  - export `createOffice(container, callbacks)` trả về đối tượng `{update(data), focusTask(id), resize(), dispose()}`.
  - Thêm vào `ASSETS`.
- `queue.js` (vẫn là script thường) gọi `import('/js/office3d.js')` một lần khi màn mở, nếu máy hỗ trợ WebGL (thử `canvas.getContext('webgl2')`).
- Mỗi lần dữ liệu tải xong, `queue.js` gọi `office.update({board, tasks})`. Cấu trúc dữ liệu giữ nguyên như U1 đang có, không đổi.
- Khi rời màn (`window.AB` leave hook, xem `app.js` `_leaveHooks`), gọi `dispose()`:
  - giải phóng geometry, material và renderer (`renderer.dispose()`, `forceContextLoss()`)
  - huỷ `requestAnimationFrame` và gỡ mọi event listener.
- **Dự phòng:** không có WebGL, `import()` lỗi, hoặc người dùng tắt 3D thì ẩn vùng 3D; 5 cột vẫn chạy bình thường.
- Nút trên thanh trạng thái **"Văn phòng 3D: bật/tắt"** lưu vào `localStorage` (bọc try/catch).
- Mặc định bật. Riêng khi `prefers-reduced-motion: reduce` thì mặc định tắt chuyển động: nhân vật nhảy thẳng tới chỗ mới, không đi bộ, không có vệt chạy.

### B1. Tám phòng và quy tắc xếp nhân vật
Hàm thuần `placeTask(task, board) -> {room, desk, flags}` đặt ở **đầu** `office3d.js` và được export để test được. Xét theo thứ tự, gặp điều kiện đầu tiên đúng thì dừng:

| # | Phòng (nhãn trên cửa) | Điều kiện |
|---|---|---|
| 7 | **Kho lưu** | `group==='done'`. Chỉ hiện 10 task mới nhất theo `updated_at` |
| 6 | **Phòng nghỉ** | Đúng quy tắc cột "Dừng" của U1: `quota_detail`, hoặc nằm trong `board.quota_stops`, hoặc `auto_state==='stopped'`, hoặc `group==='attention'`. `flags.quota` khi do quota, `flags.error` khi do lỗi |
| 2 | **Phòng Giám đốc** (bạn duyệt) | Task thuộc cột "Cần bạn duyệt" của U1: `group==='waiting'` và không dừng |
| 8 | **Studio** | Task đang chạy (`group==='running'`) của **dự án loại media**, hoặc chạy quy trình `generate-video` (xem B6). `desk` là role của bước đang chạy |
| 3 | **BA / Phân tích** | Đang chạy, bước hiện tại có role `lead` hoặc `plan-review` |
| 4 | **Kỹ thuật** | Đang chạy, role `dev` hoặc `code-review` (bước `fix` dùng role dev) |
| 5 | **QA** | Đang chạy, role `qa` |
| 1 | **Điều phối** (lễ tân) | Không phải task mà là các mục trong `board.intake` (queued/triaging) và `board.proposals`. Đề xuất `pending` đứng ở cửa Phòng Giám đốc; đề xuất `approved` đứng xếp hàng "chờ slot" trong phòng Điều phối |

- **Bước hiện tại** = phần tử đầu tiên trong `steps` có `state==='running'`. Nếu không có thì lấy theo `current_step` (`lead` → BA, `dev` → Kỹ thuật, `qa` → QA). Phải đọc các giá trị `state` thật trong `taskstate.rs` trước khi viết.
- Trong phòng Điều phối, hàng chờ xếp theo ưu tiên: Gấp đứng đầu hàng.
- Mỗi task chỉ ở **một** phòng, và kết quả phải khớp với cột U1 tương ứng. Nên **tách quy tắc phân cột của U1 thành hàm dùng chung**, hoặc gọi lại đúng hàm trong `queue.js`, thay vì viết lại một bản thứ hai.

### B2. Dựng cảnh
Đọc `.claude/skills/threejs-fundamentals/SKILL.md` và `threejs-performance/SKILL.md` trước khi viết.

- **Mặt bằng lưới:** 3 phòng công đoạn chính (BA → Kỹ thuật → QA) xếp thành một hàng theo chiều dòng việc. Hàng thứ hai gồm Điều phối, Phòng Giám đốc, Studio, Phòng nghỉ, Kho lưu.
  - Mỗi phòng có sàn màu riêng nhạt, tường thấp, khe cửa và biển tên.
  - Bàn có kích thước cố định. Số chỗ ngồi mỗi phòng bằng `max_parallel` (mặc định 3). Nếu có nhiều task hơn số ghế thì các task thừa đứng cạnh bàn.
- **Đồ vật:** chỉ dùng khối hình học cơ bản (Box, Cylinder, Capsule, Sphere). Không tải model hay texture ngoài.
  - Mỗi phòng có vài chi tiết riêng để dễ nhận ra: BA có bảng trắng, Kỹ thuật có dãy màn hình, QA có kệ thiết bị, Studio có đèn và máy quay, Phòng nghỉ có sofa, Kho lưu có tủ hồ sơ, Phòng Giám đốc có bàn lớn.
  - Dùng `InstancedMesh` cho bàn, ghế và màn hình.
- **Nhân vật:** thân là capsule, đầu là sphere.
  - Màu áo theo dự án: băm `project` ra một trong 8 màu cố định, dễ phân biệt trên nền sáng và tối.
  - Viền hoặc vòng đỏ dưới chân khi ưu tiên Gấp (lấy từ proposal nối với task nếu có).
  - Biểu tượng trên đầu: `…` khi đang phân tích, `!` khi lỗi, đồng hồ khi hết quota.
- **Di chuyển:** khi phòng hoặc bàn của task đổi giữa hai lần tải, nhân vật **đi** tới chỗ mới theo đường gấp khúc qua cửa phòng, mất khoảng 1,5–3 giây. Đường đã đi để lại **vệt sáng** trên sàn, mờ dần trong khoảng 10 giây.
- **Hiệu ứng trạng thái:**
  - Đang chạy: nhân vật nhấp nhô nhẹ và màn hình trên bàn sáng.
  - Hết quota: bàn nơi task dừng chuyển đỏ, nhân vật ở Phòng nghỉ có đồng hồ đếm ngược (dùng `resume_at` của U1) hoặc chữ "Chờ chạy lại".
- **Ánh sáng và màu:** một `HemisphereLight` và một `DirectionalLight`, không bật bóng đổ. Đổi bảng màu sáng/tối theo `document.documentElement.classList.contains('dark')`; xem `js/theme.js` để biết cách dashboard báo đổi theme.
- **Hiệu năng:**
  - **Chỉ vẽ khi cần:** có chuyển động, vệt sáng đang mờ dần, đang tương tác, hoặc dữ liệu vừa đổi. Khi đứng yên thì dừng hẳn vòng lặp.
  - Ngừng vẽ khi `document.hidden`.
  - `renderer.setPixelRatio(Math.min(devicePixelRatio, 2))`.
  - Kiểm tra với 3 task chạy và 30 mục trong hàng chờ: không giật.

### B3. Camera và điều khiển (tự viết)
- Mặc định dùng `PerspectiveCamera` FOV khoảng 35°, nhìn chéo xuống khoảng 55°, thấy trọn văn phòng.
- **Chuột:**
  - kéo trái để xoay quanh trục đứng, giới hạn ±60° so với góc mặc định
  - kéo phải, hoặc Shift + kéo, để di chuyển ngang (giới hạn trong mặt bằng)
  - con lăn để zoom (giới hạn gần và xa).
- **Cảm ứng:** một ngón để xoay, hai ngón để zoom và di chuyển.
- Nút **"Về góc mặc định"**. `focusTask(id)` bay camera tới nhân vật trong khoảng 0,6 giây.
- **Phím** (khi vùng 3D đang được focus, `tabindex="0"`): mũi tên để xoay, `+`/`-` để zoom, `0` để về góc mặc định.

### B4. Nhãn và tương tác
- **Nhãn là phần tử HTML** đặt trong một lớp phủ (overlay) chồng lên canvas. Mỗi frame có vẽ thì chiếu toạ độ 3D sang toạ độ màn hình để đặt nhãn.
  - Có nhãn: tên phòng kèm số task trong phòng; nhãn ngắn của nhân vật (tiêu đề task cắt khoảng 24 ký tự) chỉ hiện khi hover, khi được chọn, hoặc khi zoom đủ gần.
  - Class Tailwind phải là chuỗi literal. Vị trí đặt bằng `style.transform`.
- **Raycast** (xem `.claude/skills/threejs-interaction/SKILL.md`):
  - hover: con trỏ thành tay và hiện tooltip (tiêu đề, dự án, bước, engine của bước nếu có trong `steps`)
  - click: chọn nhân vật và mở **thẻ chi tiết** nổi ở góc vùng 3D.
- **Thẻ chi tiết:**
  - Nhân vật là đề xuất: dùng **đúng** các hành động của thẻ đề xuất U1 (Duyệt / Sửa rồi duyệt / Từ chối / đổi ưu tiên), gọi qua `callbacks` do `queue.js` truyền vào. Không viết lại phần gọi API.
  - Nhân vật là task: link "Mở task" (`#/task/{id}`), và "Xem và duyệt plan" nếu task đang ở Phòng Giám đốc.
  - Đóng bằng `Esc` hoặc bằng nút đóng.
- **Liên kết hai chiều với 5 cột U1:** bấm vào một thẻ trong cột (thêm nút nhỏ "Xem trong văn phòng") thì gọi `focusTask`; chọn nhân vật thì làm nổi thẻ tương ứng trong cột (thêm viền `ring`).
- **Cây phòng ban bên trái** (từ `lg` trở lên; trên màn hẹp thì ẩn):
  - danh sách 8 phòng, mỗi phòng liệt kê các task bên trong
  - bấm một mục để `focusTask`
  - danh sách là phần tử HTML, bàn phím dùng được.

### B5. Bố cục
- Vùng 3D cao `min(60vh, 560px)` trên desktop và `320px` trên điện thoại. Cây phòng ban rộng `16rem` bên trái (chỉ từ `lg`).
- Thanh trạng thái và ô giao nhanh của U1 vẫn ở trên cùng, 5 cột ở dưới. Không cuộn ngang trang.
- `ResizeObserver` gọi `resize()`.

### B6. Nhận biết task của Studio
- `/api/tasks` hiện không có tên quy trình. Kiểm tra trong `api_tasks.rs` `list_tasks`:
  - Nếu tên quy trình **có sẵn** từ dữ liệu đã đọc cho mỗi task (ví dụ `tasks.team_json`, `auto_json`, hoặc nơi `start_new_task` lưu workflow) thì thêm trường `"workflow": "<tên>"|null` vào mỗi task. Đây là thay đổi backend nhỏ duy nhất được phép.
  - Kèm một test trong `tests/` (file test của `list_tasks` đang có) kiểm tra trường này xuất hiện.
- Nếu không có sẵn mà phải thêm migration thì **không làm**. Khi đó Studio chỉ dựa vào loại dự án media.
- **Loại dự án:** kiểm tra `profile` trong task là loại dự án (`code|media|idea`) hay là thứ khác. Nếu không phải, lấy loại dự án từ `/api/projects` (U1 đã nạp danh sách dự án cho ô giao nhanh).
- Ghi rõ trong báo cáo đã chọn cách nào.

## Không làm trong U2
- Không có âm thanh, không post-processing, không bóng đổ, không model hay texture ngoài.
- Không đổi API của U1, không đổi scheduler hay triage. Không thêm migration.
- Không thêm nút Telegram.

## Test và kiểm tra
Chạy trong `agent-brain/`:
```bash
cargo build --offline --locked 2>&1 | tail -3
cargo test --offline --locked --no-fail-fast 2>&1 | grep -E "^test result|FAILED|panicked"
```
Chạy ở gốc repo (`/mnt/data/IDE/QuangVy-Orchestration`):
```bash
bash scripts/build-css.sh --check
bash scripts/dashboard-ui-lint.sh
node scripts/dashboard-js-check.mjs
bash spikes/lib/scan-secrets.sh
sha256sum agent-brain/assets/dashboard/vendor/three.core.js agent-brain/assets/dashboard/vendor/three.module.js
```
- **Test logic không cần trình duyệt:** viết `scripts/office3d-place-check.mjs`, chạy bằng `node`.
  - Script import hàm `placeTask` từ `office3d.js`. Vì `office3d.js` import three.js qua đường dẫn tuyệt đối, **tách `placeTask` và các quy tắc phân phòng ra một module riêng không phụ thuộc three**: `js/office-rooms.js`, cũng thêm vào `ASSETS`. `office3d.js` import từ module đó.
  - Kiểm tra ít nhất các trường hợp: task done → Kho lưu; quota → Phòng nghỉ; waiting → Phòng Giám đốc; running với step lead / plan-review / dev / code-review / qa → đúng phòng; running thuộc dự án media → Studio; mỗi task chỉ thuộc một phòng; kết quả khớp cột U1.
- **Kết quả mong đợi:**
  - build OK
  - mọi test pass trừ `prompts_equal_phase2` (đỏ từ trước)
  - `dashboard-js-check.mjs` không có lỗi mới. Các lỗi cũ đã biết: 1 ở board, 2 ở settings
  - sha256 của 2 file vendor khớp
  - scan-secrets OK
- **Chạy thử trên trình duyệt** (nếu làm được): chạy binary với **cổng và thư mục dữ liệu tạm riêng** (xem `scripts/dashboard-smoke.sh`), mở `#/queue`, kiểm tra console không có lỗi CSP hay lỗi JS. Không được động tới serve đang chạy ở cổng 7437.

## Không được đụng tới
- `.planning/STATE.md`, `.planning/ROADMAP.md`, mọi file `.planning/` (kể cả file plan này), `.claude/`, `~/.claude-orca`, `~/.config/agent-brain/*`.
- `agent-brain/assets/prompts/lead.md`, `brain/prompts/lead.md`, `tests/assets_parity.rs`.
- `js/board.js`, `js/settings.js` và các màn khác ngoài `queue`. Các lỗi cũ của `dashboard-js-check.mjs` để nguyên.
- Không kill hay restart serve ở cổng 7437, không dùng pkill/killall. Không dùng `git add -A`, không dùng `--no-verify`, không push.

## Tiêu chí nghiệm thu
1. `#/queue` có văn phòng 3D với 8 phòng có biển tên: Điều phối, Phòng Giám đốc, BA / Phân tích, Kỹ thuật, QA, Studio, Phòng nghỉ, Kho lưu. Năm cột U1 vẫn nằm bên dưới trên cùng màn.
2. Mỗi task và mỗi mục hàng chờ hiện thành đúng một nhân vật, trong đúng phòng theo B1, khớp với cột U1.
3. Khi bước của task đổi, nhân vật đi sang phòng hoặc bàn mới và để lại vệt sáng mờ dần.
4. Task hết quota nằm ở Phòng nghỉ, bàn nơi nó dừng chuyển đỏ, có đếm ngược nếu bật tự chạy lại.
5. Hover hiện tooltip. Bấm vào nhân vật đề xuất thì duyệt hoặc từ chối được ngay. Cây bên trái và nút "Xem trong văn phòng" đưa camera tới đúng nhân vật.
6. Khi đứng yên thì không vẽ lại (kiểm tra bằng bộ đếm frame trong code hoặc bằng log debug tắt mặc định). Rời màn thì giải phóng WebGL.
7. Không có WebGL hoặc tắt 3D: màn vẫn dùng được với 5 cột. Không có lỗi CSP.
8. Các test cũ không đổi kết quả, `office3d-place-check.mjs` pass.

## Commit
1. `chore(agent-brain): vendor three.js 0.186.0 for the dashboard`: 2 file vendor, VENDOR.md, assets.rs.
2. `feat(agent-brain): 3D office on the Điều phối screen`: office-rooms.js, office3d.js, queue.js/queue.html, assets.rs, app.css, script check, cùng thay đổi B6 và test của nó nếu có.

Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Báo cáo cuối (agy phải ghi)
- File và hash của từng commit.
- Kết quả nguyên văn các lệnh kiểm tra.
- Cách đã chọn ở B6.
- Giá trị `state` của step đọc được trong code.
- Chỗ lệch plan.
- Phần chưa kiểm tra, ví dụ chạy thật trên trình duyệt hoặc trên điện thoại.

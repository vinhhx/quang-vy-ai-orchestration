# Front-End Vendored Tooling & Assets

Tài liệu ghi lại nguồn gốc, mã băm sha256 và giấy phép của các tệp front-end và công cụ được sử dụng trong Phase 5.

## Danh sách tệp vendored

| Đường dẫn đích | Gói / Phiên bản | Tệp nguồn | Kích thước (byte) | SHA-256 | Nguồn / Repository | Giấy phép | Mục đích sử dụng |
|---|---|---|---|---|---|---|---|
| `vendor/alpine.csp.min.js` | npm `@alpinejs/csp` 3.17.4 | `package/dist/cdn.min.js` | 72198 | `0d18d7f8d7910e2e0212f0f056b12f50bebc3abb7d88d2f7c7cb4c336fe4519a` | `https://registry.npmjs.org/@alpinejs/csp/-/csp-3.17.4.tgz` (tarball integrity `sha512-SlRXmqO6kYhnxlg+99etmuzJtE9Lk4QbKjBHqerXzaMflJqoJXdz/SI3IvHJGZ/vRVyC3bR0SSBz40oY7goBeg==`), repo `github.com/alpinejs/alpine` | MIT | Nhúng trực tiếp vào binary `agent-brain` |
| `vendor/three.core.js` | npm `three` 0.186.0 | `package/build/three.core.js` | 1458113 | `9edde002b066a9a05676a6127f67735b62baf399bdea529f2f7e31657da769e6` | `https://registry.npmjs.org/three/-/three-0.186.0.tgz` (tarball integrity `sha512-cr/fIM2ddMSVbYVgkfD4jLJv7Fh/8ZTjvo+7gQeSVGUZHxpx9FDwoL5iC7hUz/LiRA8wMbqfnb90xKfm1/HHkQ==`), repo `github.com/mrdoob/three.js` | MIT | Dựng cảnh văn phòng 3D trên màn Điều phối (core runtime) |
| `vendor/three.module.js` | npm `three` 0.186.0 | `package/build/three.module.js` | 662772 | `9052042d676cb0fdc1ddfefe193053f34b7ac0513a616fdac4535d49987812ea` | `https://registry.npmjs.org/three/-/three-0.186.0.tgz` (tarball integrity `sha512-cr/fIM2ddMSVbYVgkfD4jLJv7Fh/8ZTjvo+7gQeSVGUZHxpx9FDwoL5iC7hUz/LiRA8wMbqfnb90xKfm1/HHkQ==`), repo `github.com/mrdoob/three.js` | MIT | ES module entry point cho three.js trên dashboard |
| `agent-brain/tools/tailwindcss` | Tailwind CSS standalone CLI `v4.3.3` (`tailwindcss-linux-x64`) | `tailwindcss-linux-x64` | 111749248 | `dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a` | `https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-linux-x64` | MIT | Chỉ dùng lúc phát triển (dev-time only, gitignored, không nhúng vào binary; tệp đầu ra `app.css` mang banner MIT) |

## Cách tái tạo

Các bước người dùng thực hiện khi tải công cụ và vendor tệp (Task 3, bước 5–7):

```bash
# Bước 5: Tải Tailwind binary và kiểm tra sha256
mkdir -p agent-brain/tools && curl -fsSL -o agent-brain/tools/tailwindcss https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-linux-x64 && echo "dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a  agent-brain/tools/tailwindcss" | sha256sum -c - && chmod +x agent-brain/tools/tailwindcss

# Bước 6: Tải @alpinejs/csp qua npm pack, kiểm tra sha256 và sao chép vào assets
D=$(mktemp -d) && (cd "$D" && npm pack @alpinejs/csp@3.17.4 && tar -xzf alpinejs-csp-3.17.4.tgz) && echo "0d18d7f8d7910e2e0212f0f056b12f50bebc3abb7d88d2f7c7cb4c336fe4519a  $D/package/dist/cdn.min.js" | sha256sum -c - && mkdir -p agent-brain/assets/dashboard/vendor && cp "$D/package/dist/cdn.min.js" agent-brain/assets/dashboard/vendor/alpine.csp.min.js

# Bước 6b: Tải three 0.186.0 qua npm tarball, kiểm tra sha256 và sao chép vào assets
D=$(mktemp -d) && curl -fsSL -o "$D/three.tgz" https://registry.npmjs.org/three/-/three-0.186.0.tgz \
  && echo "61eeff9d7616005c9a481c796f52287d81fbbbc0d55eaca5565322924252c1aa  $D/three.tgz" | sha256sum -c - \
  && tar xzf "$D/three.tgz" -C "$D" package/build/three.core.js package/build/three.module.js package/LICENSE \
  && cp "$D/package/build/three.core.js" agent-brain/assets/dashboard/vendor/ \
  && cp "$D/package/build/three.module.js" agent-brain/assets/dashboard/vendor/ \
  && rm -rf "$D"

# Bước 7: Dựng CSS và kiểm tra tính mới
bash scripts/build-css.sh && bash scripts/build-css.sh --check
```

**Quy tắc cập nhật:** Việc cập nhật bất kỳ tệp nào trong danh sách trên đều bắt buộc phải trải qua một checkpoint người dùng mới để kiểm tra nguồn gốc và mã băm.

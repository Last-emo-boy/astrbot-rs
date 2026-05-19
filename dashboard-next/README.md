# AstrBot Dashboard Next

SolidJS + Vite + TypeScript 重写版前端。详见 `.workflow/scratch/dashboard-next-design-2026-05-19/context.md`。

## 开发

```bash
npm install
npm run dev        # http://127.0.0.1:5173/ → 代理 /api /webchat /ws 到 127.0.0.1:6185
npm run build      # 产物输出到 dist/
npm run typecheck
```

## DTO 生成

后端 DTO 通过 `ts-rs` 在 `cargo test -p astrbot-web` 时导出到 `src/api/dto/`。**禁止手工编辑该目录下的 `.ts` 文件**。

```bash
cargo test -p astrbot-web        # 重新生成 DTO
git diff --exit-code src/api/dto # CI 漂移检测
```

## 资产加载

Rust 运行时按 `Explicit > UserDist > NextDist > BundledDist` 顺序解析，`NextDist` 默认指向 `data/dashboard-next/dist`，构建产物可直接拷贝/链接到该路径以投入运行时使用。

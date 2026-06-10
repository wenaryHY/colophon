# Colophon 图标使用指南

## 图标文件说明

### 1. `src/app-icon.svg` - 应用图标（正方形）
- **尺寸**: 112x112 (正方形)
- **用途**: 
  - 桌面应用图标
  - 任务栏图标
  - 系统托盘图标
  - Favicon
- **转换目标**: PNG (多尺寸)

### 2. `src/logo-icon.svg` - 原始图标
- **尺寸**: 80x112 (接近正方形)
- **用途**: 备用图标

### 3. `src/logo.svg` - 完整 Logo（横向）
- **尺寸**: 350x100 (横向长条)
- **用途**:
  - 网站顶部横幅
  - 启动画面
  - 关于页面
  - 营销材料
- **不适合**: 应用图标（会被压扁）

## SVG 转 PNG 步骤

### 方案一：在线工具（推荐）

1. **访问**: [https://svg-to-png.org/](https://svg-to-png.org/)

2. **转换 `app-icon.svg`** 为以下尺寸：
   ```
   32x32     - 小图标
   128x128   - 中等图标
   256x256   - 标准图标
   512x512   - 高清图标
   1024x1024 - 超高清（可选）
   ```

3. **保存位置**:
   ```
   src-tauri/icons/
   ├── 32x32.png
   ├── 128x128.png
   ├── 128x128@2x.png (256x256 重命名)
   ├── icon.png (512x512 或 1024x1024)
   └── Square*.png (Windows Store 需要)
   ```

### 方案二：使用 Inkscape

```bash
# 安装 Inkscape
# Windows: https://inkscape.org/release/

# 转换命令
inkscape src/app-icon.svg --export-filename=src-tauri/icons/32x32.png --export-width=32 --export-height=32
inkscape src/app-icon.svg --export-filename=src-tauri/icons/128x128.png --export-width=128 --export-height=128
inkscape src/app-icon.svg --export-filename=src-tauri/icons/128x128@2x.png --export-width=256 --export-height=256
inkscape src/app-icon.svg --export-filename=src-tauri/icons/icon.png --export-width=512 --export-height=512
```

### 方案三：使用 ImageMagick

```bash
# 安装 ImageMagick
# Windows: https://imagemagick.org/script/download.php

# 转换命令
magick src/app-icon.svg -resize 32x32 src-tauri/icons/32x32.png
magick src/app-icon.svg -resize 128x128 src-tauri/icons/128x128.png
magick src/app-icon.svg -resize 256x256 src-tauri/icons/128x128@2x.png
magick src/app-icon.svg -resize 512x512 src-tauri/icons/icon.png
```

## Tauri 图标要求

### 必需文件
- `icon.png` - 主图标 (512x512 或更大)
- `32x32.png` - Windows 小图标
- `128x128.png` - macOS/Linux 标准图标
- `128x128@2x.png` - macOS Retina 显示

### 可选文件（Windows Store）
- `Square30x30Logo.png` (30x30)
- `Square44x44Logo.png` (44x44)
- `Square71x71Logo.png` (71x71)
- `Square89x89Logo.png` (89x89)
- `Square107x107Logo.png` (107x107)
- `Square142x142Logo.png` (142x142)
- `Square150x150Logo.png` (150x150)
- `Square284x284Logo.png` (284x284)
- `Square310x310Logo.png` (310x310)

## 图标生成后的配置

转换完成后，更新 `src-tauri/tauri.conf.json`:

```json
{
  "bundle": {
    "active": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.png"
    ]
  }
}
```

## 注意事项

1. **透明背景**: 确保 PNG 保留透明背景
2. **清晰度**: 使用矢量转换，避免模糊
3. **尺寸精确**: 严格按照要求的尺寸生成
4. **文件命名**: 严格按照 Tauri 要求命名

## 快速开始

1. 使用 `src/app-icon.svg`（正方形）作为应用图标
2. 访问 https://svg-to-png.org/
3. 上传 `app-icon.svg`
4. 生成 32, 128, 256, 512 尺寸
5. 保存到 `src-tauri/icons/` 目录
6. 运行 `cargo tauri build` 测试打包
